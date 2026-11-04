use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;

use crate::models::{LaunchProfile, MicrosoftAccount};

const AUTH_ENDPOINT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN_ENDPOINT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_ENDPOINT: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_ENDPOINT: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT_LOGIN_ENDPOINT: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const ENTITLEMENTS_ENDPOINT: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const PROFILE_ENDPOINT: &str = "https://api.minecraftservices.com/minecraft/profile";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

struct MicrosoftTokens {
    access_token: String,
    refresh_token: Option<String>,
}

struct MinecraftSession {
    account: MicrosoftAccount,
    profile: LaunchProfile,
    refresh_token: String,
}

pub fn sign_in(client_id: &str) -> Result<(MicrosoftAccount, LaunchProfile), String> {
    ensure_token_storage_supported()?;
    let client_id = validate_client_id(client_id)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|_| "Could not start the local sign-in callback. Close another launcher sign-in and retry.".to_owned())?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "Could not prepare the local sign-in callback.".to_owned())?;
    let port = listener
        .local_addr()
        .map_err(|_| "Could not read the local sign-in address.".to_owned())?
        .port();
    // Entra's native desktop flow accepts a dynamic loopback port when the
    // registered redirect is `http://localhost`.
    let redirect_uri = format!("http://localhost:{port}");
    let state = Uuid::new_v4().to_string();
    let verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let challenge = pkce_challenge(&verifier);
    let authorize_url = authorization_url(&client_id, &redirect_uri, &state, &challenge)?;

    open_system_browser(authorize_url.as_str())?;
    let code = wait_for_authorization_code(&listener, &state)?;
    let client = http_client()?;
    let tokens = exchange_authorization_code(
        &client,
        &client_id,
        &redirect_uri,
        &code,
        &verifier,
    )?;
    let refresh_token = tokens
        .refresh_token
        .ok_or_else(|| "Microsoft did not return a refresh token. Check that offline_access is enabled for this app registration.".to_owned())?;
    let session = minecraft_session(&client, tokens.access_token, refresh_token)?;
    store_refresh_token(&session.account.id, &session.refresh_token)?;
    Ok((session.account, session.profile))
}

pub fn refresh(account: &MicrosoftAccount, client_id: &str) -> Result<LaunchProfile, String> {
    ensure_token_storage_supported()?;
    let client_id = validate_client_id(client_id)?;
    let stored_refresh = read_refresh_token(&account.id)?;
    let client = http_client()?;
    let tokens = refresh_microsoft_token(&client, &client_id, &stored_refresh)?;
    let replacement_refresh = tokens
        .refresh_token
        .unwrap_or_else(|| stored_refresh.clone());
    let session = minecraft_session(&client, tokens.access_token, replacement_refresh.clone())?;
    if session.account.uuid != account.uuid {
        return Err("The Microsoft session resolved to a different Minecraft profile. Sign in again to select the correct account.".to_owned());
    }
    store_refresh_token(&account.id, &session.refresh_token)?;
    Ok(session.profile)
}

pub fn remove_credentials(account_id: &str) -> Result<(), String> {
    clear_refresh_token(account_id)
}

fn validate_client_id(client_id: &str) -> Result<String, String> {
    let parsed = Uuid::parse_str(client_id.trim())
        .map_err(|_| "Enter the public application (client) ID from your Microsoft Entra app registration.".to_owned())?;
    Ok(parsed.to_string())
}

fn pkce_challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn authorization_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    challenge: &str,
) -> Result<Url, String> {
    let mut url = Url::parse(AUTH_ENDPOINT).map_err(|_| "Microsoft sign-in endpoint is invalid.".to_owned())?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", "XboxLive.signin XboxLive.offline_access")
        .append_pair("state", state)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");
    Ok(url)
}

fn open_system_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    let result = Command::new("xdg-open").arg(url).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    #[cfg(target_os = "windows")]
    let result = Command::new("rundll32.exe").arg("url.dll,FileProtocolHandler").arg(url).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let result: std::io::Result<std::process::Child> = Err(std::io::Error::other("unsupported platform"));
    result.map(|_| ()).map_err(|_| "Could not open the system browser for Microsoft sign-in.".to_owned())
}

