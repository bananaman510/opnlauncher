use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use zip::ZipArchive;

use crate::error::{LauncherError, Result};
use crate::models::{Instance, InstanceEngine, OfflineProfile};

const HORIZON_GAME_VERSION: &str = "26.3";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_MOD_BYTES: u64 = 512 * 1024 * 1024;

pub const HORIZON_SOURCE_REVISION: &str = "fdd290b99b4459cb4803487758ba8015a45aa47f";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HorizonClientInfo {
    pub executable: String,
    pub version: String,
    pub minecraft_version: String,
    pub launch_api: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HorizonEngineStatus {
    pub client: HorizonClientInfo,
    pub resources_path: String,
    pub resources_ready: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HorizonMod {
    pub file_name: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub enabled: bool,
    pub size: u64,
    pub valid: bool,
}

pub fn inspect_client(path: &Path) -> Result<HorizonClientInfo> {
    let executable = valid_file(path, "Horizon client")?;
    let output = Command::new(&executable)
        .arg("--launch-api")
        .env("HORIZON_TELEMETRY_URL", "off")
        .output()
        .map_err(|error| {
            LauncherError::Unsupported(format!("could not check Horizon client: {error}"))
        })?;
    if !output.status.success() {
        return Err(LauncherError::Unsupported(format!(
            "Horizon client did not report its launch interface: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let reply: Value = serde_json::from_slice(&output.stdout)?;
    let launch_api = required_string(&reply, "launch_api")?;
    let version = required_string(&reply, "horizon")?;
    let minecraft_version = required_string(&reply, "minecraft")?;
    if !launch_api.starts_with("0.1.") {
        return Err(LauncherError::Unsupported(format!(
            "Horizon launch interface {launch_api} is not supported yet"
        )));
    }
    if minecraft_version != HORIZON_GAME_VERSION {
        return Err(LauncherError::Unsupported(format!(
            "this Horizon client targets Minecraft {minecraft_version}; OpnLauncher currently supports {HORIZON_GAME_VERSION}"
        )));
    }
    for option in [
        "--resources",
        "--game-dir",
        "--home",
        "--player-dir",
        "--width",
        "--height",
        "--fullscreen",
    ] {
        if !supports_option(&reply, option) {
            return Err(LauncherError::Unsupported(format!(
                "Horizon client does not support required option {option}"
            )));
        }
    }
    Ok(HorizonClientInfo {
        executable: executable.to_string_lossy().into_owned(),
        version: version.to_owned(),
        minecraft_version: minecraft_version.to_owned(),
        launch_api: launch_api.to_owned(),
    })
}

pub fn stage_preparer_inputs(source_root: &Path, target_root: &Path) -> Result<()> {
    let lock = source_root.join("research-lock.json");
    ensure_source_directory(target_root)?;
    copy_source_file(&lock, &target_root.join("research-lock.json"))?;
    let sources = source_root.join("mods/horizon/src/test/java/dev/horizonlab/parity");
    copy_source_tree(&sources, &target_root.join("mods/horizon/src/test/java/dev/horizonlab/parity"))
}

fn copy_source_tree(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.file_type().is_dir() {
        return Err(LauncherError::Unsupported(format!(
            "Horizon source data is not a regular folder: {}",
            source.display()
        )));
    }
    ensure_source_directory(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_source_tree(&source_path, &target_path)?;
        } else if file_type.is_file() {
            copy_source_file(&source_path, &target_path)?;
        } else {
            return Err(LauncherError::Unsupported(format!(
                "Horizon source data contains an unsupported file: {}",
                source_path.display()
            )));
        }
    }
    Ok(())
}

fn copy_source_file(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.file_type().is_file() {
        return Err(LauncherError::Unsupported(format!(
            "Horizon source data is not a regular file: {}",
            source.display()
        )));
    }
    if let Some(parent) = target.parent() {
        ensure_source_directory(parent)?;
    }
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_file() => {
            if fs::read(source)? != fs::read(target)? {
                return Err(LauncherError::Unsupported(format!(
                    "managed Horizon source data was modified: {}",
                    target.display()
                )));
            }
            return Ok(());
        }
        Ok(_) => {
            return Err(LauncherError::Unsupported(format!(
                "managed Horizon source path is not a regular file: {}",
                target.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::copy(source, target)?;
    Ok(())
}

fn ensure_source_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(LauncherError::Unsupported(format!(
            "managed Horizon source folder is not a regular directory: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent().filter(|parent| *parent != path) {
                ensure_source_directory(parent)?;
            }
            fs::create_dir(path)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn offline_command(
    instance: &Instance,
    profile: &OfflineProfile,
    client_path: &Path,
    resources_path: &Path,
    width: u32,
    height: u32,
    fullscreen: bool,
) -> Result<Command> {
    if instance.engine != InstanceEngine::Horizon {
        return Err(LauncherError::InvalidInput(
            "the selected instance is not a Horizon instance".to_owned(),
        ));
    }
    if instance.version_id.as_deref() != Some(HORIZON_GAME_VERSION) {
        return Err(LauncherError::Unsupported(
            "Horizon instances must use Minecraft 26.3".to_owned(),
        ));
    }
    if profile.verified {
        return Err(LauncherError::Unsupported(
            "OpnLauncher currently starts Horizon only with clearly unverified local profiles".to_owned(),
        ));
    }
    if !(640..=8192).contains(&width) || !(480..=8192).contains(&height) {
        return Err(LauncherError::InvalidInput(
            "Horizon window size must be between 640×480 and 8192×8192".to_owned(),
        ));
    }
    if !valid_offline_player_name(&profile.name) {
        return Err(LauncherError::InvalidInput(
            "this Horizon local profile needs a 1 to 16 character ASCII name using letters, numbers, or underscores".to_owned(),
        ));
    }

    let client = valid_file(client_path, "Horizon client")?;
    inspect_client(&client)?;
    let resources = fs::canonicalize(resources_path).map_err(|_| {
        LauncherError::NotFound(format!("prepared Horizon resources at {}", resources_path.display()))
    })?;
    if !resources.is_dir() {
        return Err(LauncherError::InvalidInput(
            "the Horizon resources path must be a folder".to_owned(),
        ));
    }

    let game_dir = fs::canonicalize(&instance.game_dir).map_err(|error| {
        LauncherError::Unsupported(format!("cannot open Horizon instance folder: {error}"))
    })?;
    let private_dir = game_dir.join(".opnlauncher");
    let player_dir = private_dir.join("players").join(&profile.id);
    let home_dir = private_dir.join("home");
    fs::create_dir_all(&player_dir)?;
    fs::create_dir_all(&home_dir)?;
    write_player_name(&player_dir, &profile.name)?;

    let mut command = Command::new(client);
    command
        .arg("--resources")
        .arg(resources)
        .arg("--game-dir")
        .arg(&game_dir)
        .arg("--home")
        .arg(&home_dir)
        .arg("--player-dir")
        .arg(&player_dir)
        .arg("--width")
        .arg(width.to_string())
        .arg("--height")
        .arg(height.to_string())
        .current_dir(&game_dir)
        .env("HORIZON_TELEMETRY_URL", "off")
        .stdin(Stdio::null());
    if fullscreen {
        command.arg("--fullscreen");
    }
    Ok(command)
}

pub fn list_mods(instance: &Instance) -> Result<Vec<HorizonMod>> {
    ensure_horizon(instance)?;
    let mods_dir = Path::new(&instance.game_dir).join("horizon-mods");
    if !mods_dir.exists() {
        return Ok(Vec::new());
    }
    if !mods_dir.is_dir() {
        return Err(LauncherError::Unsupported(
            "the Horizon mods location is not a folder".to_owned(),
        ));
    }
    let enabled_ids = enabled_mod_ids(Path::new(&instance.game_dir))?;
    let mut mods = Vec::new();
    for entry in fs::read_dir(&mods_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file()
            || !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("hmod"))
        {
            continue;
        }
        let metadata = fs::metadata(&path)?;
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let manifest = read_mod_manifest(&path).ok();
        let id = manifest.as_ref().map(|manifest| manifest.id.clone());
        mods.push(HorizonMod {
            file_name,
            enabled: id.as_ref().is_some_and(|id| enabled_ids.contains(id)),
            id,
            name: manifest.as_ref().map(|manifest| manifest.name.clone()),
            version: manifest.as_ref().map(|manifest| manifest.version.clone()),
            size: metadata.len(),
            valid: manifest.is_some(),
        });
    }
    mods.sort_by_key(|item| item.file_name.to_lowercase());
    Ok(mods)
}

pub fn install_mod(instance: &Instance, source: &Path) -> Result<HorizonMod> {
    ensure_horizon(instance)?;
    let source = valid_file(source, "Horizon content mod")?;
    let metadata = fs::metadata(&source)?;
    if metadata.len() == 0 || metadata.len() > MAX_MOD_BYTES {
        return Err(LauncherError::InvalidInput(
            "Horizon content mod must be a non-empty file under 512 MiB".to_owned(),
        ));
    }
    if !source.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("hmod")) {
        return Err(LauncherError::InvalidInput(
            "Horizon content mods must use the .hmod format; Java .jar mods do not work in Horizon".to_owned(),
        ));
    }
    let manifest = read_mod_manifest(&source)?;
    let mods_dir = Path::new(&instance.game_dir).join("horizon-mods");
    fs::create_dir_all(&mods_dir)?;
    if list_mods(instance)?.iter().any(|item| item.id.as_deref() == Some(&manifest.id)) {
        return Err(LauncherError::InvalidInput(format!(
            "Horizon mod {} is already installed in this instance",
            manifest.name
        )));
    }
    let source_name = source.file_name().ok_or_else(|| {
        LauncherError::InvalidInput("Horizon mod file has no name".to_owned())
    })?;
    let destination = unique_destination(&mods_dir, source_name)?;
    fs::copy(&source, &destination)?;
    Ok(HorizonMod {
        file_name: destination.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        id: Some(manifest.id),
        name: Some(manifest.name),
        version: Some(manifest.version),
        enabled: false,
        size: metadata.len(),
        valid: true,
    })
}

pub fn set_mod_enabled(instance: &Instance, file_name: &str, enabled: bool) -> Result<()> {
    ensure_horizon(instance)?;
    let path = mod_path(instance, file_name)?;
    let manifest = read_mod_manifest(&path)?;
    let config = Path::new(&instance.game_dir).join("horizon-mods.txt");
    let mut lines = fs::read_to_string(&config)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with(&format!("mod.{}:", manifest.id)))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if enabled {
        lines.push(format!("mod.{}:true", manifest.id));
    }
    write_lines_atomic(&config, &lines)
}

pub fn remove_mod(instance: &Instance, file_name: &str, confirmed: bool) -> Result<()> {
    ensure_horizon(instance)?;
    if !confirmed {
        return Err(LauncherError::InvalidInput(
            "confirm removal before deleting this Horizon mod".to_owned(),
        ));
    }
    let path = mod_path(instance, file_name)?;
    let manifest = read_mod_manifest(&path).ok();
    fs::remove_file(path)?;
    if let Some(manifest) = manifest {
        let config = Path::new(&instance.game_dir).join("horizon-mods.txt");
        let lines = fs::read_to_string(&config)
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.starts_with(&format!("mod.{}:", manifest.id)))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        write_lines_atomic(&config, &lines)?;
    }
    Ok(())
}

fn ensure_horizon(instance: &Instance) -> Result<()> {
    if instance.engine != InstanceEngine::Horizon {
        return Err(LauncherError::InvalidInput(
            "this operation is only available for Horizon instances".to_owned(),
        ));
    }
    Ok(())
}

fn valid_file(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|_| {
        LauncherError::NotFound(format!("{label} at {}", path.display()))
    })?;
    if !canonical.is_file() {
        return Err(LauncherError::InvalidInput(format!(
            "{label} path must be a file"
        )));
    }
    Ok(canonical)
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value.get(key).and_then(Value::as_str).ok_or_else(|| {
        LauncherError::Unsupported(format!("Horizon launch reply is missing {key}"))
    })
}

fn supports_option(reply: &Value, name: &str) -> bool {
    reply
        .get("options")
        .and_then(Value::as_array)
        .is_some_and(|options| {
            options.iter().any(|option| {
                option.get("name").and_then(Value::as_str) == Some(name)
            })
        })
}

fn valid_offline_player_name(name: &str) -> bool {
    (1..=16).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn write_player_name(player_dir: &Path, name: &str) -> Result<()> {
    let target = player_dir.join("horizon-player.txt");
    write_atomic(&target, format!("name:{name}\n").as_bytes())
}

fn enabled_mod_ids(game_dir: &Path) -> Result<std::collections::HashSet<String>> {
    let config = game_dir.join("horizon-mods.txt");
    let text = match fs::read_to_string(config) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(error) => return Err(error.into()),
    };
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix("mod."))
        .filter_map(|entry| entry.strip_suffix(":true"))
        .map(str::to_owned)
        .collect())
}

fn mod_path(instance: &Instance, file_name: &str) -> Result<PathBuf> {
    let name = Path::new(file_name);
    if name.file_name().and_then(|value| value.to_str()) != Some(file_name)
        || !name.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("hmod"))
        || file_name.chars().any(char::is_control)
    {
        return Err(LauncherError::InvalidInput(
            "Horizon mod file name is invalid".to_owned(),
        ));
    }
    let root = Path::new(&instance.game_dir).join("horizon-mods");
    let path = root.join(file_name);
    if !path.is_file() {
        return Err(LauncherError::NotFound(format!("Horizon mod {file_name}")));
    }
    Ok(path)
}

#[derive(Debug)]
struct ModManifest {
    id: String,
    name: String,
    version: String,
}

fn read_mod_manifest(path: &Path) -> Result<ModManifest> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        LauncherError::Unsupported(format!("Horizon content mod is not a valid archive: {error}"))
    })?;
    let mut entry = archive.by_name("horizon-mod.toml").map_err(|error| {
        LauncherError::Unsupported(format!("Horizon mod has no horizon-mod.toml manifest: {error}"))
    })?;
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(LauncherError::Unsupported(
            "Horizon mod manifest is larger than 1 MiB".to_owned(),
        ));
    }
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    let id = manifest_value(&text, "id")?;
    let name = manifest_value(&text, "name").unwrap_or_else(|_| id.clone());
    let version = manifest_value(&text, "version").unwrap_or_else(|_| "unknown".to_owned());
    if id.is_empty()
        || id.len() > 64
        || !id.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(LauncherError::Unsupported(
            "Horizon mod manifest has an invalid id".to_owned(),
        ));
    }
    Ok(ModManifest { id, name, version })
}

