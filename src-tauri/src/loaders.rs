use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use regex::Regex;
use reqwest::blocking::{Client, Response};
use reqwest::redirect::Policy;
use serde::Serialize;
use serde_json::Value;
use sha1::{Digest, Sha1};

use crate::error::{LauncherError, Result};
use crate::java;
use crate::minecraft::is_safe_id;

const FABRIC_META: &str = "https://meta.fabricmc.net";
const FABRIC_MAVEN: &str = "https://maven.fabricmc.net/";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
const MAX_METADATA_BYTES: usize = 12 * 1024 * 1024;
const MAX_INSTALLER_BYTES: usize = 128 * 1024 * 1024;
const MAX_LIBRARY_BYTES: usize = 512 * 1024 * 1024;
const MAX_LIBRARY_COUNT: usize = 2_000;

struct RemoveFileOnDrop(PathBuf);

impl Drop for RemoveFileOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
    pub recommended: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledLoaderProfile {
    pub version_id: String,
    pub minecraft_version: String,
    pub loader: String,
    pub loader_version: String,
}

pub fn fabric_versions(game_version: &str) -> Result<Vec<LoaderVersion>> {
    validate_game_version(game_version)?;
    let url = format!("{FABRIC_META}/v2/versions/loader/{}", encode_path_segment(game_version));
    let client = build_client()?;
    let response = client
        .get(url)
        .send()
        .and_then(Response::error_for_status)
        .map_err(network_error)?;
    let data: Value = serde_json::from_slice(&read_limited(response, MAX_METADATA_BYTES)?)?;
    let entries = data
        .as_array()
        .ok_or_else(|| LauncherError::Unsupported("Fabric returned an unexpected version list".to_owned()))?;
    if entries.len() > 2_000 {
        return Err(LauncherError::Unsupported("Fabric returned too many loader versions".to_owned()));
    }
    let versions = entries
        .iter()
        .filter_map(|entry| {
            let loader = entry.get("loader")?;
            let version = loader.get("version")?.as_str()?.to_owned();
            if !is_loader_version(&version) {
                return None;
            }
            Some(LoaderVersion {
                version,
                stable: loader.get("stable").and_then(Value::as_bool).unwrap_or(false),
                recommended: false,
            })
        })
        .collect::<Vec<_>>();
    if versions.is_empty() {
        return Err(LauncherError::NotFound("Fabric did not return a compatible loader version".to_owned()));
    }
    Ok(versions)
}

pub fn forge_versions(game_version: &str) -> Result<Vec<LoaderVersion>> {
    validate_game_version(game_version)?;
    let url = format!("{FORGE_MAVEN}/net/minecraftforge/forge/maven-metadata.xml");
    let client = build_client()?;
    let response = client
        .get(url)
        .send()
        .and_then(Response::error_for_status)
        .map_err(network_error)?;
    let body = String::from_utf8(read_limited(response, MAX_METADATA_BYTES)?)
        .map_err(|_| LauncherError::Unsupported("Forge returned invalid version metadata".to_owned()))?;
    let pattern = Regex::new(r"<version>\s*([^<]+?)\s*</version>")
        .map_err(|error| LauncherError::Unsupported(error.to_string()))?;
    let recommended_version = client
        .get(format!("{FORGE_MAVEN}/net/minecraftforge/forge/promotions_slim.json"))
        .send()
        .and_then(Response::error_for_status)
        .ok()
        .and_then(|response| read_limited(response, MAX_METADATA_BYTES).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|metadata| metadata.pointer(&format!("/promos/{game_version}-recommended"))?.as_str().map(str::to_owned));
    let mut versions = Vec::new();
    for capture in pattern.captures_iter(&body) {
        let Some(value) = capture.get(1).map(|entry| entry.as_str()) else { continue };
        let Some(loader_version) = value.strip_prefix(&format!("{game_version}-")) else { continue };
        if !is_loader_version(loader_version) || loader_version.ends_with("-SNAPSHOT") {
            continue;
        }
        versions.push(LoaderVersion {
            version: loader_version.to_owned(),
            stable: true,
            recommended: recommended_version.as_deref() == Some(loader_version),
        });
    }
    versions.sort_by(|left, right| right.recommended.cmp(&left.recommended).then_with(|| compare_numeric_versions(&right.version, &left.version)));
    versions.dedup_by(|left, right| left.version == right.version);
    if versions.is_empty() {
        return Err(LauncherError::NotFound(format!("Forge versions for Minecraft {game_version}")));
    }
    Ok(versions)
}