fn wait_for_authorization_code(listener: &TcpListener, expected_state: &str) -> Result<String, String> {
    let deadline = Instant::now() + CALLBACK_TIMEOUT;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut request = [0u8; 8192];
                let count = stream.read(&mut request).unwrap_or_default();
                let request = String::from_utf8_lossy(&request[..count]);
                let result = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .map(|target| parse_callback_target(target, expected_state));
                send_callback_response(&mut stream, result.as_ref().is_some_and(Result::is_ok));
                return result.unwrap_or_else(|| Err("Microsoft returned an invalid sign-in response.".to_owned()));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return Err("The local sign-in callback stopped unexpectedly. Please retry.".to_owned()),
        }
    }
    Err("Microsoft sign-in timed out. Please start again.".to_owned())
}

fn parse_callback_target(target: &str, expected_state: &str) -> Result<String, String> {
    let callback = Url::parse(&format!("http://localhost{target}"))
        .map_err(|_| "Microsoft returned an invalid sign-in response.".to_owned())?;
    if callback.path() != "/" {
        return Err("Microsoft returned an invalid sign-in response.".to_owned());
    }
    let mut code = None;
    let mut state = None;
    let mut provider_error = None;
    for (key, value) in callback.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => provider_error = Some(value.into_owned()),
            _ => {}
        }
    }
    if state.as_deref() != Some(expected_state) {
        return Err("Microsoft sign-in could not be verified. Please close the browser tab and retry.".to_owned());
    }
    if let Some(error) = provider_error {
        return Err(match error.as_str() {
            "access_denied" => "Microsoft sign-in was cancelled.".to_owned(),
            "invalid_client" => "Microsoft rejected this app registration. Check the client ID and localhost redirect settings.".to_owned(),
            _ => "Microsoft sign-in did not complete. Check the app registration and try again.".to_owned(),
        });
    }
    code.filter(|value| !value.is_empty())
        .ok_or_else(|| "Microsoft sign-in did not return an authorization code.".to_owned())
}

fn send_callback_response(stream: &mut TcpStream, success: bool) {
    let (status, message) = if success {
        ("200 OK", "Sign-in complete. You can close this tab and return to OpnLauncher.")
    } else {
        ("400 Bad Request", "Sign-in did not complete. Return to OpnLauncher for details.")
    };
    let body = format!("<!doctype html><meta charset=utf-8><title>OpnLauncher sign-in</title><p>{message}</p>");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialize secure Microsoft sign-in requests.".to_owned())
}

fn exchange_authorization_code(
    client: &Client,
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<MicrosoftTokens, String> {
    let body = form_body(&[
            ("client_id", client_id),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
            ("scope", "XboxLive.signin XboxLive.offline_access"),
        ]);
    let response = client
        .post(TOKEN_ENDPOINT)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .map_err(|_| "Could not exchange the Microsoft sign-in code. Check your network connection.".to_owned())?;
    parse_token_response(response)
}

fn refresh_microsoft_token(client: &Client, client_id: &str, refresh_token: &str) -> Result<MicrosoftTokens, String> {
    let body = form_body(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", "XboxLive.signin XboxLive.offline_access"),
        ]);
    let response = client
        .post(TOKEN_ENDPOINT)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .map_err(|_| "Could not refresh the Microsoft session. Check your network connection.".to_owned())?;
    parse_token_response(response)
}

fn parse_token_response(response: reqwest::blocking::Response) -> Result<MicrosoftTokens, String> {
    if !response.status().is_success() {
        return Err(if response.status() == reqwest::StatusCode::BAD_REQUEST || response.status() == reqwest::StatusCode::UNAUTHORIZED {
            "Microsoft rejected the sign-in code or refresh session. Sign in again and check the app registration.".to_owned()
        } else {
            format!("Microsoft sign-in failed with HTTP {}.", response.status().as_u16())
        });
    }
    let value = response_json(response, "Microsoft returned an invalid token response.")?;
    let access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Microsoft returned no access token.".to_owned())?
        .to_owned();
    Ok(MicrosoftTokens {
        access_token,
        refresh_token: value.get("refresh_token").and_then(Value::as_str).filter(|value| !value.is_empty()).map(str::to_owned),
    })
}

