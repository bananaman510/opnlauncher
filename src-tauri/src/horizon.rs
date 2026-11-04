use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use uuid::Uuid;
use zip::ZipArchive;

use crate::error::{LauncherError, Result};
use crate::models::{Instance, InstanceEngine, OfflineProfile};

const HORIZON_GAME_VERSION: &str = "26.3";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_MOD_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FABRIC_CONTENT_FILES: usize = 65_536;
const MAX_FABRIC_CONTENT_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FABRIC_CONTENT_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_FABRIC_CODE_CLASS_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FABRIC_CODE_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
const MAX_FABRIC_CODE_CLASSES: usize = 8192;
const MAX_FABRIC_CODE_MOD_FILES: usize = 128;
const MAX_FABRIC_CODE_MOD_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const FABRIC_BRIDGE_CONFIG: &str = "horizon-fabric-bridge.json";
const FABRIC_BRIDGE_LEDGER: &str = "horizon-fabric-adapted.json";
const FABRIC_CODE_BRIDGE_DIR: &str = "fabric-code-bridge";
const FABRIC_CODE_STAGE_LEDGER: &str = "fabric-code-stage.json";

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

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct HorizonFabricBridgeSettings {
    pub enabled: bool,
    pub code_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HorizonFabricModStatus {
    pub file_name: String,
    pub mod_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub state: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HorizonFabricCodeModStatus {
    pub file_name: String,
    pub mod_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub state: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HorizonFabricBridgeStatus {
    pub settings: HorizonFabricBridgeSettings,
    pub compatible_count: usize,
    pub skipped_count: usize,
    pub mods: Vec<HorizonFabricModStatus>,
    pub code_compatible_count: usize,
    pub code_skipped_count: usize,
    pub code_mods: Vec<HorizonFabricCodeModStatus>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FabricBridgeLedger {
    mods: BTreeMap<String, AdaptedFabricMod>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdaptedFabricMod {
    source_sha1: String,
    package_sha1: String,
    mod_id: String,
    output_file: String,
}

struct FabricContentPackage {
    id: String,
    name: String,
    version: String,
    description: String,
    authors: Vec<String>,
    namespaces: Vec<String>,
    files: Vec<(String, Vec<u8>)>,
    source_sha1: String,
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
    for name in [
        "JAVA",
        "CLASS_PATH",
        "JAR",
        "MODS_DIR",
        "GAME_DIR",
        "GAME_JAR",
        "JVM_ARG_COUNT",
    ] {
        command.env_remove(format!("OPNLAUNCHER_FABRIC_BRIDGE_{name}"));
    }
    for index in 0..64 {
        command.env_remove(format!("OPNLAUNCHER_FABRIC_BRIDGE_JVM_ARG_{index}"));
    }
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

pub fn fabric_bridge_status(instance: &Instance) -> Result<HorizonFabricBridgeStatus> {
    ensure_horizon(instance)?;
    let settings = read_fabric_bridge_settings(instance)?;
    let mods_dir = Path::new(&instance.game_dir).join("mods");
    let mut mods = Vec::new();
    let mut code_mods = Vec::new();
    let mut code_mod_bytes = 0u64;
    let mut code_files_inspected = 0usize;
    match fs::symlink_metadata(&mods_dir) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            for entry in fs::read_dir(&mods_dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let file_name = entry.file_name().to_string_lossy().into_owned();
                let lower = file_name.to_ascii_lowercase();
                let disabled = lower.ends_with(".jar.disabled");
                if !disabled && !lower.ends_with(".jar") {
                    continue;
                }
                let path = entry.path();
                let mut status = match inspect_fabric_jar(&path) {
                    Ok(inspection) => inspection.status,
                    Err(error) => HorizonFabricModStatus {
                        file_name: file_name.clone(),
                        mod_id: None,
                        name: None,
                        version: None,
                        state: "invalid".to_owned(),
                        detail: error.to_string(),
                    },
                };
                status.file_name = file_name.clone();
                if disabled {
                    status.state = "disabled".to_owned();
                    status.detail = "This Java mod is disabled in the selected profile.".to_owned();
                }
                mods.push(status);
                let mut code_status = if disabled {
                    HorizonFabricCodeModStatus {
                        file_name: file_name.clone(),
                        mod_id: None,
                        name: None,
                        version: None,
                        state: "disabled".to_owned(),
                        detail: "This Java mod is disabled in the selected profile.".to_owned(),
                    }
                } else {
                    let file_size = entry.metadata()?.len();
                    if code_files_inspected >= MAX_FABRIC_CODE_MOD_FILES {
                        HorizonFabricCodeModStatus {
                            file_name: file_name.clone(),
                            mod_id: None,
                            name: None,
                            version: None,
                            state: "unsupported".to_owned(),
                            detail: format!("The code bridge inspects at most {MAX_FABRIC_CODE_MOD_FILES} enabled mod files per profile."),
                        }
                    } else if code_mod_bytes.saturating_add(file_size) > MAX_FABRIC_CODE_MOD_TOTAL_BYTES {
                        HorizonFabricCodeModStatus {
                            file_name: file_name.clone(),
                            mod_id: None,
                            name: None,
                            version: None,
                            state: "unsupported".to_owned(),
                            detail: "The code bridge inspection limit of 2 GiB of enabled mod files has been reached.".to_owned(),
                        }
                    } else {
                        code_files_inspected += 1;
                        code_mod_bytes = code_mod_bytes.saturating_add(file_size);
                        match inspect_fabric_code_jar(&path) {
                            Ok(status) => status,
                            Err(error) => HorizonFabricCodeModStatus {
                                file_name: file_name.clone(),
                                mod_id: None,
                                name: None,
                                version: None,
                                state: "invalid".to_owned(),
                                detail: error.to_string(),
                            },
                        }
                    }
                };
                code_status.file_name = file_name;
                code_mods.push(code_status);
                if mods.len() > 512 {
                    return Err(LauncherError::Unsupported(
                        "this profile contains more than 512 Fabric mod files".to_owned(),
                    ));
                }
            }
        }
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(LauncherError::Unsupported(
                "the profile mods path is not a regular directory".to_owned(),
            ));
        }
        Ok(_) => unreachable!(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    mods.sort_by(|left, right| left.file_name.to_lowercase().cmp(&right.file_name.to_lowercase()));
    code_mods.sort_by(|left, right| {
        left.file_name
            .to_lowercase()
            .cmp(&right.file_name.to_lowercase())
    });
    let mut code_ids = BTreeMap::<String, usize>::new();
    for item in code_mods.iter().filter(|item| item.state == "supported") {
        if let Some(id) = &item.mod_id {
            *code_ids.entry(id.clone()).or_default() += 1;
        }
    }
    for item in &mut code_mods {
        if item.state == "supported"
            && item
                .mod_id
                .as_ref()
                .and_then(|id| code_ids.get(id))
                .is_some_and(|count| *count > 1)
        {
            item.state = "unsupported".to_owned();
            item.detail = "Another enabled JAR declares the same Fabric mod id; duplicate ids are not staged.".to_owned();
        }
    }
    let compatible_count = mods.iter().filter(|item| item.state == "compatible").count();
    let skipped_count = mods
        .iter()
        .filter(|item| matches!(item.state.as_str(), "skipped" | "invalid"))
        .count();
    let code_compatible_count = code_mods
        .iter()
        .filter(|item| item.state == "supported")
        .count();
    let code_skipped_count = code_mods
        .iter()
        .filter(|item| matches!(item.state.as_str(), "unsupported" | "invalid"))
        .count();
    Ok(HorizonFabricBridgeStatus {
        settings,
        compatible_count,
        skipped_count,
        mods,
        code_compatible_count,
        code_skipped_count,
        code_mods,
    })
}

pub fn set_fabric_bridge_enabled(
    instance: &Instance,
    enabled: bool,
) -> Result<HorizonFabricBridgeSettings> {
    ensure_horizon(instance)?;
    let private_dir = horizon_private_dir(instance)?;
    if !enabled {
        let ledger = read_fabric_bridge_ledger(&private_dir)?;
        disable_adapted_mods(instance, &ledger)?;
    }
    let mut settings = read_fabric_bridge_settings(instance)?;
    settings.enabled = enabled;
    let settings_path = private_dir.join(FABRIC_BRIDGE_CONFIG);
    let encoded = serde_json::to_vec_pretty(&settings)?;
    write_atomic(&settings_path, &encoded)?;
    Ok(settings)
}

/// Enables the opt-in Java sidecar that maps Fabric client-tick phases to Horizon's native tick.
pub fn set_fabric_code_bridge_enabled(
    instance: &Instance,
    enabled: bool,
) -> Result<HorizonFabricBridgeSettings> {
    ensure_horizon(instance)?;
    if enabled {
        let private_dir = horizon_private_dir(instance)?;
        let runtime_dir = private_dir.join("fabric-java");
        if !runtime_dir.is_dir() {
            return Err(LauncherError::Unsupported(
                "install the optional Minecraft 26.3 Fabric runtime in Horizon settings before enabling the code bridge".to_owned(),
            ));
        }
        let version_id = instance
            .horizon_fabric_version_id
            .as_deref()
            .ok_or_else(|| {
                LauncherError::Unsupported(
                    "choose a Fabric Loader profile in Horizon settings first".to_owned(),
                )
            })?;
        if !crate::loaders::installed_profiles(&runtime_dir)?
            .iter()
            .any(|profile| {
                profile.version_id == version_id
                    && profile.minecraft_version == HORIZON_GAME_VERSION
                    && profile.loader == "fabric"
            })
        {
            return Err(LauncherError::Unsupported(
                "the Horizon compatibility files are incomplete; reinstall Fabric from Horizon settings".to_owned(),
            ));
        }
        let java_path = instance
            .horizon_fabric_java_path
            .as_deref()
            .ok_or_else(|| {
                LauncherError::Unsupported(
                    "select a Java 25 or newer runtime for the Fabric bridge".to_owned(),
                )
            })?;
        let runtime = crate::java::inspect_runtime(Path::new(java_path))?;
        if runtime.major_version < 25 {
            return Err(LauncherError::Unsupported(format!(
                "the Fabric code bridge requires Java 25 or newer; selected Java is {}",
                runtime.major_version
            )));
        }
    }
    let private_dir = horizon_private_dir(instance)?;
    let mut settings = read_fabric_bridge_settings(instance)?;
    settings.code_enabled = enabled;
    write_atomic(
        &private_dir.join(FABRIC_BRIDGE_CONFIG),
        &serde_json::to_vec_pretty(&settings)?,
    )?;
    Ok(settings)
}

/// Creates a private, filtered mod folder for the Java helper. Only JARs
/// verified against the explicit ClientTickEvents subset are copied there;
/// the user's original mods folder is never edited or passed to the helper.
pub fn prepare_fabric_code_bridge(instance: &Instance) -> Result<PathBuf> {
    ensure_horizon(instance)?;
    let status = fabric_bridge_status(instance)?;
    let private_dir = horizon_private_dir(instance)?;
    let bridge_dir = private_dir.join(FABRIC_CODE_BRIDGE_DIR);
    match fs::symlink_metadata(&bridge_dir) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            return Err(LauncherError::Unsupported(
                "the private Fabric code staging path is not a regular folder".to_owned(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&bridge_dir)?,
        Err(error) => return Err(error.into()),
    }
    let stage_dir = bridge_dir.join("mods");
    match fs::symlink_metadata(&stage_dir) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            return Err(LauncherError::Unsupported(
                "the private Fabric code mods path is not a regular folder".to_owned(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&stage_dir)?,
        Err(error) => return Err(error.into()),
    }

    let ledger_path = bridge_dir.join(FABRIC_CODE_STAGE_LEDGER);
    let mut ledger = match fs::symlink_metadata(&ledger_path) {
        Ok(metadata) if metadata.file_type().is_file() && metadata.len() <= MAX_MANIFEST_BYTES => {
            serde_json::from_slice::<FabricCodeStageLedger>(&fs::read(&ledger_path)?)?
        }
        Ok(_) => {
            return Err(LauncherError::Unsupported(
                "the private Fabric code staging index is not a bounded regular file".to_owned(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => FabricCodeStageLedger::default(),
        Err(error) => return Err(error.into()),
    };

    let source_dir = Path::new(&instance.game_dir).join("mods");
    let mut desired = BTreeMap::new();
    let mut desired_total_bytes = 0u64;
    for mod_status in status
        .code_mods
        .iter()
        .filter(|mod_status| mod_status.state == "supported")
    {
        let file_name = Path::new(&mod_status.file_name)
            .file_name()
            .filter(|name| name.to_string_lossy() == mod_status.file_name)
            .ok_or_else(|| LauncherError::InvalidInput("Fabric mod filename is invalid".to_owned()))?;
        let source = source_dir.join(file_name);
        let source_meta = fs::symlink_metadata(&source)?;
        if !source_meta.file_type().is_file() || source_meta.len() > MAX_MOD_BYTES {
            return Err(LauncherError::Unsupported(format!(
                "{} changed after code-bridge inspection",
                mod_status.file_name
            )));
        }
        let digest_before_inspection = sha1_file(&source)?;
        let current_status = inspect_fabric_code_jar(&source)?;
        let digest = sha1_file(&source)?;
        if digest != digest_before_inspection
            || current_status.state != "supported"
            || current_status.mod_id != mod_status.mod_id
        {
            return Err(LauncherError::Unsupported(format!(
                "{} changed or no longer passes the code-bridge allowlist",
                mod_status.file_name
            )));
        }
        let staged_name = format!("{digest}.jar");
        if !desired.contains_key(&staged_name) {
            if desired.len() >= MAX_FABRIC_CODE_MOD_FILES
                || desired_total_bytes.saturating_add(source_meta.len()) > MAX_FABRIC_CODE_MOD_TOTAL_BYTES
            {
                return Err(LauncherError::Unsupported(
                    "the Fabric code bridge staging limit is 128 files and 2 GiB per profile".to_owned(),
                ));
            }
            desired_total_bytes = desired_total_bytes.saturating_add(source_meta.len());
        }
        desired.insert(staged_name, (source, digest));
    }

    for (old_name, old_digest) in &ledger.files {
        validate_staged_name(old_name)?;
        if old_name.as_bytes().get(..40) != Some(old_digest.as_bytes()) {
            return Err(LauncherError::Unsupported(
                "the private Fabric staging index contains a mismatched checksum".to_owned(),
            ));
        }
        if desired.contains_key(old_name) {
            continue;
        }
    }

    let mut next_ledger = FabricCodeStageLedger::default();
    for (staged_name, (source, digest)) in &desired {
        let target = stage_dir.join(staged_name);
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_file() => {
                if sha1_file(&target)? != *digest {
                    return Err(LauncherError::Unsupported(format!(
                        "staged Fabric file {staged_name} changed outside OpnLauncher; it was left untouched"
                    )));
                }
            }
            Ok(_) => {
                return Err(LauncherError::Unsupported(format!(
                    "staged Fabric path {staged_name} is not a regular file"
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut bytes = Vec::new();
                File::open(source)?
                    .take(MAX_MOD_BYTES + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() as u64 > MAX_MOD_BYTES {
                    return Err(LauncherError::Unsupported(format!(
                        "{} grew beyond the code-bridge staging limit",
                        source.file_name().unwrap_or_default().to_string_lossy()
                    )));
                }
                if format!("{:x}", Sha1::digest(&bytes)) != *digest {
                    return Err(LauncherError::Unsupported(format!(
                        "{} changed while it was being staged; no replacement was made",
                        source.file_name().unwrap_or_default().to_string_lossy()
                    )));
                }
                write_atomic(&target, &bytes)?;
            }
            Err(error) => return Err(error.into()),
        }
        next_ledger.files.insert(staged_name.clone(), digest.clone());
    }

    for (old_name, old_digest) in &ledger.files {
        if desired.contains_key(old_name) {
            continue;
        }
        let old_path = stage_dir.join(old_name);
        match fs::symlink_metadata(&old_path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                if sha1_file(&old_path)? != *old_digest {
                    return Err(LauncherError::Unsupported(format!(
                        "staged Fabric file {old_name} changed outside OpnLauncher; it was left untouched"
                    )));
                }
                fs::remove_file(old_path)?;
            }
            Ok(_) => {
                return Err(LauncherError::Unsupported(format!(
                    "staged Fabric path {old_name} is not a regular file"
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    for entry in fs::read_dir(&stage_dir)? {
        let entry = entry?;
        let is_jar = entry
            .path()
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"));
        if is_jar && !entry.file_type()?.is_file() {
            return Err(LauncherError::Unsupported(format!(
                "staged Fabric path {} is not a regular file",
                entry.file_name().to_string_lossy()
            )));
        }
        if is_jar {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !next_ledger.files.contains_key(&name) {
                return Err(LauncherError::Unsupported(format!(
                    "unmanaged JAR {name} is present in the private Fabric staging folder; it was left untouched"
                )));
            }
        }
    }
    ledger.files = next_ledger.files;
    write_atomic(&ledger_path, &serde_json::to_vec_pretty(&ledger)?)?;
    Ok(stage_dir)
}

fn validate_staged_name(name: &str) -> Result<()> {
    let bytes = name.as_bytes();
    if bytes.len() != 44
        || !name.ends_with(".jar")
        || !bytes[..40]
            .iter()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || Path::new(name).file_name().is_none_or(|file| file.to_string_lossy() != name)
    {
        return Err(LauncherError::Unsupported(
            "the private Fabric staging index contains an invalid filename".to_owned(),
        ));
    }
    Ok(())
}

/// Prepares code-free Fabric resources once before the native Horizon process starts.
/// Java classes, entrypoints, mixins and extra runtime dependencies are never executed.
pub fn prepare_fabric_bridge(instance: &Instance) -> Result<String> {
    ensure_horizon(instance)?;
    let settings = read_fabric_bridge_settings(instance)?;
    if !settings.enabled {
        return Ok("Horizon Fabric content adapter is off.".to_owned());
    }

    let private_dir = horizon_private_dir(instance)?;
    let mut ledger = read_fabric_bridge_ledger(&private_dir)?;
    let mods_dir = Path::new(&instance.game_dir).join("mods");
    let mut seen_sources = BTreeSet::new();
    let mut adapted_count = 0usize;
    let mut skipped = Vec::new();
    let mut sources = Vec::new();

    match fs::symlink_metadata(&mods_dir) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            for entry in fs::read_dir(&mods_dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let file_name = entry.file_name().to_string_lossy().into_owned();
                let lower = file_name.to_ascii_lowercase();
                if !lower.ends_with(".jar") {
                    continue;
                }
                let path = entry.path();
                sources.push((file_name, path));
                if sources.len() + skipped.len() > 512 {
                    return Err(LauncherError::Unsupported(
                        "this profile contains more than 512 Fabric mod files".to_owned(),
                    ));
                }
            }
        }
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(LauncherError::Unsupported(
                "the profile mods path is not a regular directory".to_owned(),
            ));
        }
        Ok(_) => unreachable!(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    sources.sort_by(|left, right| left.0.to_lowercase().cmp(&right.0.to_lowercase()));
    let mods_dir = Path::new(&instance.game_dir).join("horizon-mods");
    ensure_regular_directory(&mods_dir)?;
    let mut occupied = installed_content_namespaces(instance, &mods_dir, &ledger)?;
    for (source_name, source_path) in sources {
        let package = match read_fabric_content_package(&source_path) {
            Ok(package) => package,
            Err(error) => {
                if let Some(previous) = ledger.mods.get(&source_name) {
                    set_content_mod_enabled(instance, &previous.mod_id, false)?;
                }
                skipped.push(format!("{source_name}: {error}"));
                continue;
            }
        };
        let mod_id = horizon_fabric_mod_id(&package.id, &package.source_sha1);
        let namespaces = package
            .namespaces
            .iter()
            .filter(|namespace| {
                namespace.as_str() != mod_id
                    && !matches!(namespace.as_str(), "minecraft" | "c" | "horizon")
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut owned = vec![mod_id.clone()];
        owned.extend(namespaces);
        let conflict = occupied.iter().find_map(|(other_id, other_owned)| {
            horizon_mods::content::clash(
                (&mod_id, &owned),
                (other_id.as_str(), other_owned.as_slice()),
            )
        });
        if let Some(conflict) = conflict {
            if let Some(previous) = ledger.mods.get(&source_name) {
                set_content_mod_enabled(instance, &previous.mod_id, false)?;
            }
            skipped.push(format!("{source_name}: {conflict}"));
            continue;
        }
        match write_adapted_fabric_mod(instance, &private_dir, &mut ledger, &source_name, package) {
            Ok(()) => {
                seen_sources.insert(source_name);
                occupied.push((mod_id, owned));
                adapted_count += 1;
            }
            Err(error) => {
                if let Some(previous) = ledger.mods.get(&source_name) {
                    set_content_mod_enabled(instance, &previous.mod_id, false)?;
                }
                skipped.push(format!("{source_name}: {error}"));
            }
        }
    }

    // Keep generated files on disk, but disable them when their source JAR is missing or disabled.
    for (source_name, adapted) in &ledger.mods {
        if !seen_sources.contains(source_name) {
            set_content_mod_enabled(instance, &adapted.mod_id, false)?;
        }
    }
    write_fabric_bridge_ledger(&private_dir, &ledger)?;

    let mut summary = format!(
        "Horizon Fabric content adapter prepared {adapted_count} code-free mod(s); skipped {} Java/code or unsupported mod(s).",
        skipped.len()
    );
    for item in skipped.iter().take(8) {
        summary.push_str("\nSkipped: ");
        summary.push_str(item);
    }
    if skipped.len() > 8 {
        summary.push_str(&format!("\nAnd {} more skipped mod(s).", skipped.len() - 8));
    }
    Ok(summary)
}

fn read_fabric_bridge_settings(instance: &Instance) -> Result<HorizonFabricBridgeSettings> {
    let private_dir = horizon_private_dir(instance)?;
    let path = private_dir.join(FABRIC_BRIDGE_CONFIG);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() && metadata.len() <= MAX_MANIFEST_BYTES => {
            serde_json::from_slice(&fs::read(path)?).map_err(Into::into)
        }
        Ok(_) => Err(LauncherError::Unsupported(
            "the Horizon Fabric adapter settings file is not a regular, bounded file".to_owned(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(HorizonFabricBridgeSettings::default())
        }
        Err(error) => Err(error.into()),
    }
}

fn horizon_private_dir(instance: &Instance) -> Result<PathBuf> {
    let game_dir = fs::canonicalize(&instance.game_dir).map_err(|error| {
        LauncherError::Unsupported(format!("cannot open Horizon profile folder: {error}"))
    })?;
    if !game_dir.is_dir() {
        return Err(LauncherError::InvalidInput(
            "Horizon profile path must be a directory".to_owned(),
        ));
    }
    let private_dir = game_dir.join(".opnlauncher");
    match fs::symlink_metadata(&private_dir) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(private_dir),
        Ok(_) => Err(LauncherError::Unsupported(
            "the Horizon private profile folder is not a regular directory".to_owned(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&private_dir)?;
            Ok(private_dir)
        }
        Err(error) => Err(error.into()),
    }
}

fn read_fabric_bridge_ledger(private_dir: &Path) -> Result<FabricBridgeLedger> {
    let path = private_dir.join(FABRIC_BRIDGE_LEDGER);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() && metadata.len() <= MAX_MANIFEST_BYTES => {
            serde_json::from_slice(&fs::read(path)?).map_err(Into::into)
        }
        Ok(_) => Err(LauncherError::Unsupported(
            "the Horizon adapter index is not a regular, bounded file".to_owned(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(FabricBridgeLedger::default())
        }
        Err(error) => Err(error.into()),
    }
}

fn write_fabric_bridge_ledger(private_dir: &Path, ledger: &FabricBridgeLedger) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(ledger)?;
    write_atomic(&private_dir.join(FABRIC_BRIDGE_LEDGER), &bytes)
}

fn set_content_mod_enabled(instance: &Instance, id: &str, enabled: bool) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(LauncherError::Unsupported(
            "the Horizon adapter index contains an invalid mod identifier".to_owned(),
        ));
    }
    let config = Path::new(&instance.game_dir).join("horizon-mods.txt");
    let mut lines = fs::read_to_string(&config)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with(&format!("mod.{id}:")))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if enabled {
        lines.push(format!("mod.{id}:true"));
    }
    write_lines_atomic(&config, &lines)
}

fn write_adapted_fabric_mod(
    instance: &Instance,
    private_dir: &Path,
    ledger: &mut FabricBridgeLedger,
    source_name: &str,
    source: FabricContentPackage,
) -> Result<()> {
    let id = horizon_fabric_mod_id(&source.id, &source.source_sha1);
    let namespaces = source
        .namespaces
        .iter()
        .filter(|namespace| {
            namespace.as_str() != id
                && !matches!(namespace.as_str(), "minecraft" | "c" | "horizon")
        })
        .cloned()
        .collect::<Vec<_>>();
    let manifest = FabricContentManifest {
        id: id.clone(),
        name: source.name,
        version: source.version,
        description: source.description,
        authors: source.authors,
        api: "0.2",
        namespaces,
        capabilities: vec!["registries"],
    };
    let manifest_text = toml::to_string(&manifest).map_err(|error| {
        LauncherError::Unsupported(format!("could not write Horizon content manifest: {error}"))
    })?;
    horizon_mods::manifest::Manifest::parse(&manifest_text)
        .map_err(LauncherError::Unsupported)?;
    let file_refs = source
        .files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    let package_bytes = horizon_mods::Package::write_files(&manifest_text, None, &file_refs)
        .map_err(LauncherError::Unsupported)?;
    let package_sha1 = sha1_bytes(&package_bytes);
    let output_file = format!("{id}.hmod");
    let mods_dir = Path::new(&instance.game_dir).join("horizon-mods");
    ensure_regular_directory(&mods_dir)?;
    let output_path = mods_dir.join(&output_file);

    if let Some(previous) = ledger.mods.get(source_name) {
        if previous.output_file != output_file || previous.mod_id != id {
            set_content_mod_enabled(instance, &previous.mod_id, false)?;
        }
    }

    match fs::symlink_metadata(&output_path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            let previous = ledger.mods.get(source_name).filter(|previous| {
                previous.output_file == output_file && previous.mod_id == id
            });
            let current_hash = sha1_file(&output_path)?;
            if current_hash != package_sha1 {
                if !previous.is_some_and(|previous| previous.package_sha1 == current_hash) {
                    return Err(LauncherError::Unsupported(
                        "the generated Horizon package was changed locally; it was left untouched".to_owned(),
                    ));
                }
                validate_adapted_package(private_dir, &package_bytes)?;
                write_atomic(&output_path, &package_bytes)?;
            }
        }
        Ok(_) => {
            return Err(LauncherError::Unsupported(
                "the generated Horizon package path is not a regular file".to_owned(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            validate_adapted_package(private_dir, &package_bytes)?;
            write_atomic(&output_path, &package_bytes)?;
        }
        Err(error) => return Err(error.into()),
    }

    set_content_mod_enabled(instance, &id, true)?;
    ledger.mods.insert(
        source_name.to_owned(),
        AdaptedFabricMod {
            source_sha1: source.source_sha1,
            package_sha1,
            mod_id: id,
            output_file,
        },
    );
    Ok(())
}

fn validate_adapted_package(private_dir: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = private_dir.join(format!(".fabric-adapter-{}.hmod", Uuid::new_v4()));
    write_atomic(&temporary, bytes)?;
    let validation = (|| {
        let package = horizon_mods::Package::read(&temporary)
            .map_err(LauncherError::Unsupported)?;
        horizon_mods::content::Content::read(&temporary, &package.manifest)
            .map_err(LauncherError::Unsupported)?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary);
    validation
}

fn ensure_regular_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(LauncherError::Unsupported(format!(
            "{} is not a regular directory",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn disable_adapted_mods(instance: &Instance, ledger: &FabricBridgeLedger) -> Result<()> {
    for adapted in ledger.mods.values() {
        set_content_mod_enabled(instance, &adapted.mod_id, false)?;
    }
    Ok(())
}

fn installed_content_namespaces(
    instance: &Instance,
    mods_dir: &Path,
    ledger: &FabricBridgeLedger,
) -> Result<Vec<(String, Vec<String>)>> {
    let mut generated = BTreeSet::new();
    for adapted in ledger.mods.values() {
        if adapted.output_file != format!("{}.hmod", adapted.mod_id)
            || adapted.mod_id.is_empty()
            || adapted.mod_id.len() > 64
            || !adapted
                .mod_id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            continue;
        }
        let path = mods_dir.join(&adapted.output_file);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            _ => continue,
        };
        if metadata.len() > MAX_MOD_BYTES || sha1_file(&path)? != adapted.package_sha1 {
            continue;
        }
        generated.insert(adapted.mod_id.clone());
    }

    let enabled = enabled_mod_ids(Path::new(&instance.game_dir))?;
    let mut occupied = Vec::new();
    for item in list_mods(instance)? {
        let Some(id) = item.id.as_deref() else { continue };
        if !item.enabled || generated.contains(id) || !enabled.contains(id) {
            continue;
        }
        let path = mods_dir.join(&item.file_name);
        let package = match horizon_mods::Package::read(&path) {
            Ok(package) => package,
            Err(_) => continue,
        };
        occupied.push((
            package.manifest.id.clone(),
            package.manifest.owned_namespaces().map(str::to_owned).collect(),
        ));
    }
    Ok(occupied)
}

#[derive(Serialize)]
struct FabricContentManifest {
    id: String,
    name: String,
    version: String,
    description: String,
    authors: Vec<String>,
    api: &'static str,
    namespaces: Vec<String>,
    capabilities: Vec<&'static str>,
}

struct FabricJarInspection {
    status: HorizonFabricModStatus,
    content_entries: Vec<(usize, String, u64)>,
    description: String,
    authors: Vec<String>,
    namespaces: Vec<String>,
}

fn inspect_fabric_jar(path: &Path) -> Result<FabricJarInspection> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_MOD_BYTES {
        return Err(LauncherError::Unsupported(
            "the Fabric file is not a regular JAR under 512 MiB".to_owned(),
        ));
    }
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        LauncherError::Unsupported(format!("the Fabric JAR is not a valid archive: {error}"))
    })?;
    if archive.len() > 100_000 {
        return Err(LauncherError::Unsupported(
            "the Fabric JAR has too many archive entries".to_owned(),
        ));
    }

    let mut metadata_bytes = None;
    let mut content_entries = Vec::new();
    let mut content_names = BTreeSet::new();
    let mut namespaces = BTreeSet::new();
    let mut code = false;
    let mut unsupported_path = false;
    let mut total_content = 0u64;

    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            LauncherError::Unsupported(format!("cannot inspect Fabric JAR entry: {error}"))
        })?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_owned();
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".class") || lower.starts_with("meta-inf/jars/") {
            code = true;
        }
        if name == "fabric.mod.json" {
            if metadata_bytes.is_some() || entry.size() > MAX_MANIFEST_BYTES {
                return Err(LauncherError::Unsupported(
                    "the Fabric JAR has a duplicate or oversized fabric.mod.json".to_owned(),
                ));
            }
            let mut bytes = Vec::new();
            entry
                .take(MAX_MANIFEST_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_MANIFEST_BYTES {
                return Err(LauncherError::Unsupported(
                    "fabric.mod.json is larger than 1 MiB".to_owned(),
                ));
            }
            metadata_bytes = Some(bytes);
            continue;
        }
        if !name.starts_with("assets/") && !name.starts_with("data/") {
            continue;
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            unsupported_path = true;
            continue;
        }
        if horizon_mods::content::check_path(&name).is_err() {
            unsupported_path = true;
            continue;
        }
        if !content_names.insert(name.clone()) {
            unsupported_path = true;
            continue;
        }
        if entry.size() > MAX_FABRIC_CONTENT_FILE_BYTES {
            return Err(LauncherError::Unsupported(format!(
                "{name} is larger than 16 MiB"
            )));
        }
        total_content = total_content.saturating_add(entry.size());
        if total_content > MAX_FABRIC_CONTENT_TOTAL_BYTES {
            return Err(LauncherError::Unsupported(
                "the JAR's static resources exceed the 128 MiB adapter limit".to_owned(),
            ));
        }
        if content_entries.len() >= MAX_FABRIC_CONTENT_FILES {
            return Err(LauncherError::Unsupported(
                "the JAR has too many static resource files".to_owned(),
            ));
        }
        let namespace = name.split('/').nth(1).unwrap_or_default();
        if !namespace.is_empty() {
            namespaces.insert(namespace.to_owned());
        }
        content_entries.push((index, name, entry.size()));
    }

    let metadata_bytes = metadata_bytes.ok_or_else(|| {
        LauncherError::Unsupported("fabric.mod.json is missing".to_owned())
    })?;
    let metadata: Value = serde_json::from_slice(&metadata_bytes).map_err(|error| {
        LauncherError::Unsupported(format!("fabric.mod.json is invalid: {error}"))
    })?;
    let id = metadata.get("id").and_then(Value::as_str).unwrap_or_default();
    if !valid_fabric_id(id) {
        return Err(LauncherError::Unsupported(
            "fabric.mod.json has an invalid mod id".to_owned(),
        ));
    }
    let name = metadata
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(id)
        .chars()
        .take(128)
        .collect::<String>();
    let version = metadata
        .get("version")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown")
        .chars()
        .take(128)
        .collect::<String>();
    let description = metadata
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(2048)
        .collect::<String>();
    let authors = metadata
        .get("authors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(32)
        .map(|value| value.chars().take(128).collect::<String>())
        .collect();
    let namespaces = namespaces
        .into_iter()
        .filter(|namespace| !matches!(namespace.as_str(), "minecraft" | "c" | "horizon"))
        .collect();
    let detail = if code
        || metadata_field_nonempty(&metadata, "entrypoints")
        || metadata_field_nonempty(&metadata, "mixins")
        || metadata.get("accessWidener").and_then(Value::as_str).is_some_and(|value| !value.is_empty())
    {
        Some("Contains Java classes, entrypoints, mixins, or an access widener; Java code mods are not executed by Horizon.".to_owned())
    } else if metadata
        .get("depends")
        .and_then(Value::as_object)
        .is_some_and(|dependencies| dependencies.keys().any(|id| !matches!(id.as_str(), "minecraft" | "fabricloader" | "java")))
    {
        Some("Requires another Fabric mod or API. This first adapter handles standalone code-free packages only.".to_owned())
    } else if unsupported_path {
        Some("Contains asset/data paths or archive entries Horizon cannot safely import.".to_owned())
    } else if content_entries.is_empty() {
        Some("Contains no static assets or data for the Horizon adapter.".to_owned())
    } else {
        None
    };
    let state = if detail.is_some() { "skipped" } else { "compatible" };
    Ok(FabricJarInspection {
        status: HorizonFabricModStatus {
            file_name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            mod_id: Some(id.to_owned()),
            name: Some(name),
            version: Some(version),
            state: state.to_owned(),
            detail: detail.unwrap_or_else(|| "Code-free assets/data are ready for one-time Horizon preparation.".to_owned()),
        },
        content_entries: if state == "compatible" {
            content_entries
        } else {
            Vec::new()
        },
        description,
        authors,
        namespaces,
    })
}

/// The executable mapping is deliberately small: Fabric's client start/end
/// events, bounded player/environment/block/input snapshots and actions,
/// keyboard mappings, plain system chat, and one HUD action. The code bridge
/// replaces Fabric API with a launcher-owned facade, so mods using Mixins or
/// other Minecraft/Fabric APIs must be filtered before the helper JVM.
fn inspect_fabric_code_jar(path: &Path) -> Result<HorizonFabricCodeModStatus> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_MOD_BYTES {
        return Err(LauncherError::Unsupported(
            "the Fabric file is not a regular JAR under 512 MiB".to_owned(),
        ));
    }
    let mut archive = ZipArchive::new(File::open(path)?).map_err(|error| {
        LauncherError::Unsupported(format!("the Fabric JAR is not a valid archive: {error}"))
    })?;
    if archive.len() > 100_000 {
        return Err(LauncherError::Unsupported(
            "the Fabric JAR has too many archive entries".to_owned(),
        ));
    }

    let mut metadata_bytes = None;
    let mut class_count = 0usize;
    let mut total_class_bytes = 0u64;
    let mut class_paths = BTreeSet::new();
    let mut unsupported_detail = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            LauncherError::Unsupported(format!("cannot inspect Fabric JAR entry: {error}"))
        })?;
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry.name().to_owned();
        let lower_name = entry_name.to_ascii_lowercase();
        if lower_name == "fabric.mod.json" {
            if metadata_bytes.is_some() || entry.size() > MAX_MANIFEST_BYTES {
                return Err(LauncherError::Unsupported(
                    "the Fabric JAR has a duplicate or oversized fabric.mod.json".to_owned(),
                ));
            }
            let mut bytes = Vec::new();
            entry
                .take(MAX_MANIFEST_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_MANIFEST_BYTES {
                return Err(LauncherError::Unsupported(
                    "fabric.mod.json is larger than 1 MiB".to_owned(),
                ));
            }
            metadata_bytes = Some(bytes);
            continue;
        }
        if lower_name.ends_with(".jar") {
            unsupported_detail.get_or_insert_with(|| {
                "Nested JAR dependencies are not supported by the narrow event mapping.".to_owned()
            });
            continue;
        }
        if !lower_name.ends_with(".class") {
            continue;
        }
        class_paths.insert(entry_name[..entry_name.len() - ".class".len()].to_owned());
        let declared_class_bytes = entry.size();
        class_count += 1;
        total_class_bytes = total_class_bytes.saturating_add(declared_class_bytes);
        if class_count > MAX_FABRIC_CODE_CLASSES
            || declared_class_bytes > MAX_FABRIC_CODE_CLASS_BYTES
            || total_class_bytes > MAX_FABRIC_CODE_TOTAL_BYTES
        {
            return Err(LauncherError::Unsupported(
                "the Fabric mod's class files exceed the code bridge inspection limits".to_owned(),
            ));
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .take(MAX_FABRIC_CODE_CLASS_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != declared_class_bytes {
            return Err(LauncherError::Unsupported(
                "a class file's expanded size does not match its archive metadata".to_owned(),
            ));
        }
        if let Some(detail) = unsupported_class_api(&bytes) {
            unsupported_detail.get_or_insert(detail);
        }
    }

    let metadata_bytes = metadata_bytes.ok_or_else(|| {
        LauncherError::Unsupported("fabric.mod.json is missing".to_owned())
    })?;
    let metadata: Value = serde_json::from_slice(&metadata_bytes).map_err(|error| {
        LauncherError::Unsupported(format!("fabric.mod.json is invalid: {error}"))
    })?;
    let id = metadata.get("id").and_then(Value::as_str).unwrap_or_default();
    if !valid_fabric_id(id) {
        return Err(LauncherError::Unsupported(
            "fabric.mod.json has an invalid mod id".to_owned(),
        ));
    }
    let name = metadata
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(id)
        .chars()
        .take(128)
        .collect::<String>();
    let version = metadata
        .get("version")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown")
        .chars()
        .take(128)
        .collect::<String>();

    let api_shim = matches!(
        id,
        "fabric-api"
            | "fabric-api-base"
            | "fabric-lifecycle-events-v1"
            | "fabric-key-mapping-api-v1"
    );
    if api_shim {
        return Ok(HorizonFabricCodeModStatus {
            file_name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            mod_id: Some(id.to_owned()),
            name: Some(name),
            version: Some(version),
            state: "api-shim".to_owned(),
            detail: "Replaced by OpnLauncher's 26.3 bridge shim for mapped client ticks and keyboard key mappings; this JAR is not loaded.".to_owned(),
        });
    }

    if metadata_field_nonempty(&metadata, "mixins") {
        unsupported_detail.get_or_insert_with(|| {
            "Fabric Mixin configs are unsupported: they cannot transform Horizon's Rust game.".to_owned()
        });
    }
    if metadata_field_nonempty(&metadata, "languageAdapters") {
        unsupported_detail.get_or_insert_with(|| {
            "Custom Fabric language adapters are not supported by the mapped bridge.".to_owned()
        });
    }
    if metadata
        .get("accessWidener")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        unsupported_detail.get_or_insert_with(|| {
            "Fabric access wideners are unsupported by the native Horizon bridge.".to_owned()
        });
    }
    let allowed_dependencies = [
        "minecraft",
        "java",
        "fabricloader",
        "fabric-api",
        "fabric-api-base",
        "fabric-lifecycle-events-v1",
        "fabric-key-mapping-api-v1",
    ];
    if metadata.get("depends").is_some_and(|value| !value.is_object()) {
        unsupported_detail.get_or_insert_with(|| "The Fabric dependency metadata is malformed.".to_owned());
    } else if metadata
        .get("depends")
        .and_then(Value::as_object)
        .is_some_and(|dependencies| dependencies.keys().any(|dependency| !allowed_dependencies.contains(&dependency.as_str())))
    {
        unsupported_detail.get_or_insert_with(|| {
            "This mod depends on another mod or Fabric API module outside the mapped lifecycle subset.".to_owned()
        });
    }
    if metadata
        .get("environment")
        .is_some_and(|environment| !matches!(environment.as_str(), Some("*") | Some("client")))
    {
        unsupported_detail.get_or_insert_with(|| "This is not a client-side Fabric mod.".to_owned());
    }
    match metadata.get("entrypoints").and_then(Value::as_object) {
        Some(entrypoints)
            if !entrypoints.is_empty()
                && entrypoints
                    .keys()
                    .all(|key| matches!(key.as_str(), "main" | "client"))
                && entrypoints.values().all(is_default_adapter_entrypoint) => {}
        Some(_) => {
            unsupported_detail.get_or_insert_with(|| {
                "Only Fabric main and client initializer entrypoints are mapped.".to_owned()
            });
        }
        None => {
            unsupported_detail.get_or_insert_with(|| {
                "The mod has no supported main or client initializer entrypoint.".to_owned()
            });
        }
    }
    if let Some(entrypoints) = metadata.get("entrypoints").and_then(Value::as_object) {
        for class_path in entrypoints.values().flat_map(entrypoint_class_paths) {
            if !class_paths.contains(&class_path.replace('.', "/")) {
                unsupported_detail.get_or_insert_with(|| {
                    format!("Fabric entrypoint class `{class_path}` is missing from this JAR.")
                });
            }
        }
    }
    if class_count == 0 {
        unsupported_detail.get_or_insert_with(|| "The JAR contains no Java classes to execute.".to_owned());
    }

    let state = if unsupported_detail.is_some() {
        "unsupported"
    } else {
        "supported"
    };
    Ok(HorizonFabricCodeModStatus {
        file_name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        mod_id: Some(id.to_owned()),
        name: Some(name),
        version: Some(version),
        state: state.to_owned(),
        detail: unsupported_detail.unwrap_or_else(|| {
            "Uses mapped initializers and START/END_CLIENT_TICK, read-only player/environment snapshots, bounded nearby block reads, default keyboard key mappings, plain system messages, and the action-bar subset; callbacks receive only a limited Minecraft facade.".to_owned()
        }),
    })
}

#[derive(Debug)]
enum ClassPoolEntry {
    Utf8(String),
    Class(u16),
    NameAndType(u16, u16),
    MemberRef(u8, u16, u16),
    Other,
}

fn unsupported_class_api(bytes: &[u8]) -> Option<String> {
    use std::io::Cursor;

    fn read_u8(cursor: &mut Cursor<&[u8]>) -> Option<u8> {
        let mut value = [0];
        cursor.read_exact(&mut value).ok()?;
        Some(value[0])
    }
    fn read_u16(cursor: &mut Cursor<&[u8]>) -> Option<u16> {
        let mut value = [0; 2];
        cursor.read_exact(&mut value).ok()?;
        Some(u16::from_be_bytes(value))
    }
    fn skip(cursor: &mut Cursor<&[u8]>, count: u64) -> Option<()> {
        let next = cursor.position().checked_add(count)?;
        if next > cursor.get_ref().len() as u64 {
            return None;
        }
        cursor.set_position(next);
        Some(())
    }

    let unsupported = || "The class file is malformed or uses an unsupported JVM class format.".to_owned();
    if bytes.len() < 10 || bytes.get(..4)? != [0xCA, 0xFE, 0xBA, 0xBE] {
        return Some(unsupported());
    }
    let mut cursor = Cursor::new(bytes);
    cursor.set_position(8);
    let Some(count) = read_u16(&mut cursor).map(usize::from) else {
        return Some(unsupported());
    };
    if count == 0 || count > 65_535 {
        return Some(unsupported());
    }
    let mut pool = Vec::with_capacity(count);
    pool.push(ClassPoolEntry::Other);
    let mut index = 1;
    while index < count {
        let Some(tag) = read_u8(&mut cursor) else {
            return Some(unsupported());
        };
        let entry = match tag {
            1 => {
                let Some(length) = read_u16(&mut cursor) else {
                    return Some(unsupported());
                };
                let mut value = vec![0; length as usize];
                if cursor.read_exact(&mut value).is_err() {
                    return Some(unsupported());
                }
                ClassPoolEntry::Utf8(String::from_utf8_lossy(&value).into_owned())
            }
            7 => match read_u16(&mut cursor) {
                Some(name_index) => ClassPoolEntry::Class(name_index),
                None => return Some(unsupported()),
            },
            9..=11 => {
                let (Some(class_index), Some(name_type_index)) =
                    (read_u16(&mut cursor), read_u16(&mut cursor))
                else {
                    return Some(unsupported());
                };
                ClassPoolEntry::MemberRef(tag, class_index, name_type_index)
            }
            12 => {
                let (Some(name_index), Some(descriptor_index)) =
                    (read_u16(&mut cursor), read_u16(&mut cursor))
                else {
                    return Some(unsupported());
                };
                ClassPoolEntry::NameAndType(name_index, descriptor_index)
            }
            3 | 4 | 16 | 19 | 20 => {
                if skip(&mut cursor, if matches!(tag, 3 | 4) { 4 } else { 2 }).is_none() {
                    return Some(unsupported());
                }
                ClassPoolEntry::Other
            }
            5 | 6 => {
                if skip(&mut cursor, 8).is_none() {
                    return Some(unsupported());
                }
                pool.push(ClassPoolEntry::Other);
                index += 1;
                ClassPoolEntry::Other
            }
            15 => {
                if skip(&mut cursor, 3).is_none() {
                    return Some(unsupported());
                }
                ClassPoolEntry::Other
            }
            17 | 18 => {
                if skip(&mut cursor, 4).is_none() {
                    return Some(unsupported());
                }
                ClassPoolEntry::Other
            }
            _ => return Some(unsupported()),
        };
        pool.push(entry);
        index += 1;
    }

    let utf8 = |entry_index: u16| -> Option<&str> {
        match pool.get(entry_index as usize)? {
            ClassPoolEntry::Utf8(value) => Some(value),
            _ => None,
        }
    };
    let class_name = |entry_index: u16| -> Option<&str> {
        match pool.get(entry_index as usize)? {
            ClassPoolEntry::Class(name_index) => utf8(*name_index),
            _ => None,
        }
    };
    let allowed_fabric_class = |name: &str| {
        matches!(
            name,
            "net/fabricmc/api/ModInitializer"
                | "net/fabricmc/api/ClientModInitializer"
                | "net/fabricmc/fabric/api/event/Event"
                | "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents"
                | "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents$StartTick"
                | "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents$EndTick"
                | "net/fabricmc/fabric/api/client/keymapping/v1/KeyMappingHelper"
        )
    };
    let allowed_minecraft_class = |name: &str| {
        matches!(
            name,
            "net/minecraft/client/Minecraft"
                | "net/minecraft/client/Options"
                | "net/minecraft/client/KeyMapping"
                | "net/minecraft/client/KeyMapping$Category"
                | "net/minecraft/client/multiplayer/ClientLevel"
                | "net/minecraft/world/level/Level"
                | "net/minecraft/world/level/block/Block"
                | "net/minecraft/world/level/block/state/BlockState"
                | "net/minecraft/client/player/LocalPlayer"
                | "net/minecraft/client/gui/Gui"
                | "net/minecraft/client/gui/Hud"
                | "net/minecraft/world/entity/Entity"
                | "net/minecraft/world/entity/LivingEntity"
                | "net/minecraft/world/entity/player/Player"
                | "net/minecraft/core/BlockPos"
                | "net/minecraft/world/phys/Vec3"
                | "net/minecraft/network/chat/Component"
                | "net/minecraft/network/chat/MutableComponent"
                | "net/minecraft/resources/Identifier"
        )
    };

    for entry in &pool {
        if let ClassPoolEntry::Utf8(value) = entry {
            // Class names are checked through CONSTANT_Class below. Restrict
            // this pass to JVM type/method descriptors so a plain binary name
            // is not mistaken for an unmapped descriptor reference.
            if !value.starts_with('L') && !value.starts_with('(') && !value.starts_with('[') {
                continue;
            }
            let without_mapped_types = [
                "Lnet/minecraft/client/Minecraft;",
                "Lnet/minecraft/client/Options;",
                "Lnet/minecraft/client/KeyMapping;",
                "Lnet/minecraft/client/KeyMapping$Category;",
                "Lnet/minecraft/client/multiplayer/ClientLevel;",
                "Lnet/minecraft/world/level/Level;",
                "Lnet/minecraft/world/level/block/Block;",
                "Lnet/minecraft/world/level/block/state/BlockState;",
                "Lnet/minecraft/client/player/LocalPlayer;",
                "Lnet/minecraft/client/gui/Gui;",
                "Lnet/minecraft/client/gui/Hud;",
                "Lnet/minecraft/world/entity/Entity;",
                "Lnet/minecraft/world/entity/LivingEntity;",
                "Lnet/minecraft/world/entity/player/Player;",
                "Lnet/minecraft/core/BlockPos;",
                "Lnet/minecraft/world/phys/Vec3;",
                "Lnet/minecraft/network/chat/Component;",
                "Lnet/minecraft/network/chat/MutableComponent;",
                "Lnet/minecraft/resources/Identifier;",
                "Lcom/mojang/blaze3d/platform/InputConstants;",
                "Lcom/mojang/blaze3d/platform/InputConstants$Type;",
                "Lcom/mojang/blaze3d/platform/InputConstants$Key;",
            ]
            .iter()
            .fold(value.clone(), |descriptor, mapped_type| {
                descriptor.replace(mapped_type, "")
            });
            if without_mapped_types.contains("net/minecraft/") {
                return Some("This mod references Minecraft APIs outside the mapped client tick, player/environment and bounded nearby-block snapshots, keyboard key-mapping, system-message, and action-bar subsets.".to_owned());
            }
            if without_mapped_types.contains("com/mojang/blaze3d/") {
                return Some("Mojang rendering API types are outside the mapped keyboard-key subset; graphics integrations such as Sodium cannot run in Rust Horizon.".to_owned());
            }
        }
    }

    for entry in &pool {
        if let ClassPoolEntry::Class(name_index) = entry {
            let Some(name) = utf8(*name_index) else {
                return Some(unsupported());
            };
            if name.starts_with("net/minecraft/") && !allowed_minecraft_class(name) {
                return Some(format!("Minecraft API `{name}` is outside the mapped client tick, player/environment snapshots, bounded nearby block reads, keyboard key-mapping, system-message, and action-bar subsets."));
            }
            if name.starts_with("org/spongepowered/asm/mixin") {
                return Some("Fabric Mixin bytecode is unsupported by the Rust Horizon bridge.".to_owned());
            }
            if name.starts_with("com/mojang/blaze3d/")
                && !matches!(
                    name,
                    "com/mojang/blaze3d/platform/InputConstants"
                        | "com/mojang/blaze3d/platform/InputConstants$Type"
                        | "com/mojang/blaze3d/platform/InputConstants$Key"
                )
            {
                return Some(format!("Mojang graphics API `{name}` is outside the mapped keyboard-key subset; rendering integrations such as Sodium are unsupported."));
            }
            if name.starts_with("com/mojang/blaze3d/platform/InputConstants")
                && !matches!(
                    name,
                    "com/mojang/blaze3d/platform/InputConstants"
                        | "com/mojang/blaze3d/platform/InputConstants$Type"
                        | "com/mojang/blaze3d/platform/InputConstants$Key"
                )
            {
                return Some(format!("Input API `{name}` is outside the mapped keyboard key subset."));
            }
            if name.starts_with("net/fabricmc/fabric/api/") && !allowed_fabric_class(name) {
                return Some(format!("Fabric API `{name}` is outside the mapped client tick and keyboard key-mapping subset."));
            }
        }
    }

    for entry in &pool {
        if let ClassPoolEntry::MemberRef(tag, owner_index, name_type_index) = entry {
            let (Some(owner), Some(ClassPoolEntry::NameAndType(name_index, descriptor_index))) =
                (class_name(*owner_index), pool.get(*name_type_index as usize))
            else {
                return Some(unsupported());
            };
            let (Some(name), Some(descriptor)) = (utf8(*name_index), utf8(*descriptor_index)) else {
                return Some(unsupported());
            };
            if owner.starts_with("net/minecraft/")
                && !mapped_minecraft_member(*tag, owner, name, descriptor)
            {
                return Some(format!("Minecraft API `{owner}.{name}{descriptor}` has no Horizon mapping."));
            }
            if owner == "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents"
                && (*tag != 9 || !matches!(name, "START_CLIENT_TICK" | "END_CLIENT_TICK"))
            {
                return Some("Only ClientTickEvents.START_CLIENT_TICK and END_CLIENT_TICK are mapped.".to_owned());
            }
            if owner == "net/fabricmc/fabric/api/event/Event"
                && (*tag != 11 || !matches!(name, "register" | "invoker"))
            {
                return Some("Only Fabric Event.register and Event.invoker are mapped.".to_owned());
            }
            if owner == "net/fabricmc/fabric/api/client/keymapping/v1/KeyMappingHelper"
                && !mapped_fabric_key_mapping_member(*tag, name, descriptor)
            {
                return Some(format!("Fabric key-mapping API `{name}{descriptor}` is not mapped."));
            }
            if owner == "net/minecraft/client/KeyMapping"
                && !mapped_key_mapping_member(*tag, name, descriptor)
            {
                return Some(format!("Minecraft key mapping API `{name}{descriptor}` is not mapped."));
            }
            if owner == "net/minecraft/client/KeyMapping$Category"
                && !mapped_minecraft_member(*tag, owner, name, descriptor)
            {
                return Some(format!("Minecraft key category `{name}` is not mapped."));
            }
            if owner == "com/mojang/blaze3d/platform/InputConstants"
                && !mapped_input_constants_member(*tag, name, descriptor)
            {
                return Some(format!("Input constant `{name}{descriptor}` is not mapped."));
            }
            if owner == "com/mojang/blaze3d/platform/InputConstants$Type"
                && !matches!(
                    (*tag, name, descriptor),
                    (9, "KEYBOARD", "Lcom/mojang/blaze3d/platform/InputConstants$Type;")
                        | (10, "create", "(I)Lcom/mojang/blaze3d/platform/InputConstants$Key;")
                )
            {
                return Some(format!("Input key type member `{name}{descriptor}` is not mapped."));
            }
            if owner == "com/mojang/blaze3d/platform/InputConstants$Key"
                && !matches!(
                    (*tag, name, descriptor),
                    (10, "getValue", "()I")
                        | (10, "getType", "()Lcom/mojang/blaze3d/platform/InputConstants$Type;")
                        | (10, "getName", "()Ljava/lang/String;")
                )
            {
                return Some(format!("Input key member `{name}{descriptor}` is not mapped."));
            }
            if owner.starts_with("net/fabricmc/fabric/api/")
                && owner != "net/fabricmc/fabric/api/event/Event"
                && owner != "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents"
                && owner != "net/fabricmc/fabric/api/client/keymapping/v1/KeyMappingHelper"
            {
                return Some(format!("Fabric API `{owner}.{name}{descriptor}` is not mapped."));
            }
        }
    }
    None
}

fn mapped_fabric_key_mapping_member(tag: u8, name: &str, descriptor: &str) -> bool {
    matches!(
        (tag, name, descriptor),
        (10, "registerKeyMapping", "(Lnet/minecraft/client/KeyMapping;)Lnet/minecraft/client/KeyMapping;")
            | (10, "getBoundKeyOf", "(Lnet/minecraft/client/KeyMapping;)Lcom/mojang/blaze3d/platform/InputConstants$Key;")
    )
}

fn mapped_key_mapping_member(tag: u8, name: &str, descriptor: &str) -> bool {
    matches!(
        (tag, name, descriptor),
        (10, "<init>", "(Ljava/lang/String;Lcom/mojang/blaze3d/platform/InputConstants$Type;ILnet/minecraft/client/KeyMapping$Category;)V")
            | (10, "getName", "()Ljava/lang/String;")
            | (10, "getKey", "()Lcom/mojang/blaze3d/platform/InputConstants$Key;")
            | (10, "getDefaultKey", "()Lcom/mojang/blaze3d/platform/InputConstants$Key;")
            | (10, "getCategory", "()Lnet/minecraft/client/KeyMapping$Category;")
            | (10, "setDown", "(Z)V")
            | (10, "set", "(Lcom/mojang/blaze3d/platform/InputConstants$Key;Z)V")
            | (10, "isDown", "()Z")
            | (10, "consumeClick", "()Z")
            | (9, "MOVEMENT" | "MISC" | "MULTIPLAYER" | "GAMEPLAY" | "INVENTORY" | "CREATIVE" | "SPECTATOR" | "DEBUG" | "SYSTEM", "Lnet/minecraft/client/KeyMapping$Category;")
    )
}

fn mapped_input_constants_member(tag: u8, name: &str, descriptor: &str) -> bool {
    if tag != 9 || descriptor != "I" {
        return false;
    }
    matches!(
        name,
        "KEY_0" | "KEY_1" | "KEY_2" | "KEY_3" | "KEY_4" | "KEY_5" | "KEY_6" | "KEY_7" | "KEY_8" | "KEY_9"
            | "KEY_A" | "KEY_B" | "KEY_C" | "KEY_D" | "KEY_E" | "KEY_F" | "KEY_G" | "KEY_H" | "KEY_I" | "KEY_J" | "KEY_K" | "KEY_L" | "KEY_M"
            | "KEY_N" | "KEY_O" | "KEY_P" | "KEY_Q" | "KEY_R" | "KEY_S" | "KEY_T" | "KEY_U" | "KEY_V" | "KEY_W" | "KEY_X" | "KEY_Y" | "KEY_Z"
            | "KEY_ADD" | "KEY_APOSTROPHE" | "KEY_BACKSLASH" | "KEY_BACKSPACE" | "KEY_CAPSLOCK" | "KEY_COMMA"
            | "KEY_DELETE" | "KEY_DOWN" | "KEY_END" | "KEY_EQUALS" | "KEY_ESCAPE"
            | "KEY_F1" | "KEY_F2" | "KEY_F3" | "KEY_F4" | "KEY_F5" | "KEY_F6" | "KEY_F7" | "KEY_F8" | "KEY_F9" | "KEY_F10" | "KEY_F11" | "KEY_F12"
            | "KEY_F13" | "KEY_F14" | "KEY_F15" | "KEY_F16" | "KEY_F17" | "KEY_F18" | "KEY_F19" | "KEY_F20" | "KEY_F21" | "KEY_F22" | "KEY_F23" | "KEY_F24"
            | "KEY_GRAVE" | "KEY_HOME" | "KEY_INSERT" | "KEY_LALT" | "KEY_LBRACKET" | "KEY_LCONTROL" | "KEY_LEFT" | "KEY_LGUI" | "KEY_LSHIFT"
            | "KEY_MINUS" | "KEY_MULTIPLY" | "KEY_NUMLOCK"
            | "KEY_NUMPAD0" | "KEY_NUMPAD1" | "KEY_NUMPAD2" | "KEY_NUMPAD3" | "KEY_NUMPAD4" | "KEY_NUMPAD5" | "KEY_NUMPAD6" | "KEY_NUMPAD7" | "KEY_NUMPAD8" | "KEY_NUMPAD9"
            | "KEY_NUMPADCOMMA" | "KEY_NUMPADENTER" | "KEY_NUMPADEQUALS"
            | "KEY_PAGEDOWN" | "KEY_PAGEUP" | "KEY_PAUSE" | "KEY_PERIOD" | "KEY_PRINTSCREEN"
            | "KEY_RALT" | "KEY_RBRACKET" | "KEY_RCONTROL" | "KEY_RETURN" | "KEY_RGUI" | "KEY_RIGHT" | "KEY_RSHIFT" | "KEY_SCROLLLOCK"
            | "KEY_SEMICOLON" | "KEY_SLASH" | "KEY_SPACE" | "KEY_TAB" | "KEY_UP"
    )
}

fn mapped_minecraft_member(tag: u8, owner: &str, name: &str, descriptor: &str) -> bool {
    if owner == "net/minecraft/world/level/Level" {
        return matches!(
            (tag, name, descriptor),
            (10, "getBlockState", "(Lnet/minecraft/core/BlockPos;)Lnet/minecraft/world/level/block/state/BlockState;")
                | (10, "getDayTime" | "getGameTime", "()J")
                | (10, "getMinBuildHeight" | "getMaxBuildHeight", "()I")
                | (10, "isRaining" | "isThundering", "()Z")
                | (10, "getRainLevel" | "getThunderLevel", "(F)F")
        );
    }
    if owner == "net/minecraft/world/level/block/state/BlockState" {
        return matches!(
            (tag, name, descriptor),
            (10, "getBlock", "()Lnet/minecraft/world/level/block/Block;")
                | (10, "isAir" | "isSolid" | "isSolidRender", "()Z")
        );
    }
    if owner == "net/minecraft/world/level/block/Block" {
        return matches!(
            (tag, name, descriptor),
            (10, "getDescriptionId" | "toString", "()Ljava/lang/String;")
        );
    }
    if owner == "net/minecraft/core/BlockPos" {
        return matches!(
            (tag, name, descriptor),
            (10, "<init>", "(III)V")
                | (10, "getX", "()I")
                | (10, "getY", "()I")
                | (10, "getZ", "()I")
        );
    }
    if owner == "net/minecraft/world/phys/Vec3" {
        return matches!(
            (tag, name, descriptor),
            (9, "x" | "y" | "z", "D")
                | (10, "<init>", "(DDD)V")
                | (10, "add", "(DDD)Lnet/minecraft/world/phys/Vec3;")
                | (10, "length" | "lengthSqr" | "horizontalDistance" | "horizontalDistanceSqr", "()D")
        );
    }
    if owner == "net/minecraft/client/KeyMapping" {
        return mapped_key_mapping_member(tag, name, descriptor);
    }
    if owner == "net/minecraft/client/KeyMapping$Category" {
        return matches!(
            (tag, name, descriptor),
            (10, "<init>", "(Lnet/minecraft/resources/Identifier;)V")
                | (10, "register", "(Lnet/minecraft/resources/Identifier;)Lnet/minecraft/client/KeyMapping$Category;")
                | (10, "id", "()Lnet/minecraft/resources/Identifier;")
                | (9, "MOVEMENT" | "MISC" | "MULTIPLAYER" | "GAMEPLAY" | "INVENTORY" | "CREATIVE" | "SPECTATOR" | "DEBUG" | "SYSTEM", "Lnet/minecraft/client/KeyMapping$Category;")
        );
    }
    if owner == "net/minecraft/resources/Identifier" {
        return matches!(
            (tag, name, descriptor),
            (10, "fromNamespaceAndPath", "(Ljava/lang/String;Ljava/lang/String;)Lnet/minecraft/resources/Identifier;")
                | (10, "getNamespace" | "getPath", "()Ljava/lang/String;")
                | (10, "toString", "()Ljava/lang/String;")
        );
    }
    match (owner, name, descriptor) {
        (
            "net/minecraft/client/Minecraft",
            "getInstance",
            "()Lnet/minecraft/client/Minecraft;",
        ) => tag == 10,
        (
            "net/minecraft/client/Minecraft",
            "player",
            "Lnet/minecraft/client/player/LocalPlayer;",
        ) => tag == 9,
        (
            "net/minecraft/client/Minecraft",
            "options",
            "Lnet/minecraft/client/Options;",
        ) => tag == 9,
        (
            "net/minecraft/client/Options",
            "keyUp" | "keyDown" | "keyLeft" | "keyRight" | "keyJump" | "keyShift" | "keySprint",
            "Lnet/minecraft/client/KeyMapping;",
        ) => tag == 9,
        (
            "net/minecraft/client/Minecraft",
            "level",
            "Lnet/minecraft/client/multiplayer/ClientLevel;",
        ) => tag == 9,
        (
            "net/minecraft/client/multiplayer/ClientLevel",
            "getOverworldClockTime",
            "()J",
        ) => tag == 10,
        (
            "net/minecraft/client/Minecraft",
            "gui",
            "Lnet/minecraft/client/gui/Gui;",
        ) => tag == 9,
        (
            "net/minecraft/client/gui/Gui",
            "hud",
            "Lnet/minecraft/client/gui/Hud;",
        ) => tag == 9,
        (
            "net/minecraft/client/gui/Hud",
            "setOverlayMessage",
            "(Lnet/minecraft/network/chat/Component;Z)V",
        ) => tag == 10,
        (
            "net/minecraft/network/chat/Component",
            "literal",
            "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
        ) => matches!(tag, 10 | 11),
        (
            "net/minecraft/network/chat/Component",
            "getString",
            "()Ljava/lang/String;",
        ) => matches!(tag, 10 | 11),
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "getName",
            "()Lnet/minecraft/network/chat/Component;",
        ) => matches!(tag, 10 | 11),
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "getX" | "getY" | "getZ",
            "()D",
        ) => matches!(tag, 10 | 11),
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "blockPosition",
            "()Lnet/minecraft/core/BlockPos;",
        ) => matches!(tag, 10 | 11),
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "getYRot" | "getXRot",
            "()F",
        ) => matches!(tag, 10 | 11),
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "getDeltaMovement",
            "()Lnet/minecraft/world/phys/Vec3;",
        ) => matches!(tag, 10 | 11),
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "setDeltaMovement",
            "(Lnet/minecraft/world/phys/Vec3;)V",
        ) => tag == 10,
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "isSprinting" | "isOnGround",
            "()Z",
        ) => matches!(tag, 10 | 11),
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "sendSystemMessage",
            "(Lnet/minecraft/network/chat/Component;)V",
        ) => tag == 10,
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "setSprinting",
            "(Z)V",
        ) => tag == 10,
        (
            "net/minecraft/world/entity/Entity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "setYRot" | "setXRot",
            "(F)V",
        ) => tag == 10,
        (
            "net/minecraft/world/entity/LivingEntity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "getHealth",
            "()F",
        ) => matches!(tag, 10 | 11),
        (
            "net/minecraft/world/entity/LivingEntity"
            | "net/minecraft/world/entity/player/Player"
            | "net/minecraft/client/player/LocalPlayer",
            "isAlive",
            "()Z",
        ) => matches!(tag, 10 | 11),
        _ => false,
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FabricCodeStageLedger {
    files: BTreeMap<String, String>,
}

fn metadata_field_nonempty(metadata: &Value, key: &str) -> bool {
    match metadata.get(key) {
        Some(Value::Object(value)) => !value.is_empty(),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::String(value)) => !value.trim().is_empty(),
        _ => false,
    }
}

fn is_default_adapter_entrypoint(value: &Value) -> bool {
    fn is_class_path(value: &Value) -> bool {
        value.as_str().is_some_and(|class_path| {
            !class_path.trim().is_empty() && !class_path.contains(':')
        })
    }
    match value {
        Value::String(_) => is_class_path(value),
        Value::Array(values) => !values.is_empty() && values.iter().all(is_class_path),
        _ => false,
    }
}

fn entrypoint_class_paths(value: &Value) -> Vec<&str> {
    match value {
        Value::String(class_path) => vec![class_path],
        Value::Array(values) => values.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn valid_fabric_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn horizon_fabric_mod_id(fabric_id: &str, source_sha1: &str) -> String {
    let normalized = fabric_id.replace('-', "_");
    let short_hash = &source_sha1[..8.min(source_sha1.len())];
    let max_prefix = 64usize.saturating_sub("fabric__".len() + short_hash.len());
    let prefix = normalized.chars().take(max_prefix).collect::<String>();
    format!("fabric_{prefix}_{short_hash}")
}

fn read_fabric_content_package(path: &Path) -> Result<FabricContentPackage> {
    let inspection = inspect_fabric_jar(path)?;
    if inspection.status.state != "compatible" {
        return Err(LauncherError::Unsupported(inspection.status.detail));
    }
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        LauncherError::Unsupported(format!("the Fabric JAR is not a valid archive: {error}"))
    })?;
    let mut files = Vec::with_capacity(inspection.content_entries.len());
    for (index, name, expected_size) in inspection.content_entries {
        let mut entry = archive.by_index(index).map_err(|error| {
            LauncherError::Unsupported(format!("cannot read {name} from the Fabric JAR: {error}"))
        })?;
        let mut bytes = Vec::with_capacity(expected_size.min(1024 * 1024) as usize);
        (&mut entry)
            .take(MAX_FABRIC_CONTENT_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != expected_size || bytes.len() as u64 > MAX_FABRIC_CONTENT_FILE_BYTES {
            return Err(LauncherError::Unsupported(format!(
                "{name} changed size while being read"
            )));
        }
        files.push((name, bytes));
    }
    let source_sha1 = sha1_file(path)?;
    Ok(FabricContentPackage {
        id: inspection.status.mod_id.unwrap_or_default(),
        name: inspection.status.name.unwrap_or_default(),
        version: inspection.status.version.unwrap_or_else(|| "unknown".to_owned()),
        description: inspection.description,
        authors: inspection.authors,
        namespaces: inspection.namespaces,
        files,
        source_sha1,
    })
}

fn sha1_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha1::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn sha1_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha1::digest(bytes))
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
        fabric_bridge_status, horizon_fabric_mod_id, inspect_fabric_code_jar, inspect_fabric_jar,
        list_mods, manifest_value, prepare_fabric_bridge, prepare_fabric_code_bridge,
        set_fabric_bridge_enabled, stage_preparer_inputs, unsupported_class_api,
        mapped_minecraft_member, mapped_fabric_key_mapping_member, mapped_key_mapping_member,
        mapped_input_constants_member, valid_fabric_id, valid_offline_player_name,
        validate_staged_name, write_lines_atomic,
    };
    use crate::models::{Instance, InstanceEngine};
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use uuid::Uuid;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn test_root(label: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn legacy_fabric_bridge_settings_keep_the_new_code_bridge_off() {
        let settings: super::HorizonFabricBridgeSettings =
            serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(settings.enabled);
        assert!(!settings.code_enabled);
    }

    fn write_fabric_test_jar(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let metadata = br#"{"schemaVersion":1,"id":"sample_resources","version":"1.0.0","name":"Sample Resources","description":"Static test content","authors":["Test Author"],"depends":{"minecraft":"*","fabricloader":">=0.16.0"}}"#;
        let mut archive = ZipWriter::new(File::create(path).unwrap());
        archive
            .start_file("fabric.mod.json", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(metadata).unwrap();
        for (name, bytes) in entries {
            archive
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
    }

    fn cp_utf8(value: &str) -> Vec<u8> {
        let mut bytes = vec![1];
        bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
        bytes.extend_from_slice(value.as_bytes());
        bytes
    }

    fn cp_class(name_index: u16) -> Vec<u8> {
        let mut bytes = vec![7];
        bytes.extend_from_slice(&name_index.to_be_bytes());
        bytes
    }

    fn cp_name_and_type(name_index: u16, descriptor_index: u16) -> Vec<u8> {
        let mut bytes = vec![12];
        bytes.extend_from_slice(&name_index.to_be_bytes());
        bytes.extend_from_slice(&descriptor_index.to_be_bytes());
        bytes
    }

    fn cp_member_ref(tag: u8, class_index: u16, name_type_index: u16) -> Vec<u8> {
        let mut bytes = vec![tag];
        bytes.extend_from_slice(&class_index.to_be_bytes());
        bytes.extend_from_slice(&name_type_index.to_be_bytes());
        bytes
    }

    fn push_utf8(pool: &mut Vec<Vec<u8>>, value: &str) -> u16 {
        let index = (pool.len() + 1) as u16;
        pool.push(cp_utf8(value));
        index
    }

    fn push_class(pool: &mut Vec<Vec<u8>>, name: &str) -> u16 {
        let name_index = push_utf8(pool, name);
        let index = (pool.len() + 1) as u16;
        pool.push(cp_class(name_index));
        index
    }

    fn push_member(pool: &mut Vec<Vec<u8>>, tag: u8, owner: &str, name: &str, descriptor: &str) {
        let owner_index = push_class(pool, owner);
        let name_index = push_utf8(pool, name);
        let descriptor_index = push_utf8(pool, descriptor);
        let name_type_index = (pool.len() + 1) as u16;
        pool.push(cp_name_and_type(name_index, descriptor_index));
        pool.push(cp_member_ref(tag, owner_index, name_type_index));
    }

    // A bounded constant-pool sample for scanner tests; it is not launched as a JVM class.
    fn fabric_tick_class(field_name: &str, extra_class: Option<&str>) -> Vec<u8> {
        let callback_type = if field_name == "START_CLIENT_TICK" { "StartTick" } else { "EndTick" };
        let callback_class = format!(
            "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents${callback_type}"
        );
        let mut pool = vec![
            cp_utf8("dev/example/Probe"),
            cp_class(1),
            cp_utf8("java/lang/Object"),
            cp_class(3),
            cp_utf8("net/fabricmc/api/ClientModInitializer"),
            cp_class(5),
            cp_utf8("net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents"),
            cp_class(7),
            cp_utf8(field_name),
            cp_utf8("Lnet/fabricmc/fabric/api/event/Event;"),
            cp_name_and_type(9, 10),
            cp_member_ref(9, 8, 11),
            cp_utf8(&callback_class),
            cp_class(13),
            cp_utf8("Lnet/minecraft/client/Minecraft;"),
        ];
        if let Some(class_name) = extra_class {
            let name_index = (pool.len() + 1) as u16;
            pool.push(cp_utf8(class_name));
            pool.push(cp_class(name_index));
        }
        let mut bytes = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
        bytes.extend_from_slice(&((pool.len() + 1) as u16).to_be_bytes());
        for entry in pool {
            bytes.extend_from_slice(&entry);
        }
        bytes
    }

    fn fabric_overlay_class() -> Vec<u8> {
        let pool = [
            cp_utf8("dev/example/Probe"),
            cp_class(1),
            cp_utf8("java/lang/Object"),
            cp_class(3),
            cp_utf8("net/minecraft/client/Minecraft"),
            cp_class(5),
            cp_utf8("()Lnet/minecraft/client/Minecraft;"),
            cp_utf8("getInstance"),
            cp_name_and_type(8, 7),
            cp_member_ref(10, 6, 9),
            cp_utf8("Lnet/minecraft/client/gui/Gui;"),
            cp_utf8("gui"),
            cp_name_and_type(12, 11),
            cp_member_ref(9, 6, 13),
            cp_utf8("net/minecraft/client/gui/Gui"),
            cp_class(15),
            cp_utf8("Lnet/minecraft/client/gui/Hud;"),
            cp_utf8("hud"),
            cp_name_and_type(18, 17),
            cp_member_ref(9, 16, 19),
            cp_utf8("net/minecraft/client/gui/Hud"),
            cp_class(21),
            cp_utf8("Lnet/minecraft/network/chat/Component;"),
            cp_utf8("setOverlayMessage"),
            cp_utf8("(Lnet/minecraft/network/chat/Component;Z)V"),
            cp_name_and_type(24, 25),
            cp_member_ref(10, 22, 26),
            cp_utf8("net/minecraft/network/chat/Component"),
            cp_class(28),
            cp_utf8("net/minecraft/network/chat/MutableComponent"),
            cp_class(30),
            cp_utf8("(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;"),
            cp_utf8("literal"),
            cp_name_and_type(33, 32),
            cp_member_ref(11, 29, 34),
            cp_utf8("getString"),
            cp_utf8("()Ljava/lang/String;"),
            cp_name_and_type(36, 37),
            cp_member_ref(11, 29, 38),
        ];
        let mut bytes = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
        bytes.extend_from_slice(&((pool.len() + 1) as u16).to_be_bytes());
        for entry in pool {
            bytes.extend_from_slice(&entry);
        }
        bytes
    }

    fn fabric_world_clock_class(clock_method: &str) -> Vec<u8> {
        let pool = [
            cp_utf8("dev/example/Probe"),
            cp_class(1),
            cp_utf8("java/lang/Object"),
            cp_class(3),
            cp_utf8("net/minecraft/client/Minecraft"),
            cp_class(5),
            cp_utf8("Lnet/minecraft/client/multiplayer/ClientLevel;"),
            cp_utf8("level"),
            cp_name_and_type(8, 7),
            cp_member_ref(9, 6, 9),
            cp_utf8("net/minecraft/client/multiplayer/ClientLevel"),
            cp_class(11),
            cp_utf8(clock_method),
            cp_utf8("()J"),
            cp_name_and_type(13, 14),
            cp_member_ref(10, 12, 15),
        ];
        let mut bytes = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
        bytes.extend_from_slice(&((pool.len() + 1) as u16).to_be_bytes());
        for entry in pool {
            bytes.extend_from_slice(&entry);
        }
        bytes
    }

    fn fabric_level_method_class(method: &str, descriptor: &str) -> Vec<u8> {
        let mut pool = vec![
            cp_utf8("dev/example/EnvironmentProbe"),
            cp_class(1),
            cp_utf8("java/lang/Object"),
            cp_class(3),
        ];
        push_class(&mut pool, "net/minecraft/world/level/Level");
        push_member(
            &mut pool,
            10,
            "net/minecraft/world/level/Level",
            method,
            descriptor,
        );
        let mut bytes = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
        bytes.extend_from_slice(&((pool.len() + 1) as u16).to_be_bytes());
        for entry in pool {
            bytes.extend_from_slice(&entry);
        }
        bytes
    }

    fn fabric_key_mapping_class() -> Vec<u8> {
        let mut pool = vec![
            cp_utf8("dev/example/KeyProbe"),
            cp_class(1),
            cp_utf8("java/lang/Object"),
            cp_class(3),
        ];
        for name in [
            "net/fabricmc/api/ClientModInitializer",
            "net/fabricmc/fabric/api/event/Event",
            "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents",
            "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents$EndTick",
            "net/fabricmc/fabric/api/client/keymapping/v1/KeyMappingHelper",
            "net/minecraft/client/Minecraft",
            "net/minecraft/client/Options",
            "net/minecraft/client/KeyMapping",
            "net/minecraft/client/KeyMapping$Category",
            "net/minecraft/resources/Identifier",
            "com/mojang/blaze3d/platform/InputConstants",
            "com/mojang/blaze3d/platform/InputConstants$Type",
            "com/mojang/blaze3d/platform/InputConstants$Key",
        ] {
            push_class(&mut pool, name);
        }
        push_member(
            &mut pool,
            9,
            "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents",
            "END_CLIENT_TICK",
            "Lnet/fabricmc/fabric/api/event/Event;",
        );
        push_member(
            &mut pool,
            11,
            "net/fabricmc/fabric/api/event/Event",
            "register",
            "(Ljava/lang/Object;)V",
        );
        push_member(
            &mut pool,
            9,
            "net/minecraft/client/Minecraft",
            "player",
            "Lnet/minecraft/client/player/LocalPlayer;",
        );
        push_member(
            &mut pool,
            9,
            "net/minecraft/client/Minecraft",
            "options",
            "Lnet/minecraft/client/Options;",
        );
        push_member(
            &mut pool,
            9,
            "net/minecraft/client/Options",
            "keySprint",
            "Lnet/minecraft/client/KeyMapping;",
        );
        push_member(
            &mut pool,
            9,
            "com/mojang/blaze3d/platform/InputConstants",
            "KEY_P",
            "I",
        );
        push_member(
            &mut pool,
            9,
            "com/mojang/blaze3d/platform/InputConstants$Type",
            "KEYBOARD",
            "Lcom/mojang/blaze3d/platform/InputConstants$Type;",
        );
        push_member(
            &mut pool,
            9,
            "net/minecraft/client/KeyMapping$Category",
            "MISC",
            "Lnet/minecraft/client/KeyMapping$Category;",
        );
        push_member(
            &mut pool,
            10,
            "net/minecraft/resources/Identifier",
            "fromNamespaceAndPath",
            "(Ljava/lang/String;Ljava/lang/String;)Lnet/minecraft/resources/Identifier;",
        );
        push_member(
            &mut pool,
            10,
            "net/minecraft/client/KeyMapping$Category",
            "register",
            "(Lnet/minecraft/resources/Identifier;)Lnet/minecraft/client/KeyMapping$Category;",
        );
        push_member(
            &mut pool,
            10,
            "net/minecraft/client/KeyMapping$Category",
            "id",
            "()Lnet/minecraft/resources/Identifier;",
        );
        push_member(
            &mut pool,
            10,
            "net/minecraft/resources/Identifier",
            "toString",
            "()Ljava/lang/String;",
        );
        for method in ["getNamespace", "getPath"] {
            push_member(
                &mut pool,
                10,
                "net/minecraft/resources/Identifier",
                method,
                "()Ljava/lang/String;",
            );
        }
        push_member(
            &mut pool,
            10,
            "net/minecraft/client/KeyMapping",
            "<init>",
            "(Ljava/lang/String;Lcom/mojang/blaze3d/platform/InputConstants$Type;ILnet/minecraft/client/KeyMapping$Category;)V",
        );
        push_member(
            &mut pool,
            10,
            "net/fabricmc/fabric/api/client/keymapping/v1/KeyMappingHelper",
            "registerKeyMapping",
            "(Lnet/minecraft/client/KeyMapping;)Lnet/minecraft/client/KeyMapping;",
        );
        push_member(
            &mut pool,
            10,
            "net/fabricmc/fabric/api/client/keymapping/v1/KeyMappingHelper",
            "getBoundKeyOf",
            "(Lnet/minecraft/client/KeyMapping;)Lcom/mojang/blaze3d/platform/InputConstants$Key;",
        );
        push_member(&mut pool, 10, "net/minecraft/client/KeyMapping", "isDown", "()Z");
        push_member(&mut pool, 10, "net/minecraft/client/KeyMapping", "consumeClick", "()Z");
        push_member(
            &mut pool,
            10,
            "net/minecraft/client/KeyMapping",
            "getDefaultKey",
            "()Lcom/mojang/blaze3d/platform/InputConstants$Key;",
        );
        push_member(&mut pool, 10, "net/minecraft/client/KeyMapping", "setDown", "(Z)V");
        push_member(
            &mut pool,
            10,
            "net/minecraft/client/KeyMapping",
            "set",
            "(Lcom/mojang/blaze3d/platform/InputConstants$Key;Z)V",
        );
        push_member(
            &mut pool,
            10,
            "com/mojang/blaze3d/platform/InputConstants$Key",
            "getValue",
            "()I",
        );
        let mut bytes = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
        bytes.extend_from_slice(&((pool.len() + 1) as u16).to_be_bytes());
        for entry in pool {
            bytes.extend_from_slice(&entry);
        }
        bytes
    }

    fn fabric_player_class() -> Vec<u8> {
        let mut pool = vec![
            cp_utf8("dev/example/PlayerProbe"),
            cp_class(1),
            cp_utf8("java/lang/Object"),
            cp_class(3),
        ];
        push_class(&mut pool, "net/fabricmc/api/ClientModInitializer");
        push_class(&mut pool, "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents");
        push_class(&mut pool, "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents$EndTick");
        push_class(&mut pool, "net/fabricmc/fabric/api/event/Event");
        push_class(&mut pool, "net/minecraft/client/Minecraft");
        push_class(&mut pool, "net/minecraft/client/Options");
        push_class(&mut pool, "net/minecraft/client/player/LocalPlayer");
        push_class(&mut pool, "net/minecraft/world/entity/Entity");
        push_class(&mut pool, "net/minecraft/world/entity/LivingEntity");
        push_class(&mut pool, "net/minecraft/world/entity/player/Player");
        push_class(&mut pool, "net/minecraft/core/BlockPos");
        push_class(&mut pool, "net/minecraft/world/phys/Vec3");
        push_class(&mut pool, "net/minecraft/network/chat/Component");

        push_member(
            &mut pool,
            9,
            "net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents",
            "END_CLIENT_TICK",
            "Lnet/fabricmc/fabric/api/event/Event;",
        );
        push_member(
            &mut pool,
            10,
            "net/minecraft/client/Minecraft",
            "getInstance",
            "()Lnet/minecraft/client/Minecraft;",
        );
        push_member(
            &mut pool,
            9,
            "net/minecraft/client/Minecraft",
            "player",
            "Lnet/minecraft/client/player/LocalPlayer;",
        );
        push_member(
            &mut pool,
            9,
            "net/minecraft/client/Minecraft",
            "options",
            "Lnet/minecraft/client/Options;",
        );
        for field in ["keyUp", "keyShift", "keySprint"] {
            push_member(
                &mut pool,
                9,
                "net/minecraft/client/Options",
                field,
                "Lnet/minecraft/client/KeyMapping;",
            );
        }
        push_member(&mut pool, 10, "net/minecraft/client/KeyMapping", "isDown", "()Z");
        push_member(&mut pool, 10, "net/minecraft/client/KeyMapping", "consumeClick", "()Z");
        push_member(
            &mut pool,
            10,
            "net/minecraft/client/KeyMapping",
            "getDefaultKey",
            "()Lcom/mojang/blaze3d/platform/InputConstants$Key;",
        );
        push_member(
            &mut pool,
            10,
            "net/minecraft/client/KeyMapping",
            "setDown",
            "(Z)V",
        );
        push_member(
            &mut pool,
            10,
            "net/minecraft/client/KeyMapping",
            "set",
            "(Lcom/mojang/blaze3d/platform/InputConstants$Key;Z)V",
        );
        for method in ["getX", "getY", "getZ"] {
            push_member(&mut pool, 10, "net/minecraft/world/entity/Entity", method, "()D");
        }
        push_member(
            &mut pool,
            10,
            "net/minecraft/world/entity/Entity",
            "blockPosition",
            "()Lnet/minecraft/core/BlockPos;",
        );
        for method in ["getX", "getY", "getZ"] {
            push_member(&mut pool, 10, "net/minecraft/core/BlockPos", method, "()I");
        }
        push_member(
            &mut pool,
            10,
            "net/minecraft/world/entity/Entity",
            "getName",
            "()Lnet/minecraft/network/chat/Component;",
        );
        for method in ["isSprinting", "isOnGround"] {
            push_member(&mut pool, 10, "net/minecraft/world/entity/Entity", method, "()Z");
        }
        for method in ["getYRot", "getXRot"] {
            push_member(&mut pool, 10, "net/minecraft/world/entity/Entity", method, "()F");
        }
        push_member(
            &mut pool,
            10,
            "net/minecraft/world/entity/Entity",
            "getDeltaMovement",
            "()Lnet/minecraft/world/phys/Vec3;",
        );
        push_member(
            &mut pool,
            10,
            "net/minecraft/world/entity/Entity",
            "sendSystemMessage",
            "(Lnet/minecraft/network/chat/Component;)V",
        );
        for component in ["x", "y", "z"] {
            push_member(&mut pool, 9, "net/minecraft/world/phys/Vec3", component, "D");
        }
        push_member(&mut pool, 10, "net/minecraft/world/phys/Vec3", "<init>", "(DDD)V");
        for method in ["length", "lengthSqr", "horizontalDistance", "horizontalDistanceSqr"] {
            push_member(&mut pool, 10, "net/minecraft/world/phys/Vec3", method, "()D");
        }
        push_member(
            &mut pool,
            10,
            "net/minecraft/world/phys/Vec3",
            "add",
            "(DDD)Lnet/minecraft/world/phys/Vec3;",
        );
        push_member(&mut pool, 10, "net/minecraft/world/entity/Entity", "setSprinting", "(Z)V");
        push_member(&mut pool, 10, "net/minecraft/world/entity/Entity", "setYRot", "(F)V");
        push_member(&mut pool, 10, "net/minecraft/world/entity/Entity", "setXRot", "(F)V");
        push_member(
            &mut pool,
            10,
            "net/minecraft/world/entity/LivingEntity",
            "getHealth",
            "()F",
        );
        push_member(
            &mut pool,
            11,
            "net/minecraft/network/chat/Component",
            "getString",
            "()Ljava/lang/String;",
        );

        let mut bytes = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
        bytes.extend_from_slice(&((pool.len() + 1) as u16).to_be_bytes());
        for entry in pool {
            bytes.extend_from_slice(&entry);
        }
        bytes
    }

    fn write_fabric_code_test_jar(path: &std::path::Path, id: &str, class: &[u8]) {
        let metadata = format!(
            r#"{{"schemaVersion":1,"id":"{id}","version":"1.0.0","name":"Tick Probe","environment":"client","depends":{{"minecraft":"26.3","fabricloader":">=0.16.0","fabric-api":"*","fabric-key-mapping-api-v1":"*"}},"entrypoints":{{"client":"dev.example.Probe"}}}}"#
        );
        write_fabric_code_test_jar_with_metadata(path, &metadata, class);
    }

    fn write_fabric_code_test_jar_with_metadata(
        path: &std::path::Path,
        metadata: &str,
        class: &[u8],
    ) {
        let mut archive = ZipWriter::new(File::create(path).unwrap());
        archive
            .start_file("fabric.mod.json", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(metadata.as_bytes()).unwrap();
        archive
            .start_file("dev/example/Probe.class", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(class).unwrap();
        archive.finish().unwrap();
    }

    fn horizon_instance(game_dir: &std::path::Path) -> Instance {
        Instance {
            id: Uuid::new_v4().to_string(),
            name: "Bridge test profile".to_owned(),
            engine: InstanceEngine::Horizon,
            game_dir: game_dir.to_string_lossy().into_owned(),
            version_id: Some("26.3".to_owned()),
            java_path: None,
            horizon_fabric_enabled: false,
            horizon_fabric_version_id: None,
            horizon_fabric_java_path: None,
            memory_mb: 4096,
            created_at: 0,
            last_played_at: None,
        }
    }

    #[test]
    fn fabric_bridge_adapts_static_content_only_when_enabled() {
        let root = test_root("horizon-fabric-bridge-test");
        let game_dir = root.join("game");
        let source_dir = game_dir.join("mods");
        fs::create_dir_all(&source_dir).unwrap();
        write_fabric_test_jar(
            &source_dir.join("sample-resources.jar"),
            &[("assets/sample_resources/lang/en_us.json", br#"{"item.sample":"Sample"}"#)],
        );
        let instance = horizon_instance(&game_dir);

        let before = fabric_bridge_status(&instance).unwrap();
        assert_eq!(before.compatible_count, 1);
        assert!(!before.settings.enabled);
        assert!(list_mods(&instance).unwrap().is_empty());

        set_fabric_bridge_enabled(&instance, true).unwrap();
        let summary = prepare_fabric_bridge(&instance).unwrap();
        assert!(summary.contains("prepared 1 code-free mod(s)"));
        let adapted = list_mods(&instance).unwrap();
        assert_eq!(adapted.len(), 1);
        assert!(adapted[0].valid);
        assert!(adapted[0].enabled);
        let source_hash = super::sha1_file(&source_dir.join("sample-resources.jar")).unwrap();
        let expected_id = horizon_fabric_mod_id("sample_resources", &source_hash);
        assert_eq!(adapted[0].id.as_deref(), Some(expected_id.as_str()));

        set_fabric_bridge_enabled(&instance, false).unwrap();
        assert!(!list_mods(&instance).unwrap()[0].enabled);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fabric_bridge_skips_java_code_and_unsafe_resource_paths() {
        let root = test_root("horizon-fabric-bridge-skip-test");
        let code_jar = root.join("code.jar");
        write_fabric_test_jar(
            &code_jar,
            &[("dev/example/Entry.class", b"not executed")],
        );
        let inspection = inspect_fabric_jar(&code_jar).unwrap();
        assert_eq!(inspection.status.state, "skipped");
        assert!(inspection.status.detail.contains("Java classes"));

        let unsafe_jar = root.join("unsafe.jar");
        write_fabric_test_jar(
            &unsafe_jar,
            &[("assets/sample_resources/../escape.txt", b"not copied")],
        );
        let inspection = inspect_fabric_jar(&unsafe_jar).unwrap();
        assert_eq!(inspection.status.state, "skipped");
        assert!(inspection.status.detail.contains("cannot safely import"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn code_bridge_accepts_mapped_start_and_end_tick_player_snapshot_and_action_bar_subset() {
        let root = test_root("horizon-fabric-code-bridge-test");
        let game_dir = root.join("game");
        let mods_dir = game_dir.join("mods");
        fs::create_dir_all(&mods_dir).unwrap();
        let source = mods_dir.join("probe.jar");
        write_fabric_code_test_jar(&source, "tick_probe", &fabric_tick_class("END_CLIENT_TICK", None));
        let original = fs::read(&source).unwrap();
        let instance = horizon_instance(&game_dir);

        assert_eq!(inspect_fabric_code_jar(&source).unwrap().state, "supported");
        let start_tick = root.join("start_tick.jar");
        write_fabric_code_test_jar(&start_tick, "start_tick_probe", &fabric_tick_class("START_CLIENT_TICK", None));
        assert_eq!(inspect_fabric_code_jar(&start_tick).unwrap().state, "supported");
        let world_clock = root.join("world_clock.jar");
        write_fabric_code_test_jar(
            &world_clock,
            "world_clock_probe",
            &fabric_world_clock_class("getOverworldClockTime"),
        );
        assert_eq!(inspect_fabric_code_jar(&world_clock).unwrap().state, "supported");
        let unmapped_clock = root.join("unmapped_clock.jar");
        write_fabric_code_test_jar(
            &unmapped_clock,
            "unmapped_clock_probe",
            &fabric_world_clock_class("getDayTime"),
        );
        assert_eq!(inspect_fabric_code_jar(&unmapped_clock).unwrap().state, "unsupported");
        let stage_dir = prepare_fabric_code_bridge(&instance).unwrap();
        let staged = fs::read_dir(&stage_dir).unwrap().next().unwrap().unwrap().path();
        assert_eq!(fs::read(staged).unwrap(), original);
        assert_eq!(fs::read(&source).unwrap(), original);

        let unsupported = root.join("unsupported.jar");
        write_fabric_code_test_jar(
            &unsupported,
            "uses_unmapped_minecraft",
            &fabric_tick_class("END_CLIENT_TICK", Some("net/minecraft/client/gui/screens/Screen")),
        );
        assert_eq!(inspect_fabric_code_jar(&unsupported).unwrap().state, "unsupported");

        let renderer = root.join("renderer.jar");
        write_fabric_code_test_jar(
            &renderer,
            "uses_unmapped_renderer",
            &fabric_tick_class("END_CLIENT_TICK", Some("com/mojang/blaze3d/systems/RenderSystem")),
        );
        let renderer_status = inspect_fabric_code_jar(&renderer).unwrap();
        assert_eq!(renderer_status.state, "unsupported");
        assert!(renderer_status.detail.contains("rendering integrations such as Sodium"));

        let mixin = root.join("mixin.jar");
        let mixin_metadata = r#"{"schemaVersion":1,"id":"mixin_probe","version":"1.0.0","environment":"client","depends":{"minecraft":"26.3","fabricloader":">=0.16.0"},"mixins":["mixin_probe.mixins.json"],"entrypoints":{"client":"dev.example.Probe"}}"#;
        write_fabric_code_test_jar_with_metadata(
            &mixin,
            mixin_metadata,
            &fabric_tick_class("END_CLIENT_TICK", None),
        );
        let mixin_status = inspect_fabric_code_jar(&mixin).unwrap();
        assert_eq!(mixin_status.state, "unsupported");
        assert!(mixin_status.detail.contains("Mixin configs"));
        assert!(validate_staged_name("éxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.jar").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn code_bridge_accepts_mapped_keyboard_registration_and_auto_sprint_references() {
        let root = test_root("horizon-fabric-key-mapping-test");
        let source = root.join("key-probe.jar");
        write_fabric_code_test_jar(&source, "key_probe", &fabric_key_mapping_class());
        let inspection = inspect_fabric_code_jar(&source).unwrap();
        assert_eq!(inspection.state, "supported", "{}", inspection.detail);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn code_bridge_leaves_unmanaged_staging_files_untouched_and_refuses_duplicate_ids() {
        let root = test_root("horizon-fabric-code-stage-safety-test");
        let game_dir = root.join("game");
        let mods_dir = game_dir.join("mods");
        fs::create_dir_all(&mods_dir).unwrap();
        let first = mods_dir.join("first.jar");
        let second = mods_dir.join("second.jar");
        let class = fabric_tick_class("END_CLIENT_TICK", None);
        write_fabric_code_test_jar(&first, "duplicate_mod", &class);
        write_fabric_code_test_jar(&second, "duplicate_mod", &class);
        let instance = horizon_instance(&game_dir);
        let status = fabric_bridge_status(&instance).unwrap();
        assert_eq!(status.code_compatible_count, 0);
        assert!(status.code_mods.iter().all(|item| item.state == "unsupported"));

        fs::remove_file(&second).unwrap();
        let stage_dir = prepare_fabric_code_bridge(&instance).unwrap();
        let unmanaged = stage_dir.join("user-added.jar");
        fs::write(&unmanaged, b"keep user file").unwrap();
        assert!(prepare_fabric_code_bridge(&instance).is_err());
        assert_eq!(fs::read(&unmanaged).unwrap(), b"keep user file");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_truncated_or_unmapped_fabric_class_references() {
        assert!(unsupported_class_api(&[0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 2]).is_some());
        assert!(unsupported_class_api(&fabric_tick_class("START_LEVEL_TICK", None)).is_some());
        assert!(unsupported_class_api(&fabric_tick_class(
            "END_CLIENT_TICK",
            Some("org/spongepowered/asm/mixin/Mixin")
        ))
        .is_some_and(|detail| detail.contains("Mixin bytecode")));
        assert!(unsupported_class_api(
            &fabric_tick_class("END_CLIENT_TICK", Some("net/minecraft/client/player/LocalPlayer"))
        )
        .is_none());
        assert_eq!(unsupported_class_api(&fabric_overlay_class()), None);
        assert_eq!(unsupported_class_api(&fabric_player_class()), None);
        assert_eq!(unsupported_class_api(&fabric_world_clock_class("getOverworldClockTime")), None);
        assert_eq!(unsupported_class_api(&fabric_key_mapping_class()), None);
        assert!(unsupported_class_api(&fabric_world_clock_class("getDayTime")).is_some());
        for (method, descriptor) in [
            ("getDayTime", "()J"),
            ("getGameTime", "()J"),
            ("getMinBuildHeight", "()I"),
            ("getMaxBuildHeight", "()I"),
            ("isRaining", "()Z"),
            ("isThundering", "()Z"),
            ("getRainLevel", "(F)F"),
            ("getThunderLevel", "(F)F"),
        ] {
            assert_eq!(
                unsupported_class_api(&fabric_level_method_class(method, descriptor)),
                None,
                "the environment API {method}{descriptor} should be mapped"
            );
        }
        assert_eq!(unsupported_class_api(&fabric_level_method_class(
            "getBlockState",
            "(Lnet/minecraft/core/BlockPos;)Lnet/minecraft/world/level/block/state/BlockState;"
        )), None);
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/world/level/block/state/BlockState",
            "getBlock",
            "()Lnet/minecraft/world/level/block/Block;"
        ));
        for method in ["isAir", "isSolid", "isSolidRender"] {
            assert!(mapped_minecraft_member(
                10,
                "net/minecraft/world/level/block/state/BlockState",
                method,
                "()Z"
            ));
        }
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/world/level/block/Block",
            "getDescriptionId",
            "()Ljava/lang/String;"
        ));
        assert!(mapped_fabric_key_mapping_member(
            10,
            "registerKeyMapping",
            "(Lnet/minecraft/client/KeyMapping;)Lnet/minecraft/client/KeyMapping;"
        ));
        assert!(!mapped_fabric_key_mapping_member(10, "register", "(Ljava/lang/Object;)V"));
        assert!(mapped_key_mapping_member(10, "consumeClick", "()Z"));
        assert!(mapped_key_mapping_member(10, "setDown", "(Z)V"));
        assert!(mapped_key_mapping_member(
            10,
            "set",
            "(Lcom/mojang/blaze3d/platform/InputConstants$Key;Z)V"
        ));
        assert!(!mapped_key_mapping_member(10, "set", "(Ljava/lang/Object;Z)V"));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/resources/Identifier",
            "fromNamespaceAndPath",
            "(Ljava/lang/String;Ljava/lang/String;)Lnet/minecraft/resources/Identifier;"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/client/KeyMapping$Category",
            "register",
            "(Lnet/minecraft/resources/Identifier;)Lnet/minecraft/client/KeyMapping$Category;"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/client/KeyMapping$Category",
            "id",
            "()Lnet/minecraft/resources/Identifier;"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/resources/Identifier",
            "getNamespace",
            "()Ljava/lang/String;"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/resources/Identifier",
            "getPath",
            "()Ljava/lang/String;"
        ));
        assert!(!mapped_minecraft_member(
            10,
            "net/minecraft/client/KeyMapping$Category",
            "register",
            "(Ljava/lang/String;)Lnet/minecraft/client/KeyMapping$Category;"
        ));
        assert!(mapped_input_constants_member(9, "KEY_P", "I"));
        assert!(mapped_input_constants_member(9, "KEY_LBRACKET", "I"));
        assert!(mapped_input_constants_member(9, "KEY_NUMPADENTER", "I"));
        assert!(!mapped_input_constants_member(9, "KEY_UNKNOWN", "I"));
        assert!(!mapped_input_constants_member(9, "KEY_LEFT_BRACKET", "I"));
        assert!(mapped_minecraft_member(
            9,
            "net/minecraft/client/Minecraft",
            "level",
            "Lnet/minecraft/client/multiplayer/ClientLevel;"
        ));
        assert!(mapped_minecraft_member(
            9,
            "net/minecraft/client/Minecraft",
            "options",
            "Lnet/minecraft/client/Options;"
        ));
        for field in ["keyUp", "keyDown", "keyLeft", "keyRight", "keyJump", "keyShift", "keySprint"] {
            assert!(mapped_minecraft_member(
                9,
                "net/minecraft/client/Options",
                field,
                "Lnet/minecraft/client/KeyMapping;"
            ));
        }
        assert!(!mapped_minecraft_member(
            10,
            "net/minecraft/client/Options",
            "setKeyUp",
            "(Z)V"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/client/multiplayer/ClientLevel",
            "getOverworldClockTime",
            "()J"
        ));
        assert!(!mapped_minecraft_member(
            10,
            "net/minecraft/world/entity/LivingEntity",
            "setHealth",
            "(F)V"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/world/entity/Entity",
            "setSprinting",
            "(Z)V"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/world/entity/Entity",
            "getYRot",
            "()F"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/world/entity/Entity",
            "blockPosition",
            "()Lnet/minecraft/core/BlockPos;"
        ));
        for component in ["getX", "getY", "getZ"] {
            assert!(mapped_minecraft_member(
                10,
                "net/minecraft/core/BlockPos",
                component,
                "()I"
            ));
        }
        assert!(!mapped_minecraft_member(
            10,
            "net/minecraft/core/BlockPos",
            "above",
            "()Lnet/minecraft/core/BlockPos;"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/client/player/LocalPlayer",
            "getXRot",
            "()F"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/world/entity/Entity",
            "setYRot",
            "(F)V"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/client/player/LocalPlayer",
            "setXRot",
            "(F)V"
        ));
        assert!(!mapped_minecraft_member(
            10,
            "net/minecraft/world/entity/Entity",
            "setYRot",
            "(D)V"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/client/player/LocalPlayer",
            "getDeltaMovement",
            "()Lnet/minecraft/world/phys/Vec3;"
        ));
        assert!(mapped_minecraft_member(
            9,
            "net/minecraft/world/phys/Vec3",
            "x",
            "D"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/world/phys/Vec3",
            "horizontalDistance",
            "()D"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/world/phys/Vec3",
            "add",
            "(DDD)Lnet/minecraft/world/phys/Vec3;"
        ));
        assert!(!mapped_minecraft_member(
            10,
            "net/minecraft/world/phys/Vec3",
            "add",
            "(Ljava/lang/Object;)Lnet/minecraft/world/phys/Vec3;"
        ));
        assert!(!mapped_minecraft_member(
            10,
            "net/minecraft/world/phys/Vec3",
            "add",
            "(Lnet/minecraft/world/phys/Vec3;)Lnet/minecraft/world/phys/Vec3;"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/client/player/LocalPlayer",
            "setDeltaMovement",
            "(Lnet/minecraft/world/phys/Vec3;)V"
        ));
        assert!(mapped_minecraft_member(
            10,
            "net/minecraft/client/player/LocalPlayer",
            "sendSystemMessage",
            "(Lnet/minecraft/network/chat/Component;)V"
        ));
        assert!(!mapped_minecraft_member(
            10,
            "net/minecraft/client/player/LocalPlayer",
            "sendSystemMessage",
            "(Lnet/minecraft/network/chat/Component;Z)V"
        ));
        assert!(!mapped_minecraft_member(
            10,
            "net/minecraft/world/entity/Entity",
            "teleportTo",
            "(DDD)V"
        ));
    }

    #[test]
    fn fabric_ids_and_generated_horizon_ids_are_bounded_and_safe() {
        assert!(valid_fabric_id("sodium-bridge"));
        assert!(!valid_fabric_id("Invalid ID"));
        let generated = horizon_fabric_mod_id("a-very-long-fabric-id-that-is-safe", &"ab".repeat(20));
        assert!(generated.len() <= 64);
        assert!(generated
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'));
    }

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