pub fn installed_profiles(game_dir: &Path) -> Result<Vec<InstalledLoaderProfile>> {
    let root = canonical_game_dir(game_dir)?;
    let versions_path = root.join("versions");
    if fs::symlink_metadata(&versions_path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(LauncherError::Unsupported("the versions folder is a symbolic link".to_owned()));
    }
    let mut profiles = Vec::new();
    let Ok(entries) = fs::read_dir(&versions_path) else { return Ok(profiles) };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let version_id = entry.file_name().to_string_lossy().into_owned();
        if !is_safe_id(&version_id) {
            continue;
        }
        let json_path = entry.path().join(format!("{version_id}.json"));
        let Ok(metadata) = fs::symlink_metadata(&json_path) else { continue };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        let Ok(bytes) = fs::read(&json_path) else { continue };
        let Ok(json) = serde_json::from_slice::<Value>(&bytes) else { continue };
        if json.get("id").and_then(Value::as_str) != Some(&version_id) {
            continue;
        }
        let Some(game_version) = json.get("inheritsFrom").and_then(Value::as_str) else { continue };
        let Some((loader, loader_version)) = detect_loader(&json, &version_id) else { continue };
        profiles.push(InstalledLoaderProfile {
            version_id,
            minecraft_version: game_version.to_owned(),
            loader,
            loader_version,
        });
    }
    profiles.sort_by(|left, right| {
        left.minecraft_version.cmp(&right.minecraft_version)
            .then_with(|| left.loader.cmp(&right.loader))
            .then_with(|| compare_numeric_versions(&right.loader_version, &left.loader_version))
    });
    Ok(profiles)
}

pub fn base_game_version(game_dir: &Path, version_id: &str) -> Result<String> {
    if !is_safe_id(version_id) {
        return Err(LauncherError::InvalidInput("instance version identifier is invalid".to_owned()));
    }
    let root = canonical_game_dir(game_dir)?;
    let metadata_path = root.join("versions").join(version_id).join(format!("{version_id}.json"));
    let metadata: Value = serde_json::from_slice(&fs::read(&metadata_path).map_err(|_| LauncherError::NotFound("instance version metadata".to_owned()))?)?;
    if metadata.get("id").and_then(Value::as_str) != Some(version_id) {
        return Err(LauncherError::Unsupported("instance version metadata does not match its folder".to_owned()));
    }
    let game_version = metadata.get("inheritsFrom").and_then(Value::as_str).unwrap_or(version_id);
    validate_game_version(game_version)?;
    Ok(game_version.to_owned())
}

pub fn install_fabric(game_dir: &Path, game_version: &str, loader_version: &str) -> Result<String> {
    validate_game_version(game_version)?;
    validate_loader_version(loader_version)?;
    let available = fabric_versions(game_version)?;
    if !available.iter().any(|entry| entry.version == loader_version) {
        return Err(LauncherError::InvalidInput("the selected Fabric version is not available for this Minecraft version".to_owned()));
    }
    let root = canonical_game_dir(game_dir)?;
    validate_base_version(&root, game_version)?;
    let profile_url = format!(
        "{FABRIC_META}/v2/versions/loader/{}/{loader_version}/profile/json",
        encode_path_segment(game_version)
    );
    let client = build_client()?;
    let response = client.get(profile_url).send().and_then(Response::error_for_status).map_err(network_error)?;
    let mut profile: Value = serde_json::from_slice(&read_limited(response, MAX_METADATA_BYTES)?)?;
    let version_id = profile.get("id").and_then(Value::as_str)
        .ok_or_else(|| LauncherError::Unsupported("Fabric profile has no version identifier".to_owned()))?.to_owned();
    if !is_safe_id(&version_id)
        || profile.get("inheritsFrom").and_then(Value::as_str) != Some(game_version)
    {
        return Err(LauncherError::Unsupported("Fabric returned a profile for a different Minecraft version".to_owned()));
    }
    let libraries = profile.get("libraries").and_then(Value::as_array)
        .ok_or_else(|| LauncherError::Unsupported("Fabric profile has no library list".to_owned()))?;
    if libraries.is_empty() || libraries.len() > MAX_LIBRARY_COUNT {
        return Err(LauncherError::Unsupported("Fabric profile has an invalid library count".to_owned()));
    }
    for library in libraries {
        let artifact = fabric_library_artifact(library)?;
        install_fabric_library(&client, &root, &artifact)?;
    }
    let object = profile.as_object_mut().ok_or_else(|| LauncherError::Unsupported("Fabric profile is not an object".to_owned()))?;
    object.insert("jar".to_owned(), Value::String(game_version.to_owned()));
    install_profile_metadata(&root, &version_id, &profile, game_version, loader_version)?;
    Ok(version_id)
}