fn minecraft_session(client: &Client, microsoft_access: String, refresh_token: String) -> Result<MinecraftSession, String> {
    let xbl = post_json(
        client,
        XBL_ENDPOINT,
        json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={microsoft_access}")},
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
        true,
    )?;
    let xbl_token = string_field(&xbl, "Token", "Xbox sign-in")?;
    let xsts = post_json(
        client,
        XSTS_ENDPOINT,
        json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
        true,
    )
    .map_err(|error| xsts_error(&error, None))?;
    let xsts_token = string_field(&xsts, "Token", "Xbox sign-in")?;
    let user_hash = xsts
        .pointer("/DisplayClaims/xui/0/uhs")
        .and_then(Value::as_str)
        .ok_or_else(|| "Xbox sign-in returned no user identity.".to_owned())?;
    let xuid = xsts
        .pointer("/DisplayClaims/xui/0/xid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let minecraft = post_json(
        client,
        MINECRAFT_LOGIN_ENDPOINT,
        json!({"identityToken": format!("XBL3.0 x={user_hash};{xsts_token}")}),
        false,
    )?;
    let minecraft_access = string_field(&minecraft, "access_token", "Minecraft sign-in")?.to_owned();

    let entitlements = client
        .get(ENTITLEMENTS_ENDPOINT)
        .bearer_auth(&minecraft_access)
        .send()
        .map_err(|_| "Could not verify Minecraft ownership. Check your network connection.".to_owned())?;
    if !entitlements.status().is_success() {
        return Err(format!("Minecraft ownership verification failed with HTTP {}.", entitlements.status().as_u16()));
    }
    let entitlements = response_json(entitlements, "Minecraft returned an invalid ownership response.")?;
    let owns_java = entitlements
        .get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|item| item.get("name").and_then(Value::as_str) == Some("game_minecraft")));
    if !owns_java {
        return Err("This Microsoft account does not have a verified Minecraft Java Edition entitlement.".to_owned());
    }

    let profile = client
        .get(PROFILE_ENDPOINT)
        .bearer_auth(&minecraft_access)
        .send()
        .map_err(|_| "Could not load the Minecraft profile. Check your network connection.".to_owned())?;
    if profile.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("This Microsoft account has no Minecraft Java Edition profile.".to_owned());
    }
    if !profile.status().is_success() {
        return Err(format!("Minecraft profile verification failed with HTTP {}.", profile.status().as_u16()));
    }
    let profile = response_json(profile, "Minecraft returned an invalid profile response.")?;
    let name = string_field(&profile, "name", "Minecraft profile")?.to_owned();
    let uuid = Uuid::parse_str(string_field(&profile, "id", "Minecraft profile")?)
        .map_err(|_| "Minecraft returned an invalid profile ID.".to_owned())?
        .to_string();
    let id = format!("microsoft:{uuid}");
    let account = MicrosoftAccount {
        id,
        name: name.clone(),
        uuid: uuid.clone(),
        created_at: crate::database::unix_time(),
    };
    let launch_profile = LaunchProfile {
        name,
        uuid,
        access_token: minecraft_access,
        user_type: "msa".to_owned(),
        xuid,
        verified: true,
    };
    Ok(MinecraftSession {
        account,
        profile: launch_profile,
        refresh_token,
    })
}

fn post_json(client: &Client, url: &str, body: Value, xbox_header: bool) -> Result<Value, String> {
    let body = serde_json::to_vec(&body)
        .map_err(|_| "Could not prepare an authentication request.".to_owned())?;
    let mut request = client.post(url).header("content-type", "application/json").body(body);
    if xbox_header {
        request = request.header("x-xbl-contract-version", "1");
    }
    let response = request
        .send()
        .map_err(|_| "Could not contact an authentication service. Check your network connection.".to_owned())?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response_json(response, "").unwrap_or(Value::Null);
        return Err(xsts_error(&format!("HTTP {}", status.as_u16()), body.get("XErr").and_then(Value::as_u64)));
    }
    response_json(response, "An authentication service returned an invalid response.")
}

fn form_body(fields: &[(&str, &str)]) -> String {
    let mut url = Url::parse("http://localhost/").expect("fixed form URL is valid");
    url.query_pairs_mut().extend_pairs(fields.iter().copied());
    url.query().unwrap_or_default().to_owned()
}

fn response_json(response: reqwest::blocking::Response, error: &str) -> Result<Value, String> {
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|_| if error.is_empty() { "Authentication service response could not be read.".to_owned() } else { error.to_owned() })?;
    serde_json::from_slice(&bytes)
        .map_err(|_| if error.is_empty() { "Authentication service returned invalid JSON.".to_owned() } else { error.to_owned() })
}

fn xsts_error(generic: &str, error_code: Option<u64>) -> String {
    match error_code {
        Some(2_148_916_233) => "This Microsoft account needs an Xbox profile before it can play Minecraft Java Edition.".to_owned(),
        Some(2_148_916_235) => "Xbox services are not available for this account's region.".to_owned(),
        Some(2_148_916_238) => "This child account needs its family organizer to enable Xbox access.".to_owned(),
        _ if generic.starts_with("HTTP") => format!("Xbox authentication failed ({generic}). Check the Microsoft account and try again."),
        _ => generic.to_owned(),
    }
}