fn manifest_value(text: &str, key: &str) -> Result<String> {
    let prefix = format!("{key} =");
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&prefix).map(str::trim))
        .and_then(|value| value.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')))
        .map(str::to_owned)
        .ok_or_else(|| LauncherError::Unsupported(format!("Horizon mod manifest is missing {key}")))
}

fn unique_destination(dir: &Path, source_name: &std::ffi::OsStr) -> Result<PathBuf> {
    let source_name = Path::new(source_name);
    let stem = source_name.file_stem().and_then(|s| s.to_str()).unwrap_or("horizon-mod");
    let extension = source_name.extension().and_then(|s| s.to_str()).unwrap_or("hmod");
    for index in 0..1000 {
        let name = if index == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{index}.{extension}")
        };
        let path = dir.join(name);
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(LauncherError::Unsupported(
        "could not choose a unique filename for this Horizon mod".to_owned(),
    ))
}

fn write_lines_atomic(path: &Path, lines: &[String]) -> Result<()> {
    let content = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    write_atomic(path, content.as_bytes())
}

fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        LauncherError::InvalidInput("Horizon settings file has no parent folder".to_owned())
    })?;
    let temporary = parent.join(format!(".opnlauncher-tmp-{}", Uuid::new_v4()));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(content)?;
        file.sync_all()
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }

    let replacement = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            let backup = parent.join(format!(".opnlauncher-backup-{}", Uuid::new_v4()));
            if let Err(error) = fs::rename(path, &backup) {
                let _ = fs::remove_file(&temporary);
                return Err(error.into());
            }
            if let Err(error) = fs::rename(&temporary, path) {
                let _ = fs::rename(&backup, path);
                let _ = fs::remove_file(&temporary);
                return Err(error.into());
            }
            fs::remove_file(backup)
        }
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Horizon settings destination must be a regular file",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::rename(&temporary, path)
        }
        Err(error) => Err(error),
    };
    if replacement.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    replacement.map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{
        manifest_value, stage_preparer_inputs, valid_offline_player_name, write_lines_atomic,
    };
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn reads_basic_horizon_manifest_values() {
        let text = "id = \"test_mod\"\nname = \"A test mod\"\nversion = \"1.2.0\"\n";
        assert_eq!(manifest_value(text, "id").unwrap(), "test_mod");
        assert_eq!(manifest_value(text, "name").unwrap(), "A test mod");
    }

    #[test]
    fn validates_names_for_local_horizon_players() {
        assert!(valid_offline_player_name("Tester_2"));
        assert!(valid_offline_player_name("x"));
        assert!(!valid_offline_player_name("has space"));
        assert!(!valid_offline_player_name("WayTooLongForHorizon"));
    }

    #[test]
    fn atomically_replaces_horizon_mod_settings() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("horizon-settings-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let settings = root.join("horizon-mods.txt");
        fs::write(&settings, "mod.first:true\n").unwrap();
        write_lines_atomic(&settings, &["mod.second:true".to_owned()]).unwrap();
        assert_eq!(fs::read_to_string(&settings).unwrap(), "mod.second:true\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stages_horizon_preparer_sources_without_overwriting_changes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("horizon-source-test-{}", Uuid::new_v4()));
        let source = root.join("upstream");
        let target = root.join("managed");
        let parity = source.join("mods/horizon/src/test/java/dev/horizonlab/parity");
        fs::create_dir_all(&parity).unwrap();
        fs::write(source.join("research-lock.json"), b"source lock").unwrap();
        fs::write(parity.join("Oracle.java"), b"class Oracle {}\n").unwrap();

        stage_preparer_inputs(&source, &target).unwrap();
        assert_eq!(fs::read(target.join("research-lock.json")).unwrap(), b"source lock");
        assert_eq!(
            fs::read(target.join("mods/horizon/src/test/java/dev/horizonlab/parity/Oracle.java")).unwrap(),
            b"class Oracle {}\n"
        );
        stage_preparer_inputs(&source, &target).unwrap();

        fs::write(target.join("research-lock.json"), b"local change").unwrap();
        assert!(stage_preparer_inputs(&source, &target).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinks_in_horizon_preparer_sources() {
        use std::os::unix::fs::symlink;

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("horizon-source-link-test-{}", Uuid::new_v4()));
        let source = root.join("upstream");
        let parity = source.join("mods/horizon/src/test/java/dev/horizonlab/parity");
        fs::create_dir_all(&parity).unwrap();
        fs::write(source.join("research-lock.json"), b"source lock").unwrap();
        fs::write(root.join("outside.java"), b"outside").unwrap();
        symlink(root.join("outside.java"), parity.join("Oracle.java")).unwrap();

        assert!(stage_preparer_inputs(&source, &root.join("managed")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