pub fn install_forge(game_dir: &Path, game_version: &str, loader_version: &str, java_path: &Path) -> Result<String> {
    validate_game_version(game_version)?;
    validate_loader_version(loader_version)?;
    java::inspect_runtime(java_path)?;
    let available = forge_versions(game_version)?;
    if !available.iter().any(|entry| entry.version == loader_version) {
        return Err(LauncherError::InvalidInput("the selected Forge version is not available for this Minecraft version".to_owned()));
    }
    let root = canonical_game_dir(game_dir)?;
    reject_symlink_tree(&root.join("versions"))?;
    reject_symlink_tree(&root.join("libraries"))?;
    validate_base_version(&root, game_version)?;
    let forge_coordinate = format!("{game_version}-{loader_version}");
    let artifact = format!("forge-{forge_coordinate}-installer.jar");
    let url = format!("{FORGE_MAVEN}/net/minecraftforge/forge/{forge_coordinate}/{artifact}");
    let client = build_client()?;
    let expected_sha1 = client.get(format!("{url}.sha1"))
        .send().and_then(Response::error_for_status).map_err(network_error)?;
    let expected_sha1 = String::from_utf8(read_limited(expected_sha1, 256)?)
        .map_err(|_| LauncherError::Unsupported("Forge returned an invalid installer checksum".to_owned()))?;
    let expected_sha1 = expected_sha1.split_whitespace().next().unwrap_or_default().to_ascii_lowercase();
    if !is_sha1(&expected_sha1) {
        return Err(LauncherError::Unsupported("Forge returned an invalid installer checksum".to_owned()));
    }
    let response = client.get(url).send().and_then(Response::error_for_status).map_err(network_error)?;
    let installer = read_limited(response, MAX_INSTALLER_BYTES)?;
    if sha1_hex(&installer) != expected_sha1 {
        return Err(LauncherError::Unsupported("Forge installer checksum did not match".to_owned()));
    }

    let app_temp = root.join(".opnlauncher").join("installers");
    create_safe_directory(&root, &app_temp)?;
    let installer_path = app_temp.join(format!("forge-{forge_coordinate}-{}.jar", uuid::Uuid::new_v4()));
    write_new_file(&installer_path, &installer)?;
    let _installer_cleanup = RemoveFileOnDrop(installer_path.clone());

    let profiles_path = root.join("launcher_profiles.json");
    let original_profiles = match fs::symlink_metadata(&profiles_path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            let _ = fs::remove_file(&installer_path);
            return Err(LauncherError::Unsupported("launcher_profiles.json is not a regular file".to_owned()));
        }
        Ok(_) => Some(fs::read(&profiles_path)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(bytes) = original_profiles.as_deref() {
        let profile_json: Value = serde_json::from_slice(bytes).map_err(|_| LauncherError::Unsupported("launcher_profiles.json is invalid; Forge installation was not started".to_owned()))?;
        if !profile_json.is_object() {
            let _ = fs::remove_file(&installer_path);
            return Err(LauncherError::Unsupported("launcher_profiles.json must contain an object".to_owned()));
        }
    } else {
        if let Err(error) = write_new_file(&profiles_path, br#"{"profiles":{}}"#) {
            let _ = fs::remove_file(&installer_path);
            return Err(error);
        }
    }

    let process_result = Command::new(java_path)
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installClient")
        .arg(&root)
        .current_dir(&root)
        .output();
    let restore_result = match original_profiles {
        Some(bytes) => replace_file(&profiles_path, &bytes),
        None => match fs::remove_file(&profiles_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        },
    };
    let _ = fs::remove_file(&installer_path);
    if let Ok(entries) = fs::read_dir(&app_temp) {
        if entries.count() == 0 {
            let _ = fs::remove_dir(&app_temp);
            let _ = fs::remove_dir(app_temp.parent().unwrap_or(&root));
        }
    }
    restore_result?;

    let output = process_result.map_err(|error| LauncherError::Unsupported(format!("could not start the Forge installer: {error}")))?;
    if !output.status.success() {
        let mut details = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if details.is_empty() {
            details = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        }
        details.truncate(1_200);
        return Err(LauncherError::Unsupported(if details.is_empty() {
            format!("Forge installer exited with {}. Any files it wrote remain in this game folder; the selected version was not changed.", output.status)
        } else {
            format!("Forge installer failed: {details}. Any files it wrote remain in this game folder; the selected version was not changed.")
        }));
    }
    let installed_profile = installed_profiles(&root)?.into_iter()
        .find(|entry| entry.minecraft_version == game_version && entry.loader == "forge" && entry.loader_version == loader_version)
        .ok_or_else(|| LauncherError::Unsupported("Forge finished, but its installed profile could not be identified. The added files were left in the instance folder.".to_owned()))?;
    normalize_inherited_loader_jar(&root, &installed_profile.version_id, game_version, "forge", loader_version)?;
    Ok(installed_profile.version_id)
}

fn normalize_inherited_loader_jar(
    root: &Path,
    version_id: &str,
    game_version: &str,
    loader: &str,
    loader_version: &str,
) -> Result<()> {
    if !is_safe_id(version_id) {
        return Err(LauncherError::Unsupported("loader profile has an unsafe version identifier".to_owned()));
    }
    let versions_dir = root.join("versions");
    let profile_dir = versions_dir.join(version_id);
    let metadata_path = profile_dir.join(format!("{version_id}.json"));
    for (path, expected_directory) in [(&versions_dir, true), (&profile_dir, true), (&metadata_path, false)] {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            LauncherError::Unsupported(format!("loader profile metadata is unavailable: {error}"))
        })?;
        if metadata.file_type().is_symlink()
            || (expected_directory && !metadata.is_dir())
            || (!expected_directory && !metadata.is_file())
        {
            return Err(LauncherError::Unsupported("loader profile metadata contains an unsafe path".to_owned()));
        }
    }
    let bytes = fs::read(&metadata_path)?;
    let mut profile: Value = serde_json::from_slice(&bytes)?;
    normalize_inherited_loader_profile(&mut profile, version_id, game_version, loader, loader_version)?;
    replace_file(&metadata_path, &serde_json::to_vec_pretty(&profile)?)
}