fn string_field<'a>(value: &'a Value, field: &str, service: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{service} returned no {field}."))
}

#[cfg(target_os = "linux")]
fn secret_tool(args: &[&str], secret: Option<&str>) -> Result<Option<String>, String> {
    let mut command = Command::new("secret-tool");
    command.args(args).stdin(if secret.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|_| "The desktop credential store is unavailable. Unlock Secret Service and install secret-tool to use Microsoft sign-in.".to_owned())?;
    if let Some(secret) = secret {
        child.stdin.take().ok_or_else(|| "Could not write to the desktop credential store.".to_owned())?
            .write_all(secret.as_bytes())
            .map_err(|_| "Could not save the Microsoft session in the desktop credential store.".to_owned())?;
    }
    let output = child.wait_with_output().map_err(|_| "Could not read the desktop credential store.".to_owned())?;
    if !output.status.success() {
        return Err("The desktop credential store did not complete the request. Unlock it and retry.".to_owned());
    }
    Ok(secret.is_none().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned()))
}

fn store_refresh_token(account_id: &str, refresh_token: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let account = account_id.to_owned();
        let _ = secret_tool(&["store", "--label=OpnLauncher Microsoft session", "service", "org.opnlauncher.launcher", "account", &account], Some(refresh_token))?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (account_id, refresh_token);
        Err("Secure Microsoft token storage is not yet available on this platform.".to_owned())
    }
}

fn read_refresh_token(account_id: &str) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let account = account_id.to_owned();
        secret_tool(&["lookup", "service", "org.opnlauncher.launcher", "account", &account], None)?
            .filter(|token| !token.is_empty())
            .ok_or_else(|| "The saved Microsoft session is missing. Sign in again.".to_owned())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = account_id;
        Err("Secure Microsoft token storage is not yet available on this platform.".to_owned())
    }
}

fn clear_refresh_token(account_id: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let account = account_id.to_owned();
        let _ = secret_tool(&["clear", "service", "org.opnlauncher.launcher", "account", &account], None)?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = account_id;
        // Sign-in is disabled on these platforms, so there can be no launcher-managed
        // credential to remove. Still allow users to delete migrated local metadata.
        Ok(())
    }
}

fn ensure_token_storage_supported() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return Ok(());
    #[cfg(not(target_os = "linux"))]
    Err("Microsoft sign-in needs secure credential storage. OpnLauncher currently supports this flow on Linux only; Windows and macOS support is not ready yet.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{authorization_url, parse_callback_target, pkce_challenge, validate_client_id};

    #[test]
    fn pkce_challenge_is_url_safe_and_non_empty() {
        let challenge = pkce_challenge("test-verifier");
        assert!(!challenge.is_empty());
        assert!(!challenge.contains('='));
        assert!(challenge.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'));
    }

    #[test]
    fn authorization_url_uses_system_browser_code_flow_and_pkce() {
        let url = authorization_url(
            "123e4567-e89b-12d3-a456-426614174000",
            "http://localhost:12345",
            "state-token",
            "challenge-token",
        ).unwrap();
        let query = url.query_pairs().collect::<std::collections::HashMap<_, _>>();
        assert_eq!(query.get("response_type").map(|value| value.as_ref()), Some("code"));
        assert_eq!(query.get("code_challenge_method").map(|value| value.as_ref()), Some("S256"));
        assert_eq!(query.get("state").map(|value| value.as_ref()), Some("state-token"));
        assert_eq!(query.get("prompt").map(|value| value.as_ref()), Some("select_account"));
    }

    #[test]
    fn callback_requires_the_matching_state_and_hides_provider_error_details() {
        assert_eq!(
            parse_callback_target("/?code=auth-code&state=expected", "expected").unwrap(),
            "auth-code"
        );
        assert!(parse_callback_target("/?code=auth-code&state=wrong", "expected").is_err());
        assert_eq!(
            parse_callback_target("/?error=access_denied&state=expected", "expected").unwrap_err(),
            "Microsoft sign-in was cancelled."
        );
    }

    #[test]
    fn client_id_must_be_a_guid() {
        assert!(validate_client_id("123e4567-e89b-12d3-a456-426614174000").is_ok());
        assert!(validate_client_id("not-an-id").is_err());
    }
}