fn normalize_inherited_loader_profile(
    profile: &mut Value,
    version_id: &str,
    game_version: &str,
    loader: &str,
    loader_version: &str,
) -> Result<()> {
    let loader_matches = detect_loader(profile, version_id)
        .is_some_and(|(found_loader, found_version)| found_loader == loader && found_version == loader_version);
    if profile.get("id").and_then(Value::as_str) != Some(version_id)
        || profile.get("inheritsFrom").and_then(Value::as_str) != Some(game_version)
        || !loader_matches
    {
        return Err(LauncherError::Unsupported("installed loader profile does not match the selected game and loader versions".to_owned()));
    }
    let object = profile.as_object_mut().ok_or_else(|| LauncherError::Unsupported("installed loader profile is not an object".to_owned()))?;
    object.insert("jar".to_owned(), Value::String(game_version.to_owned()));
    Ok(())
}

struct FabricLibraryArtifact {
    path: String,
    url: reqwest::Url,
    sha1: Option<String>,
    size: Option<u64>,
}

fn fabric_library_artifact(library: &Value) -> Result<FabricLibraryArtifact> {
    let artifact = library.get("downloads").and_then(|value| value.get("artifact"));
    let path = artifact
        .and_then(|value| value.get("path"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| library.get("name").and_then(Value::as_str).and_then(|name| crate::launch::maven_path(name).ok()).map(|path| path.to_string_lossy().into_owned()))
        .ok_or_else(|| LauncherError::Unsupported("Fabric library has no safe Maven artifact path".to_owned()))?;
    if !is_safe_relative_path(&path) {
        return Err(LauncherError::Unsupported("Fabric library has an unsafe artifact path".to_owned()));
    }
    let repository = library.get("url").and_then(Value::as_str).unwrap_or(FABRIC_MAVEN);
    let repository_url = trusted_fabric_url(repository)?;
    let url = match artifact.and_then(|value| value.get("url")).and_then(Value::as_str) {
        Some(url) => trusted_fabric_url(url)?,
        None => trusted_fabric_url(repository_url.join(&path).map_err(|_| LauncherError::Unsupported("Fabric library URL could not be constructed".to_owned()))?.as_str())?,
    };
    let sha1 = artifact.and_then(|value| value.get("sha1")).and_then(Value::as_str).map(|value| value.to_ascii_lowercase());
    if sha1.as_deref().is_some_and(|value| !is_sha1(value)) {
        return Err(LauncherError::Unsupported("Fabric library has an invalid SHA-1 checksum".to_owned()));
    }
    let size = artifact.and_then(|value| value.get("size")).and_then(Value::as_u64);
    if size.is_some_and(|size| size > MAX_LIBRARY_BYTES as u64) {
        return Err(LauncherError::Unsupported("Fabric library exceeds the download size limit".to_owned()));
    }
    Ok(FabricLibraryArtifact { path, url, sha1, size })
}

fn trusted_fabric_url(value: &str) -> Result<reqwest::Url> {
    let parsed = reqwest::Url::parse(value).map_err(|_| LauncherError::Unsupported("Fabric library URL is invalid".to_owned()))?;
    if parsed.scheme() != "https"
        || !matches!(parsed.host_str(), Some("maven.fabricmc.net" | "libraries.minecraft.net"))
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(LauncherError::Unsupported("Fabric library points to an untrusted download host".to_owned()));
    }
    Ok(parsed)
}

fn install_fabric_library(client: &Client, root: &Path, artifact: &FabricLibraryArtifact) -> Result<()> {
    let destination = safe_destination(root, &format!("libraries/{}", artifact.path))?;
    let expected_sha1 = match artifact.sha1.as_deref() {
        Some(sha1) => sha1.to_owned(),
        None => fetch_sha1_sidecar(client, &artifact.url)?,
    };
    if let Ok(metadata) = fs::symlink_metadata(&destination) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(LauncherError::Unsupported(format!("Fabric library destination {} is not a regular file", destination.display())));
        }
        if metadata.len() <= MAX_LIBRARY_BYTES as u64
            && artifact.size.is_none_or(|size| metadata.len() == size)
            && sha1_hex(&fs::read(&destination)?) == expected_sha1
        {
            return Ok(());
        }
        return Err(LauncherError::Unsupported(format!("existing library {} has a different checksum; it was left unchanged", artifact.path)));
    }
    let response = client.get(artifact.url.clone()).send().and_then(Response::error_for_status).map_err(network_error)?;
    let bytes = read_limited(response, MAX_LIBRARY_BYTES)?;
    if artifact.size.is_some_and(|size| bytes.len() as u64 != size) || sha1_hex(&bytes) != expected_sha1 {
        return Err(LauncherError::Unsupported(format!("Fabric library {} failed its integrity check", artifact.path)));
    }
    write_new_file(&destination, &bytes)
}

fn fetch_sha1_sidecar(client: &Client, artifact_url: &reqwest::Url) -> Result<String> {
    let checksum_url = format!("{artifact_url}.sha1");
    let response = client.get(&checksum_url).send().and_then(Response::error_for_status).map_err(network_error)?;
    let body = String::from_utf8(read_limited(response, 256)?)
        .map_err(|_| LauncherError::Unsupported("Fabric Maven returned an invalid checksum".to_owned()))?;
    let checksum = body.split_whitespace().next().unwrap_or_default().to_ascii_lowercase();
    if !is_sha1(&checksum) {
        return Err(LauncherError::Unsupported("Fabric Maven returned an invalid checksum".to_owned()));
    }
    Ok(checksum)
}

fn install_profile_metadata(root: &Path, version_id: &str, profile: &Value, game_version: &str, loader_version: &str) -> Result<()> {
    let versions_dir = root.join("versions");
    create_safe_directory(root, &versions_dir)?;
    let profile_dir = versions_dir.join(version_id);
    if fs::symlink_metadata(&profile_dir).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(LauncherError::Unsupported("Fabric profile destination is a symbolic link".to_owned()));
    }
    fs::create_dir_all(&profile_dir)?;
    let metadata_path = profile_dir.join(format!("{version_id}.json"));
    let bytes = serde_json::to_vec_pretty(profile)?;
    if let Ok(metadata) = fs::symlink_metadata(&metadata_path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(LauncherError::Unsupported("Fabric profile destination is not a regular file".to_owned()));
        }
        let existing = fs::read(&metadata_path)?;
        let value: Value = serde_json::from_slice(&existing)?;
        if value.get("id").and_then(Value::as_str) == Some(version_id)
            && value.get("inheritsFrom").and_then(Value::as_str) == Some(game_version)
            && detect_loader(&value, version_id).is_some_and(|(loader, version)| loader == "fabric" && version == loader_version)
        {
            return Ok(());
        }
        return Err(LauncherError::Unsupported("a different version profile already uses the Fabric destination ID; it was left unchanged".to_owned()));
    }
    write_new_file(&metadata_path, &bytes)
}

fn validate_base_version(root: &Path, game_version: &str) -> Result<()> {
    let version_dir = root.join("versions").join(game_version);
    let version_dir_metadata = fs::symlink_metadata(&version_dir).map_err(|_| LauncherError::NotFound(format!("Minecraft {game_version} is not installed in this instance")))?;
    if version_dir_metadata.file_type().is_symlink() || !version_dir_metadata.is_dir() {
        return Err(LauncherError::Unsupported("Minecraft version folder is not a regular directory".to_owned()));
    }
    let metadata_path = version_dir.join(format!("{game_version}.json"));
    let metadata_file = fs::symlink_metadata(&metadata_path).map_err(|_| LauncherError::NotFound(format!("Minecraft {game_version} metadata is missing")))?;
    if metadata_file.file_type().is_symlink() || !metadata_file.is_file() {
        return Err(LauncherError::Unsupported("Minecraft version metadata is not a regular file".to_owned()));
    }
    let bytes = fs::read(&metadata_path)?;
    let metadata: Value = serde_json::from_slice(&bytes)?;
    if metadata.get("id").and_then(Value::as_str) != Some(game_version) {
        return Err(LauncherError::Unsupported("Minecraft version metadata does not match its folder".to_owned()));
    }
    let jar_id = metadata.get("jar").and_then(Value::as_str).unwrap_or(game_version);
    if !is_safe_id(jar_id) {
        return Err(LauncherError::Unsupported("Minecraft version metadata contains an unsafe client JAR identifier".to_owned()));
    }
    let jar_path = root.join("versions").join(jar_id).join(format!("{jar_id}.jar"));
    let jar_metadata = fs::symlink_metadata(&jar_path);
    if !jar_metadata.is_ok_and(|metadata| !metadata.file_type().is_symlink() && metadata.is_file()) {
        return Err(LauncherError::NotFound(format!("the Minecraft {game_version} client JAR is missing")));
    }
    Ok(())
}

fn detect_loader(metadata: &Value, version_id: &str) -> Option<(String, String)> {
    let libraries = metadata.get("libraries").and_then(Value::as_array).into_iter().flatten();
    for library in libraries {
        let name = library.get("name").and_then(Value::as_str).unwrap_or_default();
        let (loader, version) = if let Some(rest) = name.strip_prefix("net.fabricmc:fabric-loader:") {
            ("fabric", rest.split('@').next().unwrap_or(rest))
        } else if let Some(rest) = name.strip_prefix("net.minecraftforge:forge:") {
            ("forge", rest.split('@').next().unwrap_or(rest).split('-').last().unwrap_or(rest))
        } else if let Some(rest) = name.strip_prefix("net.neoforged:neoforge:") {
            ("neoforge", rest.split('@').next().unwrap_or(rest))
        } else if let Some(rest) = name.strip_prefix("org.quiltmc:quilt-loader:") {
            ("quilt", rest.split('@').next().unwrap_or(rest))
        } else {
            continue;
        };
        return Some((loader.to_owned(), version.to_owned()));
    }
    let lower_id = version_id.to_ascii_lowercase();
    for (loader, marker) in [("fabric", "fabric-loader-"), ("forge", "-forge-"), ("neoforge", "-neoforge-"), ("quilt", "quilt-loader-")] {
        if let Some(index) = lower_id.find(marker) {
            let version = &version_id[index + marker.len()..];
            let version = version.split('-').next().unwrap_or(version);
            if is_loader_version(version) {
                return Some((loader.to_owned(), version.to_owned()));
            }
        }
    }
    None
}

fn canonical_game_dir(game_dir: &Path) -> Result<PathBuf> {
    let root = fs::canonicalize(game_dir).map_err(|error| LauncherError::Unsupported(format!("game folder is unavailable: {error}")))?;
    if !root.is_dir() {
        return Err(LauncherError::InvalidInput("game folder must be a directory".to_owned()));
    }
    Ok(root)
}

fn safe_destination(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if !is_safe_relative_path(relative) {
        return Err(LauncherError::Unsupported("loader metadata contains an unsafe file path".to_owned()));
    }
    let destination = root.join(path);
    let mut current = root.to_path_buf();
    for component in path.parent().into_iter().flat_map(Path::components) {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(LauncherError::Unsupported("loader library path contains a symbolic link or non-folder".to_owned()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)?,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(destination)
}

fn is_safe_relative_path(relative: &str) -> bool {
    let path = Path::new(relative);
    !path.as_os_str().is_empty() && path.components().all(|part| matches!(part, Component::Normal(_)))
}

fn create_safe_directory(root: &Path, directory: &Path) -> Result<()> {
    let relative = directory.strip_prefix(root).map_err(|_| LauncherError::InvalidInput("loader path escaped the game folder".to_owned()))?;
    if relative.components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err(LauncherError::InvalidInput("loader path contains an invalid component".to_owned()));
    }
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => return Err(LauncherError::Unsupported("loader destination contains a symbolic link or non-folder".to_owned())),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)?,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn reject_symlink_tree(root: &Path) -> Result<()> {
    let Ok(metadata) = fs::symlink_metadata(root) else { return Ok(()) };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LauncherError::Unsupported(format!("{} must be a regular folder", root.display())));
    }
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut visited = 0_usize;
    while let Some((directory, depth)) = pending.pop() {
        if depth > 64 {
            return Err(LauncherError::Unsupported("Minecraft install folder is nested too deeply".to_owned()));
        }
        for entry in fs::read_dir(directory)? {
            visited += 1;
            if visited > 200_000 {
                return Err(LauncherError::Unsupported("Minecraft install folder has too many entries to validate safely".to_owned()));
            }
            let entry = entry?;
            let metadata = entry.file_type()?;
            if metadata.is_symlink() {
                return Err(LauncherError::Unsupported(format!("symbolic links are not supported in {}", root.display())));
            }
            if metadata.is_dir() {
                pending.push((entry.path(), depth + 1));
            }
        }
    }
    Ok(())
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

fn replace_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!("opn-{}.tmp", uuid::Uuid::new_v4()));
    write_new_file(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn build_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("OpnLauncher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(10 * 60))
        .redirect(Policy::none())
        .build()
        .map_err(|error| LauncherError::Unsupported(format!("could not initialize secure loader downloads: {error}")))
}

fn read_limited(response: Response, max_size: usize) -> Result<Vec<u8>> {
    if response.content_length().is_some_and(|length| length > max_size as u64) {
        return Err(LauncherError::Unsupported("loader download exceeds the size limit".to_owned()));
    }
    let mut bytes = Vec::new();
    response.take(max_size as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max_size {
        return Err(LauncherError::Unsupported("loader download exceeds the size limit".to_owned()));
    }
    Ok(bytes)
}

fn network_error(error: reqwest::Error) -> LauncherError {
    LauncherError::Unsupported(format!("loader metadata or download request failed: {error}"))
}

fn validate_game_version(version: &str) -> Result<()> {
    if !is_safe_id(version) || version.len() > 128 {
        return Err(LauncherError::InvalidInput("Minecraft version identifier is invalid".to_owned()));
    }
    Ok(())
}

fn validate_loader_version(version: &str) -> Result<()> {
    if !is_loader_version(version) {
        return Err(LauncherError::InvalidInput("loader version identifier is invalid".to_owned()));
    }
    Ok(())
}

fn is_loader_version(version: &str) -> bool {
    !version.is_empty() && version.len() <= 64
        && version.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'))
}

fn encode_path_segment(value: &str) -> String {
    let mut result = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            result.push(byte as char);
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}

fn is_sha1(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha1_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha1::digest(bytes))
}

fn compare_numeric_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let left_parts = left.split(['.', '-', '+']).collect::<Vec<_>>();
    let right_parts = right.split(['.', '-', '+']).collect::<Vec<_>>();
    for (left_part, right_part) in left_parts.iter().zip(&right_parts) {
        let order = match (left_part.parse::<u64>(), right_part.parse::<u64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            _ => left_part.cmp(right_part),
        };
        if order != std::cmp::Ordering::Equal {
            return order;
        }
    }
    left_parts.len().cmp(&right_parts.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_metadata_path_segments_without_changing_safe_characters() {
        assert_eq!(encode_path_segment("1.21.4"), "1.21.4");
        assert_eq!(encode_path_segment("20w14∞"), "20w14%E2%88%9E");
        assert_eq!(encode_path_segment("Release Candidate"), "Release%20Candidate");
    }

    #[test]
    fn detects_loader_profiles_from_library_coordinates() {
        let profile = serde_json::json!({"libraries": [{"name": "net.fabricmc:fabric-loader:0.16.10"}]});
        assert_eq!(detect_loader(&profile, "custom"), Some(("fabric".to_owned(), "0.16.10".to_owned())));
        let profile = serde_json::json!({"libraries": [{"name": "net.minecraftforge:forge:1.20.1-47.3.0"}]});
        assert_eq!(detect_loader(&profile, "custom"), Some(("forge".to_owned(), "47.3.0".to_owned())));
    }

    #[test]
    fn sorts_numeric_loader_versions_numerically() {
        assert_eq!(compare_numeric_versions("47.3.10", "47.3.9"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn validates_loader_version_and_safe_relative_paths() {
        assert!(validate_loader_version("0.16.10").is_ok());
        assert!(validate_loader_version("../escape").is_err());
        assert!(is_safe_relative_path("libraries/net/fabricmc/loader.jar"));
        assert!(!is_safe_relative_path("../../outside.jar"));
        assert!(!is_safe_relative_path("/outside.jar"));
    }

    #[test]
    fn supports_fabric_metadata_with_maven_coordinates_and_repository_url() {
        let library = serde_json::json!({
            "name": "net.fabricmc:fabric-loader:0.16.14",
            "url": "https://maven.fabricmc.net/"
        });
        let artifact = fabric_library_artifact(&library).unwrap();
        assert_eq!(artifact.path, "net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar");
        assert_eq!(artifact.url.as_str(), "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar");
        assert!(artifact.sha1.is_none());
        assert!(artifact.size.is_none());
    }

    #[test]
    fn supports_fabric_metadata_with_download_artifact_details() {
        let library = serde_json::json!({
            "name": "net.fabricmc:fabric-loader:0.16.14",
            "downloads": {
                "artifact": {
                    "path": "custom/fabric-loader.jar",
                    "url": "https://maven.fabricmc.net/custom/fabric-loader.jar",
                    "sha1": "0123456789012345678901234567890123456789",
                    "size": 123
                }
            }
        });
        let artifact = fabric_library_artifact(&library).unwrap();
        assert_eq!(artifact.path, "custom/fabric-loader.jar");
        assert_eq!(artifact.sha1.as_deref(), Some("0123456789012345678901234567890123456789"));
        assert_eq!(artifact.size, Some(123));
    }

    #[test]
    fn rejects_fabric_library_paths_and_repositories_outside_the_allowlist() {
        let library = serde_json::json!({
            "name": "example:library:1.0",
            "url": "https://example.com/maven/"
        });
        assert!(fabric_library_artifact(&library).is_err());
        let library = serde_json::json!({
            "name": "../../escape:library:1.0",
            "url": "https://maven.fabricmc.net/"
        });
        assert!(fabric_library_artifact(&library).is_err());
    }

    #[test]
    fn points_inherited_forge_profiles_at_the_installed_vanilla_jar() {
        let mut profile = serde_json::json!({
            "id": "1.20.1-forge-47.3.0",
            "inheritsFrom": "1.20.1",
            "libraries": [{"name": "net.minecraftforge:forge:1.20.1-47.3.0"}]
        });
        normalize_inherited_loader_profile(&mut profile, "1.20.1-forge-47.3.0", "1.20.1", "forge", "47.3.0").unwrap();
        assert_eq!(profile.get("jar").and_then(Value::as_str), Some("1.20.1"));
    }

    #[test]
    fn refuses_to_normalize_a_loader_profile_for_a_different_game_version() {
        let mut profile = serde_json::json!({
            "id": "1.20.1-forge-47.3.0",
            "inheritsFrom": "1.20.1",
            "libraries": [{"name": "net.minecraftforge:forge:1.20.1-47.3.0"}]
        });
        assert!(normalize_inherited_loader_profile(&mut profile, "1.20.1-forge-47.3.0", "1.19.4", "forge", "47.3.0").is_err());
        assert!(profile.get("jar").is_none());
    }
}
