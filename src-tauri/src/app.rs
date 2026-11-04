use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::database::Database;
use crate::downloads::{self, DownloadProgress, ProgressSink, RemoteMinecraftVersion};
use crate::horizon::{self, HorizonEngineStatus, HorizonMod};
use crate::java;
use crate::launch::{build_launch_plan, LaunchPlan, LaunchRequest};
use crate::loaders::{self, InstalledLoaderProfile, LoaderVersion};
use crate::minecraft;
use crate::mods::{self, ModDownloadProgress};
use crate::models::{
    Instance, InstanceEngine, JavaRuntime, LaunchExitEvent, LaunchLogEvent, LaunchProfile,
    LaunchStarted, MicrosoftAccount, MinecraftInstallation, OfflineProfile, PlaytimeSummary,
};

struct AppState {
    database: Arc<Database>,
    data_dir: PathBuf,
    active_downloads: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

#[tauri::command]
fn list_instances(state: State<'_, AppState>) -> std::result::Result<Vec<Instance>, String> {
    state.database.list_instances().map_err(display_error)
}

#[tauri::command]
fn get_playtime_tracking_enabled(state: State<'_, AppState>) -> std::result::Result<bool, String> {
    state.database.setting("playtime_tracking_enabled")
        .map(|value| value.as_deref() == Some("true"))
        .map_err(display_error)
}

#[tauri::command]
fn set_playtime_tracking_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    state.database.set_playtime_tracking_enabled(enabled).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn get_playtime_summary(
    start_at: i64,
    end_at: i64,
    state: State<'_, AppState>,
) -> std::result::Result<PlaytimeSummary, String> {
    state.database.playtime_summary(start_at, end_at).map_err(display_error)
}

#[tauri::command]
fn scan_installations() -> Vec<MinecraftInstallation> {
    minecraft::discover_installations()
}

fn horizon_runtime_path(app: &AppHandle) -> std::result::Result<PathBuf, String> {
    let development = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join("horizon-runtime.exe");
    if cfg!(debug_assertions) && development.is_file() {
        return Ok(development);
    }
    let bundled = app
        .path()
        .resource_dir()
        .map_err(display_error)?
        .join("horizon/horizon-runtime.exe");
    if bundled.is_file() {
        return Ok(bundled);
    }
    if development.is_file() {
        return Ok(development);
    }
    Err("The built-in Horizon engine is missing. Rebuild OpnLauncher so its Horizon runtime is included.".to_owned())
}

fn horizon_fabric_bridge_path(app: &AppHandle) -> std::result::Result<PathBuf, String> {
    let development = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join("horizon-fabric-bridge.jar");
    if cfg!(debug_assertions) && development.is_file() {
        return Ok(development);
    }
    let bundled = app
        .path()
        .resource_dir()
        .map_err(display_error)?
        .join("horizon/horizon-fabric-bridge.jar");
    if bundled.is_file() {
        return Ok(bundled);
    }
    if development.is_file() {
        return Ok(development);
    }
    Err("The experimental Fabric bridge helper is missing. Rebuild OpnLauncher so its Fabric bridge is included.".to_owned())
}

fn horizon_source_root(app: &AppHandle) -> std::result::Result<PathBuf, String> {
    let development = Path::new(env!("CARGO_MANIFEST_DIR")).join("horizon-upstream");
    if cfg!(debug_assertions)
        && development.join("research-lock.json").is_file()
        && development
            .join("mods/horizon/src/test/java/dev/horizonlab/parity")
            .is_dir()
    {
        return Ok(development);
    }
    let bundled = app
        .path()
        .resource_dir()
        .map_err(display_error)?
        .join("horizon-source");
    if bundled.join("research-lock.json").is_file()
        && bundled
            .join("mods/horizon/src/test/java/dev/horizonlab/parity")
            .is_dir()
    {
        return Ok(bundled);
    }
    if development.join("research-lock.json").is_file()
        && development
            .join("mods/horizon/src/test/java/dev/horizonlab/parity")
            .is_dir()
    {
        return Ok(development);
    }
    Err("Horizon's bundled resource-preparation sources are missing from this OpnLauncher build.".to_owned())
}

fn horizon_fabric_game_directory(instance: &Instance, create: bool) -> std::result::Result<PathBuf, String> {
    if instance.engine != InstanceEngine::Horizon {
        return Err("the isolated Fabric runtime is only available for Horizon profiles".to_owned());
    }
    let instance_root = fs::canonicalize(&instance.game_dir).map_err(display_error)?;
    if !instance_root.is_dir() {
        return Err("the Horizon profile folder is not a directory".to_owned());
    }
    let private_root = instance_root.join(".opnlauncher");
    match fs::symlink_metadata(&private_root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("the Horizon profile's private folder is not a regular directory".to_owned());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
            fs::create_dir(&private_root).map_err(display_error)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("install the optional Java Fabric runtime for this profile first".to_owned());
        }
        Err(error) => return Err(display_error(error)),
    }
    let runtime = private_root.join("fabric-java");
    match fs::symlink_metadata(&runtime) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err("the isolated Fabric game folder is not a regular directory".to_owned())
        }
        Ok(_) => Ok(runtime),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
            fs::create_dir(&runtime).map_err(display_error)?;
            Ok(runtime)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err("install the optional Java Fabric runtime for this profile first".to_owned())
        }
        Err(error) => Err(display_error(error)),
    }
}

fn configure_horizon_fabric_code_bridge(
    instance: &Instance,
    profile: &LaunchProfile,
    state: &AppState,
    app: &AppHandle,
    command: &mut Command,
) -> std::result::Result<Option<String>, String> {
    let bridge = horizon::fabric_bridge_status(instance).map_err(display_error)?;
    if !bridge.settings.code_enabled {
        return Ok(None);
    }

    let runtime_dir = horizon_fabric_game_directory(instance, false)?;
    let version_id = instance
        .horizon_fabric_version_id
        .as_deref()
        .ok_or_else(|| {
            "install a Fabric Loader profile in Horizon settings before using the code bridge"
                .to_owned()
        })?;
    let java_path = instance
        .horizon_fabric_java_path
        .as_deref()
        .ok_or_else(|| "select a Java runtime for the Horizon Fabric code bridge".to_owned())?;
    let java_runtime = java::inspect_runtime(Path::new(java_path)).map_err(display_error)?;
    if java_runtime.major_version < 25 {
        return Err(format!(
            "the Horizon Fabric code bridge needs Java 25 or newer; selected Java is {}",
            java_runtime.major_version
        ));
    }
    if !loaders::installed_profiles(&runtime_dir)
        .map_err(display_error)?
        .iter()
        .any(|installed| {
            installed.version_id == version_id
                && installed.minecraft_version == "26.3"
                && installed.loader == "fabric"
        })
    {
        return Err("the isolated Fabric profile is incomplete; reinstall Fabric from Horizon settings".to_owned());
    }

    let mut fabric_instance = instance.clone();
    fabric_instance.game_dir = runtime_dir.to_string_lossy().into_owned();
    fabric_instance.version_id = Some(version_id.to_owned());
    fabric_instance.java_path = Some(java_path.to_owned());
    let plan = build_launch_plan(LaunchRequest {
        instance: &fabric_instance,
        profile,
        java_path: Path::new(java_path),
        native_cache: &state.data_dir.join("cache/natives"),
        launcher_version: env!("CARGO_PKG_VERSION"),
        prepare_natives: false,
    }).map_err(display_error)?;

    let class_path_index = plan
        .arguments
        .iter()
        .position(|argument| argument == "-cp" || argument == "-classpath")
        .ok_or_else(|| "the installed Fabric launch profile has no Java class path".to_owned())?;
    let class_path = plan.arguments.get(class_path_index + 1)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| "the installed Fabric launch profile has an empty Java class path".to_owned())?;
    let separator = if cfg!(windows) { ';' } else { ':' };
    let game_jar = class_path
        .split(separator)
        .rev()
        .map(Path::new)
        .find(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
        })
        .ok_or_else(|| {
            "the installed Minecraft 26.3 client JAR is missing from the Fabric class path"
                .to_owned()
        })?;
    let canonical_game_jar = game_jar.canonicalize().map_err(display_error)?;
    let bridge_class_path = class_path
        .split(separator)
        .filter(|entry| !entry.is_empty())
        .filter(|entry| {
            let normalized = entry.replace('\\', "/").to_ascii_lowercase();
            !normalized.contains("/net/fabricmc/fabric-api/")
                && Path::new(entry)
                    .canonicalize()
                    .map(|path| path != canonical_game_jar)
                    .unwrap_or(true)
        })
        .collect::<Vec<_>>()
        .join(&separator.to_string());
    if bridge_class_path.is_empty() {
        return Err("the Fabric helper class path has no loader libraries after removing the Java game and Fabric API JARs".to_owned());
    }
    let mods_dir = horizon::prepare_fabric_code_bridge(instance).map_err(display_error)?;
    let helper_jar = horizon_fabric_bridge_path(app)?;
    let jvm_arguments = plan.arguments[..class_path_index]
        .iter()
        .filter(|argument| !argument.is_empty())
        .cloned()
        .collect::<Vec<_>>();

    command
        .env("OPNLAUNCHER_FABRIC_BRIDGE_JAVA", java_path)
        .env("OPNLAUNCHER_FABRIC_BRIDGE_CLASS_PATH", bridge_class_path)
        .env("OPNLAUNCHER_FABRIC_BRIDGE_JAR", helper_jar)
        .env("OPNLAUNCHER_FABRIC_BRIDGE_MODS_DIR", mods_dir)
        .env("OPNLAUNCHER_FABRIC_BRIDGE_GAME_DIR", runtime_dir)
        .env("OPNLAUNCHER_FABRIC_BRIDGE_JVM_ARG_COUNT", jvm_arguments.len().to_string());
    for (index, argument) in jvm_arguments.iter().enumerate() {
        command.env(format!("OPNLAUNCHER_FABRIC_BRIDGE_JVM_ARG_{index}"), argument);
    }

    Ok(Some(format!(
        "Experimental Fabric bridge armed for profile {version_id}. {} mod JAR(s) passed the current API allowlist; {} unsupported JAR(s) were excluded. Original mod files are unchanged. Player/input snapshots and bounded sprint, velocity, look, chat, and HUD actions are mapped. Mixins and rendering APIs, including Sodium, are unsupported.",
        bridge.code_mods.iter().filter(|item| item.state == "supported").count(),
        bridge.code_mods.iter().filter(|item| item.state == "unsupported" || item.state == "invalid").count(),
    )))
}

fn copy_existing_horizon_mods(instance: &Instance, runtime_dir: &Path) -> std::result::Result<u32, String> {
    const MAX_MOD_FILE_BYTES: u64 = 768 * 1024 * 1024;
    const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    const MAX_FILES: usize = 512;

    let profile_root = fs::canonicalize(&instance.game_dir).map_err(display_error)?;
    let source_dir = profile_root.join("mods");
    let source_metadata = match fs::symlink_metadata(&source_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("the Horizon profile's existing mods folder is not a regular directory".to_owned());
        }
        Ok(_) => source_dir,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(display_error(error)),
    };
    let destination_dir = runtime_dir.join("mods");
    match fs::symlink_metadata(&destination_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("the isolated Fabric mods folder is not a regular directory".to_owned());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&destination_dir).map_err(display_error)?;
        }
        Err(error) => return Err(display_error(error)),
    }

    let mut copied = 0u32;
    let mut total_bytes = 0u64;
    for entry in fs::read_dir(source_metadata).map_err(display_error)? {
        let entry = entry.map_err(display_error)?;
        let entry_type = entry.file_type().map_err(display_error)?;
        if !entry_type.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let lower = file_name.to_string_lossy().to_ascii_lowercase();
        if !(lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
            continue;
        }
        let metadata = entry.metadata().map_err(display_error)?;
        if metadata.len() > MAX_MOD_FILE_BYTES {
            return Err(format!("existing mod {} is larger than the safe copy limit", file_name.to_string_lossy()));
        }
        if copied as usize >= MAX_FILES || total_bytes.saturating_add(metadata.len()) > MAX_TOTAL_BYTES {
            return Err("existing Horizon mods exceed the safe copy limits for the Java runtime".to_owned());
        }
        let destination = destination_dir.join(&file_name);
        let mut output = match fs::OpenOptions::new().write(true).create_new(true).open(&destination) {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(display_error(error)),
        };
        let copy_result = (|| {
            let mut source = File::open(entry.path()).map_err(display_error)?;
            let copied_bytes = std::io::copy(&mut source, &mut output).map_err(display_error)?;
            if copied_bytes != metadata.len() {
                return Err("an existing mod changed while it was being copied".to_owned());
            }
            output.flush().map_err(display_error)?;
            Ok(())
        })();
        if let Err(error) = copy_result {
            let _ = fs::remove_file(&destination);
            return Err(error);
        }
        copied += 1;
        total_bytes = total_bytes.saturating_add(metadata.len());
    }
    Ok(copied)
}

fn set_launch_argument(arguments: &mut Vec<String>, name: &str, value: String) {
    if let Some(index) = arguments.iter().position(|argument| argument == name) {
        if let Some(argument) = arguments.get_mut(index + 1) {
            *argument = value;
        } else {
            arguments.push(value);
        }
    } else {
        arguments.push(name.to_owned());
        arguments.push(value);
    }
}

#[tauri::command(rename_all = "camelCase")]
fn inspect_horizon_engine(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<HorizonEngineStatus, String> {
    let client = horizon::inspect_client(&horizon_runtime_path(&app)?).map_err(display_error)?;
    let source_root = horizon_source_root(&app)?;
    let resources = state.data_dir.join("horizon/resources");
    let resources_ready = horizon_prepare::check(&source_root, &resources)
        .map(|missing| missing.is_empty())
        .unwrap_or(false);
    Ok(HorizonEngineStatus {
        client,
        resources_path: resources.to_string_lossy().into_owned(),
        resources_ready,
    })
}

#[tauri::command]
async fn list_official_versions() -> std::result::Result<Vec<RemoteMinecraftVersion>, String> {
    tauri::async_runtime::spawn_blocking(downloads::available_versions)
        .await
        .map_err(display_error)?
        .map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn search_mod_projects(
    provider: String,
    query: String,
    game_version: String,
    loader: String,
    sort: String,
    offset: u32,
    api_key: Option<String>,
) -> std::result::Result<mods::ModSearchResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        mods::search_projects(&provider, &query, &game_version, &loader, &sort, offset, api_key.as_deref())
    })
    .await
    .map_err(display_error)?
    .map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn get_mod_project(
    provider: String,
    project_id: String,
    api_key: Option<String>,
) -> std::result::Result<mods::ModProject, String> {
    tauri::async_runtime::spawn_blocking(move || mods::get_project(&provider, &project_id, api_key.as_deref()))
        .await
        .map_err(display_error)?
        .map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn get_mod_project_versions(
    provider: String,
    project_id: String,
    game_version: String,
    loader: String,
    api_key: Option<String>,
) -> std::result::Result<Vec<mods::ModVersion>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        mods::get_project_versions(&provider, &project_id, &game_version, &loader, api_key.as_deref())
    })
    .await
    .map_err(display_error)?
    .map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn get_all_mod_project_versions(
    provider: String,
    project_id: String,
    api_key: Option<String>,
) -> std::result::Result<Vec<mods::ModVersion>, String> {
    tauri::async_runtime::spawn_blocking(move || mods::get_all_project_versions(&provider, &project_id, api_key.as_deref()))
        .await
        .map_err(display_error)?
        .map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn inspect_instance_mod_context(
    instance_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<mods::ModInstanceContext, String> {
    mods::inspect_instance(&state.database, &instance_id).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn list_installed_mods(
    instance_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<mods::InstalledMod>, String> {
    mods::list_installed_mods(&state.database, &instance_id).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn set_mod_enabled(
    instance_id: String,
    file_name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    mods::set_mod_enabled(&state.database, &instance_id, &file_name, enabled).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn remove_mod_file(
    instance_id: String,
    file_name: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    mods::remove_mod(&state.database, &instance_id, &file_name, confirmed).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn remove_mod_files(
    instance_id: String,
    file_names: Vec<String>,
    confirmed: bool,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    mods::remove_mod_files(&state.database, &instance_id, &file_names, confirmed).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn resolve_installed_mod_source(
    instance_id: String,
    file_name: String,
    state: State<'_, AppState>,
) -> std::result::Result<Option<crate::database::ModInstallSource>, String> {
    let database = Arc::clone(&state.database);
    tauri::async_runtime::spawn_blocking(move || mods::resolve_installed_mod_source(&database, &instance_id, &file_name))
        .await
        .map_err(display_error)?
        .map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn link_installed_mod_to_project(
    instance_id: String,
    file_name: String,
    provider: String,
    project_id: String,
    api_key: Option<String>,
    state: State<'_, AppState>,
) -> std::result::Result<crate::database::ModInstallSource, String> {
    let database = Arc::clone(&state.database);
    tauri::async_runtime::spawn_blocking(move || {
        mods::link_installed_mod_to_project(&database, &instance_id, &file_name, &provider, &project_id, api_key.as_deref())
    })
    .await
    .map_err(display_error)?
    .map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn install_mod_version(
    instance_id: String,
    provider: String,
    project_id: String,
    version_id: String,
    download_id: String,
    api_key: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> std::result::Result<Vec<mods::InstalledMod>, String> {
    Uuid::parse_str(&download_id).map_err(|_| "download identifier is invalid".to_owned())?;
    let cancellation = Arc::new(AtomicBool::new(false));
    let active_downloads = Arc::clone(&state.active_downloads);
    {
        let mut active = active_downloads.lock().map_err(display_error)?;
        if active.contains_key(&download_id) {
            return Err("a download with this identifier is already active".to_owned());
        }
        active.insert(download_id.clone(), Arc::clone(&cancellation));
    }
    let database = Arc::clone(&state.database);
    let worker_download_id = download_id.clone();
    let app_for_progress = app.clone();
    let progress: mods::ModProgressSink = Arc::new(move |event: ModDownloadProgress| {
        let _ = app_for_progress.emit("mod-download-progress", event);
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        mods::install_project_version(
            &database,
            &instance_id,
            &provider,
            &project_id,
            &version_id,
            api_key.as_deref(),
            &worker_download_id,
            cancellation,
            progress,
        )
    })
    .await;
    if let Ok(mut active) = active_downloads.lock() {
        active.remove(&download_id);
    }
    result.map_err(display_error)?.map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn switch_mod_version(
    instance_id: String,
    old_file_name: String,
    provider: String,
    project_id: String,
    version_id: String,
    download_id: String,
    api_key: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> std::result::Result<Vec<mods::InstalledMod>, String> {
    Uuid::parse_str(&download_id).map_err(|_| "download identifier is invalid".to_owned())?;
    let cancellation = Arc::new(AtomicBool::new(false));
    let active_downloads = Arc::clone(&state.active_downloads);
    {
        let mut active = active_downloads.lock().map_err(display_error)?;
        if active.contains_key(&download_id) {
            return Err("a download with this identifier is already active".to_owned());
        }
        active.insert(download_id.clone(), Arc::clone(&cancellation));
    }
    let database = Arc::clone(&state.database);
    let worker_download_id = download_id.clone();
    let app_for_progress = app.clone();
    let progress: mods::ModProgressSink = Arc::new(move |event: ModDownloadProgress| {
        let _ = app_for_progress.emit("mod-download-progress", event);
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        mods::switch_project_version(
            &database,
            &instance_id,
            &old_file_name,
            &provider,
            &project_id,
            &version_id,
            api_key.as_deref(),
            &worker_download_id,
            cancellation,
            progress,
        )
    })
    .await;
    if let Ok(mut active) = active_downloads.lock() {
        active.remove(&download_id);
    }
    result.map_err(display_error)?.map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn install_minecraft_version(
    name: String,
    version_id: String,
    download_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> std::result::Result<Instance, String> {
    Uuid::parse_str(&download_id).map_err(|_| "download identifier is invalid".to_owned())?;
    let name = crate::database::validate_instance_name(&name).map_err(display_error)?;
    let cancellation = Arc::new(AtomicBool::new(false));
    let active_downloads = Arc::clone(&state.active_downloads);
    {
        let mut active = active_downloads.lock().map_err(display_error)?;
        if active.contains_key(&download_id) {
            return Err("a download with this identifier is already active".to_owned());
        }
        active.insert(download_id.clone(), Arc::clone(&cancellation));
    }

    let database = Arc::clone(&state.database);
    let data_dir = state.data_dir.clone();
    let instance_id = Uuid::new_v4().to_string();
    let instances_dir = data_dir.join("instances");
    let game_dir = instances_dir.join(&instance_id);
    let worker_download_id = download_id.clone();
    let app_for_progress = app.clone();
    let progress_sink: ProgressSink = Arc::new(move |progress: DownloadProgress| {
        let _ = app_for_progress.emit("download-progress", progress);
    });
    let install_result = tauri::async_runtime::spawn_blocking(move || {
        let mut created_game_dir = false;
        let result = (|| {
            fs::create_dir_all(&instances_dir).map_err(display_error)?;
            fs::create_dir(&game_dir).map_err(display_error)?;
            created_game_dir = true;
            downloads::install_version(
                &version_id,
                &game_dir,
                &worker_download_id,
                Arc::clone(&cancellation),
                progress_sink,
            )
            .map_err(display_error)?;
            database
                .create_instance_with_id(&instance_id, &name, &game_dir.to_string_lossy(), Some(&version_id))
                .map_err(display_error)
        })();
        if result.is_err() && created_game_dir {
            let _ = fs::remove_dir_all(&game_dir);
        }
        result
    })
    .await;
    if let Ok(mut active) = active_downloads.lock() {
        active.remove(&download_id);
    }
    install_result.map_err(display_error)?
}

#[tauri::command(rename_all = "camelCase")]
async fn install_instance_setup(
    instance_id: String,
    minecraft_version: String,
    loader: String,
    loader_version: Option<String>,
    java_path: Option<String>,
    download_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> std::result::Result<Instance, String> {
    Uuid::parse_str(&download_id).map_err(|_| "download identifier is invalid".to_owned())?;
    if !minecraft::is_safe_id(&minecraft_version) {
        return Err("Minecraft version identifier is invalid".to_owned());
    }
    if !matches!(loader.as_str(), "vanilla" | "fabric" | "forge") {
        return Err("unsupported game loader".to_owned());
    }
    if loader != "vanilla" && loader_version.as_deref().is_none_or(str::is_empty) {
        return Err("choose a compatible loader version".to_owned());
    }
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Minecraft {
        return Err("Java game setup is not available for Horizon profiles".to_owned());
    }
    let cancellation = Arc::new(AtomicBool::new(false));
    let active_downloads = Arc::clone(&state.active_downloads);
    {
        let mut active = active_downloads.lock().map_err(display_error)?;
        if active.contains_key(&download_id) {
            return Err("a download with this identifier is already active".to_owned());
        }
        active.insert(download_id.clone(), Arc::clone(&cancellation));
    }

    let database = Arc::clone(&state.database);
    let game_dir = PathBuf::from(&instance.game_dir);
    let worker_download_id = download_id.clone();
    let worker_instance_id = instance_id.clone();
    let worker_loader = loader.clone();
    let worker_version = minecraft_version.clone();
    let worker_loader_version = loader_version.clone();
    let worker_java_path = java_path.map(PathBuf::from);
    let progress_app = app.clone();
    let phase_app = app.clone();
    let progress_sink: ProgressSink = Arc::new(move |progress: DownloadProgress| {
        let _ = progress_app.emit("download-progress", progress);
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            match worker_loader.as_str() {
                "vanilla" => {}
                "fabric" => {
                    let version = worker_loader_version.as_deref().expect("loader version validated above");
                    if !loaders::fabric_versions(&worker_version)?.iter().any(|entry| entry.version == version) {
                        return Err(crate::error::LauncherError::InvalidInput("the selected Fabric build is not compatible with this Minecraft version".to_owned()));
                    }
                }
                "forge" => {
                    let version = worker_loader_version.as_deref().expect("loader version validated above");
                    if !loaders::forge_versions(&worker_version)?.iter().any(|entry| entry.version == version) {
                        return Err(crate::error::LauncherError::InvalidInput("the selected Forge build is not compatible with this Minecraft version".to_owned()));
                    }
                    let java = worker_java_path.as_deref().ok_or_else(|| crate::error::LauncherError::InvalidInput("choose a Java runtime for the Forge installer".to_owned()))?;
                    java::inspect_runtime(java)?;
                }
                _ => unreachable!("loader choice validated above"),
            }
            downloads::install_version_for_instance(
                &worker_version,
                &game_dir,
                &worker_download_id,
                Arc::clone(&cancellation),
                progress_sink,
            )?;
            if cancellation.load(Ordering::Relaxed) {
                return Err(crate::error::LauncherError::Unsupported("installation cancelled".to_owned()));
            }
            let selected_version = match worker_loader.as_str() {
                "vanilla" => worker_version.clone(),
                "fabric" => {
                    let _ = phase_app.emit("download-progress", DownloadProgress {
                        download_id: worker_download_id.clone(), version_id: worker_version.clone(),
                        phase: "Installing Fabric loader".to_owned(), current_file: String::new(),
                        completed_files: 0, total_files: 0, downloaded_bytes: 0, total_bytes: 0,
                    });
                    loaders::install_fabric(&game_dir, &worker_version, worker_loader_version.as_deref().expect("loader version validated above"))?
                }
                "forge" => {
                    let _ = phase_app.emit("download-progress", DownloadProgress {
                        download_id: worker_download_id.clone(), version_id: worker_version.clone(),
                        phase: "Installing Forge loader".to_owned(), current_file: String::new(),
                        completed_files: 0, total_files: 0, downloaded_bytes: 0, total_bytes: 0,
                    });
                    loaders::install_forge(
                        &game_dir,
                        &worker_version,
                        worker_loader_version.as_deref().expect("loader version validated above"),
                        worker_java_path.as_deref().expect("Java runtime validated above"),
                    )?
                }
                _ => unreachable!("loader choice validated above"),
            };
            database.set_instance_version(&worker_instance_id, &selected_version)?;
            let _ = phase_app.emit("download-progress", DownloadProgress {
                download_id: worker_download_id.clone(), version_id: worker_version.clone(),
                phase: "Installation complete".to_owned(), current_file: String::new(),
                completed_files: 1, total_files: 1, downloaded_bytes: 0, total_bytes: 0,
            });
            Ok(selected_version)
        })();
        result
    })
    .await;
    if let Ok(mut active) = active_downloads.lock() {
        active.remove(&download_id);
    }
    result.map_err(display_error)?.map_err(display_error)?;
    state.database.get_instance(&instance_id).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn install_horizon_fabric_setup(
    instance_id: String,
    loader_version: String,
    java_path: String,
    download_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> std::result::Result<Instance, String> {
    Uuid::parse_str(&download_id).map_err(|_| "download identifier is invalid".to_owned())?;
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Horizon {
        return Err("choose a Horizon profile before installing its Fabric compatibility runtime".to_owned());
    }
    let java = java::inspect_runtime(Path::new(&java_path)).map_err(display_error)?;
    if java.major_version < 25 {
        return Err(format!("Minecraft 26.3 Fabric requires Java 25 or newer; selected Java {}", java.major_version));
    }
    if !loaders::fabric_versions("26.3").map_err(display_error)?.iter().any(|version| version.version == loader_version) {
        return Err("the selected Fabric Loader build is not compatible with Minecraft 26.3".to_owned());
    }
    let runtime = horizon_fabric_game_directory(&instance, true)?;
    let runtime_was_enabled = instance.horizon_fabric_enabled;

    let cancellation = Arc::new(AtomicBool::new(false));
    let active_downloads = Arc::clone(&state.active_downloads);
    {
        let mut active = active_downloads.lock().map_err(display_error)?;
        if active.contains_key(&download_id) {
            return Err("a download with this identifier is already active".to_owned());
        }
        active.insert(download_id.clone(), Arc::clone(&cancellation));
    }
    let database = Arc::clone(&state.database);
    let worker_download_id = download_id.clone();
    let worker_instance_id = instance_id.clone();
    let worker_loader_version = loader_version.clone();
    let worker_java_path = java.path.clone();
    let worker_runtime_enabled = runtime_was_enabled;
    let progress_app = app.clone();
    let phase_app = app.clone();
    let progress_sink: ProgressSink = Arc::new(move |progress: DownloadProgress| {
        let _ = progress_app.emit("download-progress", progress);
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        let install = (|| {
            downloads::install_version_for_instance(
                "26.3",
                &runtime,
                &worker_download_id,
                Arc::clone(&cancellation),
                progress_sink,
            )?;
            if cancellation.load(Ordering::Relaxed) {
                return Err(crate::error::LauncherError::Unsupported("installation cancelled".to_owned()));
            }
            let _ = phase_app.emit("download-progress", DownloadProgress {
                download_id: worker_download_id.clone(),
                version_id: "26.3".to_owned(),
                phase: "Installing Fabric Loader".to_owned(),
                current_file: String::new(),
                completed_files: 0,
                total_files: 0,
                downloaded_bytes: 0,
                total_bytes: 0,
            });
            let profile_id = loaders::install_fabric(&runtime, "26.3", &worker_loader_version)?;
            copy_existing_horizon_mods(&instance, &runtime).map_err(crate::error::LauncherError::Unsupported)?;
            database.set_horizon_fabric_runtime(
                &worker_instance_id,
                Some(&profile_id),
                Some(&worker_java_path),
                worker_runtime_enabled,
            )?;
            database.get_instance(&worker_instance_id)
        })();
        install
    }).await;
    if let Ok(mut active) = active_downloads.lock() {
        active.remove(&download_id);
    }
    result.map_err(display_error)?.map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn set_horizon_fabric_runtime_enabled(
    instance_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Horizon {
        return Err("the Fabric compatibility runtime is only available for Horizon profiles".to_owned());
    }
    if enabled {
        let _ = horizon_fabric_game_directory(&instance, false)?;
        let version_id = instance.horizon_fabric_version_id.as_deref()
            .ok_or_else(|| "install the isolated Fabric runtime before enabling it".to_owned())?;
        let java_path = instance.horizon_fabric_java_path.as_deref()
            .ok_or_else(|| "choose a Java runtime for the Fabric compatibility mode".to_owned())?;
        let java = java::inspect_runtime(Path::new(java_path)).map_err(display_error)?;
        if java.major_version < 25 {
            return Err("Minecraft 26.3 Fabric needs Java 25 or newer".to_owned());
        }
        let runtime = horizon_fabric_game_directory(&instance, false)?;
        if loaders::base_game_version(&runtime, version_id).map_err(display_error)? != "26.3" {
            return Err("the installed Fabric profile is not based on Minecraft 26.3".to_owned());
        }
        if !loaders::installed_profiles(&runtime).map_err(display_error)?.iter().any(|profile| {
            profile.version_id == version_id && profile.minecraft_version == "26.3" && profile.loader == "fabric"
        }) {
            return Err("the isolated Java profile does not contain a complete Fabric installation".to_owned());
        }
    }
    state.database.set_horizon_fabric_runtime_enabled(&instance_id, enabled).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn cancel_download(download_id: String, state: State<'_, AppState>) -> std::result::Result<(), String> {
    let active = state.active_downloads.lock().map_err(display_error)?;
    let cancellation = active
        .get(&download_id)
        .ok_or_else(|| "download is not active".to_owned())?;
    cancellation.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
fn inspect_installation(path: String) -> std::result::Result<MinecraftInstallation, String> {
    minecraft::inspect_installation(Path::new(&path)).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn register_installation(
    name: String,
    root: String,
    version_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<Instance, String> {
    let installation = minecraft::inspect_installation(Path::new(&root)).map_err(display_error)?;
    if !installation.versions.iter().any(|version| version.id == version_id) {
        return Err(format!("version {version_id} is not installed in this directory"));
    }
    state
        .database
        .create_instance(&name, &installation.root, Some(&version_id))
        .map_err(display_error)
}

#[tauri::command]
fn create_instance(
    name: String,
    state: State<'_, AppState>,
) -> std::result::Result<Instance, String> {
    let id = Uuid::new_v4().to_string();
    let game_dir = state.data_dir.join("instances").join(&id);
    fs::create_dir_all(&game_dir).map_err(display_error)?;
    match state.database.create_instance(&name, &game_dir.to_string_lossy(), None) {
        Ok(instance) => Ok(instance),
        Err(error) => {
            let _ = fs::remove_dir(&game_dir);
            Err(display_error(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
fn create_horizon_instance(
    name: String,
    state: State<'_, AppState>,
) -> std::result::Result<Instance, String> {
    let id = Uuid::new_v4().to_string();
    let game_dir = state.data_dir.join("instances").join(&id);
    fs::create_dir_all(&game_dir).map_err(display_error)?;
    match state.database.create_horizon_instance_with_id(&id, &name, &game_dir.to_string_lossy()) {
        Ok(instance) => Ok(instance),
        Err(error) => {
            let _ = fs::remove_dir(&game_dir);
            Err(display_error(error))
        }
    }
}

fn import_horizon_sound_assets(
    prepare_root: &Path,
    minecraft_dir: &Path,
    resources_dir: &Path,
    log: horizon_prepare::Log<'_>,
) -> std::result::Result<(), String> {
    horizon_prepare::add_assets(
        "sounds",
        prepare_root,
        Some(resources_dir),
        Some(minecraft_dir),
        &horizon_prepare::artifacts::Inputs::default(),
        log,
    )
    .map(|_| ())
}

#[tauri::command(rename_all = "camelCase")]
async fn prepare_horizon_resources(
    java_path: String,
    minecraft_source_path: Option<String>,
    download_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> std::result::Result<String, String> {
    if !offline_profiles_enabled() {
        return Err("Horizon preparation is disabled in this build".to_owned());
    }
    Uuid::parse_str(&download_id).map_err(|_| "download identifier is invalid".to_owned())?;
    let bundled_source_root = horizon_source_root(&app)?;
    let cancellation = Arc::new(AtomicBool::new(false));
    let active_downloads = Arc::clone(&state.active_downloads);
    {
        let mut active = active_downloads.lock().map_err(display_error)?;
        if active.contains_key(&download_id) {
            return Err("a download with this identifier is already active".to_owned());
        }
        active.insert(download_id.clone(), Arc::clone(&cancellation));
    }

    let data_dir = state.data_dir.clone();
    let worker_download_id = download_id.clone();
    let source_root = minecraft_source_path.map(PathBuf::from);
    let progress_app = app.clone();
    let prepare_app = app.clone();
    let progress_sink: ProgressSink = Arc::new(move |mut progress: DownloadProgress| {
        progress.version_id = format!("{} · Horizon", progress.version_id);
        let _ = progress_app.emit("download-progress", progress);
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        let horizon_dir = data_dir.join("horizon");
        let minecraft_dir = horizon_dir.join("minecraft");
        let resources_dir = horizon_dir.join("resources");
        fs::create_dir_all(&minecraft_dir).map_err(display_error)?;

        let java_executable = fs::canonicalize(&java_path)
            .map_err(|error| format!("could not open the selected Java 25 runtime: {error}"))?;
        let java_runtime = java::inspect_runtime(&java_executable).map_err(display_error)?;
        if java_runtime.major_version != 25 {
            return Err(format!(
                "Horizon resource preparation needs JDK 25; selected Java is {}",
                java_runtime.major_version
            ));
        }
        let java_home = java_executable
            .parent()
            .and_then(Path::parent)
            .filter(|home| home.join("bin").join(if cfg!(windows) { "javac.exe" } else { "javac" }).is_file())
            .ok_or_else(|| "Horizon resource preparation needs a JDK 25 installation with javac".to_owned())?
            .to_path_buf();

        downloads::install_horizon_version_using_source(
            "26.3",
            &minecraft_dir,
            &worker_download_id,
            Arc::clone(&cancellation),
            progress_sink,
            source_root.as_deref(),
        )
        .map_err(display_error)?;
        if cancellation.load(Ordering::Relaxed) {
            return Err("installation cancelled".to_owned());
        }

        let prepare_root = horizon_dir.join(format!("preparer-source-{}", &horizon::HORIZON_SOURCE_REVISION[..8]));
        horizon::stage_preparer_inputs(&bundled_source_root, &prepare_root).map_err(display_error)?;
        let _ = prepare_app.emit("download-progress", DownloadProgress {
            download_id: worker_download_id.clone(),
            version_id: "26.3 · Horizon".to_owned(),
            phase: "Preparing official metadata".to_owned(),
            current_file: "Verifying Mojang’s pinned Minecraft 26.3 manifest".to_owned(),
            completed_files: 0,
            total_files: 0,
            downloaded_bytes: 0,
            total_bytes: 0,
        });
        downloads::stage_pinned_horizon_manifest(&prepare_root).map_err(display_error)?;

        let _ = prepare_app.emit("download-progress", DownloadProgress {
            download_id: worker_download_id.clone(),
            version_id: "26.3 · Horizon".to_owned(),
            phase: "Installing Horizon resources".to_owned(),
            current_file: "Compiling the prepared game resource pack".to_owned(),
            completed_files: 0,
            total_files: 0,
            downloaded_bytes: 0,
            total_bytes: 0,
        });

        let staging_dir = horizon_dir.join(format!(".resources-staging-{worker_download_id}"));
        fs::create_dir(&staging_dir).map_err(display_error)?;
        let options = horizon_prepare::Options {
            root: prepare_root.clone(),
            import: true,
            minecraft_dir: Some(minecraft_dir.clone()),
            server_jar: Some(minecraft_dir.join("versions/26.3/26.3-server.jar")),
            java_home: Some(java_home),
            inputs: horizon_prepare::artifacts::Inputs::default(),
            output: Some(staging_dir.clone()),
        };
        let progress_log_app = prepare_app.clone();
        let progress_log_id = worker_download_id.clone();
        let log = move |line: &str| {
            let _ = progress_log_app.emit("download-progress", DownloadProgress {
                download_id: progress_log_id.clone(),
                version_id: "26.3 · Horizon".to_owned(),
                phase: "Preparing Horizon resources".to_owned(),
                current_file: line.to_owned(),
                completed_files: 0,
                total_files: 0,
                downloaded_bytes: 0,
                total_bytes: 0,
            });
        };
        let progress_app = prepare_app.clone();
        let progress_id = worker_download_id.clone();
        let progress = move |fraction: f32| {
            const SCALE: u64 = 1000;
            let completed = (fraction.clamp(0.0, 1.0) * SCALE as f32) as u64;
            let _ = progress_app.emit("download-progress", DownloadProgress {
                download_id: progress_id.clone(),
                version_id: "26.3 · Horizon".to_owned(),
                phase: "Preparing Horizon resources".to_owned(),
                current_file: "Building game resources from the selected Java 25 installation".to_owned(),
                completed_files: completed,
                total_files: SCALE,
                downloaded_bytes: 0,
                total_bytes: 0,
            });
        };
        let preparation = horizon_prepare::prepare_with_progress(&options, &log, &progress)
            .and_then(|prepared| {
                if prepared != staging_dir {
                    return Err("Horizon prepared resources in an unexpected folder".to_owned());
                }
                if cancellation.load(Ordering::Relaxed) {
                    return Err("installation cancelled".to_owned());
                }

                let _ = prepare_app.emit("download-progress", DownloadProgress {
                    download_id: worker_download_id.clone(),
                    version_id: "26.3 · Horizon".to_owned(),
                    phase: "Importing Horizon sound assets".to_owned(),
                    current_file: "Copying verified Minecraft sounds (about 380 MB)".to_owned(),
                    completed_files: 0,
                    total_files: 0,
                    downloaded_bytes: 0,
                    total_bytes: 0,
                });
                let sound_app = prepare_app.clone();
                let sound_download_id = worker_download_id.clone();
                let sound_log = move |line: &str| {
                    let _ = sound_app.emit("download-progress", DownloadProgress {
                        download_id: sound_download_id.clone(),
                        version_id: "26.3 · Horizon".to_owned(),
                        phase: "Importing Horizon sound assets".to_owned(),
                        current_file: line.to_owned(),
                        completed_files: 0,
                        total_files: 0,
                        downloaded_bytes: 0,
                        total_bytes: 0,
                    });
                };
                import_horizon_sound_assets(
                    &prepare_root,
                    &minecraft_dir,
                    &staging_dir,
                    &sound_log,
                )?;

                if cancellation.load(Ordering::Relaxed) {
                    return Err("installation cancelled".to_owned());
                }
                let missing = horizon_prepare::check(&prepare_root, &staging_dir)?;
                if missing.is_empty() {
                    Ok(prepared)
                } else {
                    Err(format!("Horizon resources are incomplete: {}", missing.join("; ")))
                }
            });
        if let Err(error) = preparation {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(error);
        }

        let backup_dir = horizon_dir.join(format!(".resources-backup-{worker_download_id}"));
        let had_previous = match fs::symlink_metadata(&resources_dir) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                if let Err(error) = fs::rename(&resources_dir, &backup_dir) {
                    let _ = fs::remove_dir_all(&staging_dir);
                    return Err(display_error(error));
                }
                true
            }
            Ok(_) => {
                let _ = fs::remove_dir_all(&staging_dir);
                return Err("the existing Horizon resources path is not a regular folder".to_owned());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                let _ = fs::remove_dir_all(&staging_dir);
                return Err(display_error(error));
            }
        };
        if let Err(error) = fs::rename(&staging_dir, &resources_dir) {
            if had_previous {
                let _ = fs::rename(&backup_dir, &resources_dir);
            }
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(display_error(error));
        }
        if had_previous {
            fs::remove_dir_all(&backup_dir).map_err(display_error)?;
        }
        let resource_path = fs::canonicalize(&resources_dir).map_err(display_error)?;
        let _ = prepare_app.emit("download-progress", DownloadProgress {
            download_id: worker_download_id,
            version_id: "26.3 · Horizon".to_owned(),
            phase: "Installation complete".to_owned(),
            current_file: resource_path.to_string_lossy().into_owned(),
            completed_files: 1,
            total_files: 1,
            downloaded_bytes: 0,
            total_bytes: 0,
        });
        Ok(resource_path.to_string_lossy().into_owned())
    })
    .await;
    if let Ok(mut active) = active_downloads.lock() {
        active.remove(&download_id);
    }
    result.map_err(display_error)?
}

#[tauri::command(rename_all = "camelCase")]
fn list_horizon_mods(
    instance_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<HorizonMod>, String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    horizon::list_mods(&instance).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn get_horizon_fabric_bridge_status(
    instance_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<horizon::HorizonFabricBridgeStatus, String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    horizon::fabric_bridge_status(&instance).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn set_horizon_fabric_bridge_enabled(
    instance_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> std::result::Result<horizon::HorizonFabricBridgeSettings, String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    horizon::set_fabric_bridge_enabled(&instance, enabled).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn set_horizon_fabric_code_bridge_enabled(
    instance_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> std::result::Result<horizon::HorizonFabricBridgeSettings, String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    horizon::set_fabric_code_bridge_enabled(&instance, enabled).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn install_horizon_mod(
    instance_id: String,
    source_path: String,
    state: State<'_, AppState>,
) -> std::result::Result<HorizonMod, String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    horizon::install_mod(&instance, Path::new(&source_path)).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn set_horizon_mod_enabled(
    instance_id: String,
    file_name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    horizon::set_mod_enabled(&instance, &file_name, enabled).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn remove_horizon_mod(
    instance_id: String,
    file_name: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    horizon::remove_mod(&instance, &file_name, confirmed).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn rename_instance(
    instance_id: String,
    name: String,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    state.database.rename_instance(&instance_id, &name).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn forget_instance(
    instance_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    if !confirmed {
        return Err("confirm removal before forgetting this instance".to_owned());
    }
    state.database.delete_instance_record(&instance_id).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn open_instance_folder(
    instance_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    let path = fs::canonicalize(&instance.game_dir).map_err(|error| {
        format!("cannot open this instance folder: {error}")
    })?;
    if !path.is_dir() {
        return Err("the instance game folder is not a directory".to_owned());
    }

    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer.exe");
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return Err("opening the file manager is not supported on this platform".to_owned());

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    {
        command.arg(path).spawn().map_err(|error| {
            format!("could not open the instance folder in the file manager: {error}")
        })?;
        Ok(())
    }
}

#[tauri::command(rename_all = "camelCase")]
fn set_instance_java(
    instance_id: String,
    java_path: Option<String>,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Minecraft {
        return Err("Java runtime settings are not available for Horizon profiles".to_owned());
    }
    if let Some(path) = java_path.as_deref() {
        java::inspect_runtime(Path::new(path)).map_err(display_error)?;
    }
    state
        .database
        .set_instance_java(&instance_id, java_path.as_deref())
        .map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn set_instance_memory(
    instance_id: String,
    memory_mb: u32,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Minecraft {
        return Err("Java memory settings are not available for Horizon profiles".to_owned());
    }
    state.database.set_memory(&instance_id, memory_mb).map_err(display_error)
}

#[tauri::command]
fn list_java_runtimes() -> Vec<JavaRuntime> {
    java::discover_runtimes()
}

#[tauri::command(rename_all = "camelCase")]
fn inspect_java_runtime(path: String) -> std::result::Result<JavaRuntime, String> {
    java::inspect_runtime(Path::new(&path)).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn list_loader_versions(
    loader: String,
    game_version: String,
) -> std::result::Result<Vec<LoaderVersion>, String> {
    tauri::async_runtime::spawn_blocking(move || match loader.as_str() {
        "fabric" => loaders::fabric_versions(&game_version),
        "forge" => loaders::forge_versions(&game_version),
        _ => Err(crate::error::LauncherError::InvalidInput("unsupported mod loader".to_owned())),
    })
    .await
    .map_err(display_error)?
    .map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn list_installed_loader_profiles(
    instance_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<InstalledLoaderProfile>, String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Minecraft {
        return Err("Java mod loaders are not available for Horizon profiles".to_owned());
    }
    loaders::installed_profiles(Path::new(&instance.game_dir)).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn install_fabric_loader(
    instance_id: String,
    loader_version: String,
    state: State<'_, AppState>,
) -> std::result::Result<Instance, String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Minecraft {
        return Err("Fabric cannot be installed on a Horizon profile".to_owned());
    }
    let version_id = instance.version_id.as_deref().ok_or_else(|| "install a Minecraft version before adding Fabric".to_owned())?;
    let game_version = loaders::base_game_version(Path::new(&instance.game_dir), version_id).map_err(display_error)?;
    let game_dir = PathBuf::from(&instance.game_dir);
    let installed_version = tauri::async_runtime::spawn_blocking(move || {
        loaders::install_fabric(&game_dir, &game_version, &loader_version)
    })
    .await
    .map_err(display_error)?
    .map_err(display_error)?;
    state.database.set_instance_version(&instance_id, &installed_version).map_err(display_error)?;
    state.database.get_instance(&instance_id).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn install_forge_loader(
    instance_id: String,
    loader_version: String,
    java_path: String,
    state: State<'_, AppState>,
) -> std::result::Result<Instance, String> {
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Minecraft {
        return Err("Forge cannot be installed on a Horizon profile".to_owned());
    }
    let version_id = instance.version_id.as_deref().ok_or_else(|| "install a Minecraft version before adding Forge".to_owned())?;
    let game_version = loaders::base_game_version(Path::new(&instance.game_dir), version_id).map_err(display_error)?;
    let game_dir = PathBuf::from(&instance.game_dir);
    let java_path = PathBuf::from(java_path);
    let installed_version = tauri::async_runtime::spawn_blocking(move || {
        loaders::install_forge(&game_dir, &game_version, &loader_version, &java_path)
    })
    .await
    .map_err(display_error)?
    .map_err(display_error)?;
    state.database.set_instance_version(&instance_id, &installed_version).map_err(display_error)?;
    state.database.get_instance(&instance_id).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn set_instance_version(
    instance_id: String,
    version_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<Instance, String> {
    if !minecraft::is_safe_id(&version_id) {
        return Err("version identifier is invalid".to_owned());
    }
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Minecraft {
        return Err("Java versions cannot be selected for Horizon profiles".to_owned());
    }
    let installation = minecraft::inspect_installation(Path::new(&instance.game_dir)).map_err(display_error)?;
    if !installation.versions.iter().any(|version| version.id == version_id) {
        return Err("the selected version is not installed in this instance folder".to_owned());
    }
    state.database.set_instance_version(&instance_id, &version_id).map_err(display_error)?;
    state.database.get_instance(&instance_id).map_err(display_error)
}

#[tauri::command]
fn list_offline_profiles(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<OfflineProfile>, String> {
    if !offline_profiles_enabled() {
        return Err("development-only offline profiles are disabled in this build".to_owned());
    }
    state.database.list_offline_profiles().map_err(display_error)
}

#[tauri::command]
fn list_microsoft_accounts(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<MicrosoftAccount>, String> {
    state.database.list_microsoft_accounts().map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
async fn add_microsoft_account(
    client_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<MicrosoftAccount, String> {
    let database = Arc::clone(&state.database);
    tauri::async_runtime::spawn_blocking(move || {
        let client_id = client_id.trim().to_owned();
        let (account, _) = crate::microsoft_auth::sign_in(&client_id)?;
        if let Err(error) = database.set_setting("microsoft_client_id", &client_id) {
            let _ = crate::microsoft_auth::remove_credentials(&account.id);
            return Err(display_error(error));
        }
        if let Err(error) = database.save_microsoft_account(&account.name, &account.uuid) {
            let _ = crate::microsoft_auth::remove_credentials(&account.id);
            return Err(display_error(error));
        }
        database
            .get_microsoft_account(&account.id)
            .map_err(display_error)
    })
    .await
    .map_err(|_| "Microsoft sign-in stopped unexpectedly.".to_owned())?
}

#[tauri::command(rename_all = "camelCase")]
fn remove_microsoft_account(
    account_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let account = state.database.get_microsoft_account(&account_id).map_err(display_error)?;
    crate::microsoft_auth::remove_credentials(&account.id)?;
    state.database.remove_microsoft_account(&account.id).map_err(display_error)
}

fn offline_launch_profile(profile: OfflineProfile) -> LaunchProfile {
    LaunchProfile {
        name: profile.name,
        uuid: profile.uuid,
        access_token: String::new(),
        user_type: "legacy".to_owned(),
        xuid: String::new(),
        verified: false,
    }
}

fn resolve_launch_profile(state: &AppState, profile_id: &str) -> std::result::Result<LaunchProfile, String> {
    if profile_id.starts_with("microsoft:") {
        let account = state.database.get_microsoft_account(profile_id).map_err(display_error)?;
        let client_id = state.database.setting("microsoft_client_id")
            .map_err(display_error)?
            .ok_or_else(|| "Enter the Microsoft app registration client ID on the Accounts page, then sign in again.".to_owned())?;
        crate::microsoft_auth::refresh(&account, &client_id)
    } else {
        state.database.get_offline_profile(profile_id)
            .map(offline_launch_profile)
            .map_err(display_error)
    }
}

#[tauri::command]
fn create_offline_profile(
    name: String,
    state: State<'_, AppState>,
) -> std::result::Result<OfflineProfile, String> {
    if !offline_profiles_enabled() {
        return Err("development-only offline profiles are disabled in this build".to_owned());
    }
    state.database.create_offline_profile(&name).map_err(display_error)
}

async fn build_launch_plan_with_repair(
    instance: Instance,
    profile: LaunchProfile,
    java_path: PathBuf,
    native_cache: PathBuf,
    download_id: String,
    active_downloads: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    app: AppHandle,
) -> std::result::Result<LaunchPlan, String> {
    Uuid::parse_str(&download_id).map_err(|_| "download identifier is invalid".to_owned())?;
    let cancellation = Arc::new(AtomicBool::new(false));
    {
        let mut active = active_downloads.lock().map_err(display_error)?;
        if active.contains_key(&download_id) {
            return Err("a download with this identifier is already active".to_owned());
        }
        active.insert(download_id.clone(), Arc::clone(&cancellation));
    }
    let version_id = instance.version_id.clone().unwrap_or_else(|| "Minecraft".to_owned());
    let _ = app.emit("download-progress", DownloadProgress {
        download_id: download_id.clone(),
        version_id: version_id.clone(),
        phase: "Checking Minecraft files".to_owned(),
        current_file: String::new(),
        completed_files: 0,
        total_files: 0,
        downloaded_bytes: 0,
        total_bytes: 0,
    });
    let progress_app = app.clone();
    let progress_sink: ProgressSink = Arc::new(move |progress: DownloadProgress| {
        let _ = progress_app.emit("download-progress", progress);
    });
    let worker_download_id = download_id.clone();
    let worker_cancel = Arc::clone(&cancellation);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let selected_version = instance.version_id.as_deref().ok_or_else(|| {
            crate::error::LauncherError::InvalidInput("select a Minecraft version first".to_owned())
        })?;
        let missing = crate::launch::missing_minecraft_files(Path::new(&instance.game_dir), selected_version)?;
        let repaired = !missing.is_empty();
        if repaired {
            downloads::repair_missing_instance_files(
                selected_version,
                Path::new(&instance.game_dir),
                &worker_download_id,
                Arc::clone(&worker_cancel),
                progress_sink,
            )?;
        }
        if worker_cancel.load(Ordering::Relaxed) {
            return Err(crate::error::LauncherError::Unsupported("launch file repair cancelled".to_owned()));
        }
        let plan = build_launch_plan(LaunchRequest {
            instance: &instance,
            profile: &profile,
            java_path: &java_path,
            native_cache: &native_cache,
            launcher_version: env!("CARGO_PKG_VERSION"),
            prepare_natives: true,
        })?;
        Ok((plan, repaired))
    }).await;
    if let Ok(mut active) = active_downloads.lock() {
        active.remove(&download_id);
    }
    let (plan, repaired) = result.map_err(display_error)?.map_err(display_error)?;
    let _ = app.emit("download-progress", DownloadProgress {
        download_id,
        version_id,
        phase: if repaired { "Installation complete" } else { "Minecraft files checked" }.to_owned(),
        current_file: String::new(),
        completed_files: u64::from(repaired),
        total_files: u64::from(repaired),
        downloaded_bytes: 0,
        total_bytes: 0,
    });
    Ok(plan)
}

#[tauri::command(rename_all = "camelCase")]
async fn launch_instance(
    instance_id: String,
    profile_id: String,
    download_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> std::result::Result<LaunchStarted, String> {
    if !profile_id.starts_with("microsoft:") && !offline_profiles_enabled() {
        return Err("Development-only local profiles are disabled in this build. Choose a Microsoft account.".to_owned());
    }
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Minecraft {
        return Err("Horizon profiles use the Horizon launch pipeline".to_owned());
    }
    let profile = resolve_launch_profile(&state, &profile_id)?;
    let discovered = java::discover_runtimes();
    let java_path = instance
        .java_path
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| discovered.first().map(|runtime| PathBuf::from(&runtime.path)))
        .ok_or_else(|| "No Java runtime was found. Install Java and set JAVA_HOME.".to_owned())?;
    let native_cache = state.data_dir.join("cache").join("natives");
    let plan = build_launch_plan_with_repair(
        instance.clone(),
        profile.clone(),
        java_path.clone(),
        native_cache,
        download_id,
        Arc::clone(&state.active_downloads),
        app.clone(),
    ).await?;
    let mut command = Command::new(&plan.executable);
    command.args(&plan.arguments).current_dir(&plan.working_directory);
    start_game_process(
        command,
        &instance,
        &state,
        &app,
        plan.native_directory.clone(),
        &[profile.access_token.as_str()],
        &format!(
            "Starting {} ({}) with {}",
            instance.name,
            instance.version_id.as_deref().unwrap_or("no version"),
            java_path.display()
        ),
    )
}

#[tauri::command(rename_all = "camelCase")]
async fn launch_horizon_instance(
    instance_id: String,
    profile_id: String,
    width: u32,
    height: u32,
    fullscreen: bool,
    download_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> std::result::Result<LaunchStarted, String> {
    if !offline_profiles_enabled() {
        return Err("local Horizon profiles are disabled in this build".to_owned());
    }
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Horizon {
        return Err("select a Horizon instance before launching Horizon".to_owned());
    }
    if profile_id.starts_with("microsoft:") {
        return Err("Horizon currently uses local test profiles. Select a local account for Horizon; Microsoft sign-in is used for Minecraft Java launches.".to_owned());
    }
    let offline_profile = state.database.get_offline_profile(&profile_id).map_err(display_error)?;
    let profile = offline_launch_profile(offline_profile.clone());
    if instance.horizon_fabric_enabled {
        let runtime_dir = horizon_fabric_game_directory(&instance, false)?;
        let version_id = instance.horizon_fabric_version_id.as_deref()
            .ok_or_else(|| "install the isolated Fabric runtime before launching this profile".to_owned())?;
        let java_path = instance.horizon_fabric_java_path.as_deref()
            .ok_or_else(|| "choose a Java runtime for this profile's Fabric compatibility mode".to_owned())?;
        let java_runtime = java::inspect_runtime(Path::new(java_path)).map_err(display_error)?;
        if java_runtime.major_version < 25 {
            return Err(format!("Minecraft 26.3 Fabric needs Java 25 or newer; selected Java is {}", java_runtime.major_version));
        }
        let installed = loaders::installed_profiles(&runtime_dir).map_err(display_error)?;
        if !installed.iter().any(|installed| {
            installed.version_id == version_id
                && installed.minecraft_version == "26.3"
                && installed.loader == "fabric"
        }) {
            return Err("the isolated Fabric profile is incomplete. Reinstall Fabric from Horizon settings.".to_owned());
        }
        let mut java_instance = instance.clone();
        java_instance.game_dir = runtime_dir.to_string_lossy().into_owned();
        java_instance.version_id = Some(version_id.to_owned());
        java_instance.java_path = Some(java_path.to_owned());
        let mut plan = build_launch_plan_with_repair(
            java_instance,
            profile.clone(),
            PathBuf::from(java_path),
            state.data_dir.join("cache/natives"),
            download_id,
            Arc::clone(&state.active_downloads),
            app.clone(),
        ).await?;
        set_launch_argument(&mut plan.arguments, "--width", width.to_string());
        set_launch_argument(&mut plan.arguments, "--height", height.to_string());
        plan.arguments.retain(|argument| argument != "--fullscreen");
        if fullscreen {
            plan.arguments.push("--fullscreen".to_owned());
        }
        let mut command = Command::new(&plan.executable);
        command.args(&plan.arguments).current_dir(&plan.working_directory);
        return start_game_process(
            command,
            &instance,
            &state,
            &app,
            plan.native_directory.clone(),
            &[],
            &format!(
                "Starting {} with the optional Java Minecraft + Fabric runtime ({}) and local profile {}. The Rust Horizon engine is unchanged and is not used for this launch.",
                instance.name, version_id, profile.name
            ),
        );
    }
    let client_path = horizon_runtime_path(&app)?;
    let resources_path = state.data_dir.join("horizon/resources");
    let source_root = horizon_source_root(&app)?;
    let missing = horizon_prepare::check(&source_root, &resources_path).map_err(display_error)?;
    if !missing.is_empty() {
        return Err(format!("Horizon resources are not ready: {}", missing.join("; ")));
    }
    let bridge_summary = horizon::prepare_fabric_bridge(&instance).map_err(display_error)?;
    let mut command = horizon::offline_command(
        &instance,
        &offline_profile,
        &client_path,
        &resources_path,
        width,
        height,
        fullscreen,
    )
    .map_err(display_error)?;
    let code_bridge_summary = configure_horizon_fabric_code_bridge(
        &instance,
        &profile,
        &state,
        &app,
        &mut command,
    )?;
    let code_bridge_summary = code_bridge_summary.unwrap_or_else(|| "Experimental Fabric code bridge is off.".to_owned());
    start_game_process(
        command,
        &instance,
        &state,
        &app,
        None,
        &[],
        &format!(
            "Starting Horizon {} offline with local profile {} (telemetry disabled)\n{}",
            instance.version_id.as_deref().unwrap_or("unknown"),
            profile.name,
            format!("{bridge_summary}\n{code_bridge_summary}")
        ),
    )
}

fn start_game_process(
    mut command: Command,
    instance: &Instance,
    state: &AppState,
    app: &AppHandle,
    native_directory: Option<PathBuf>,
    secrets_to_redact: &[&str],
    start_message: &str,
) -> std::result::Result<LaunchStarted, String> {
    let logs_dir = state.data_dir.join("logs");
    fs::create_dir_all(&logs_dir).map_err(display_error)?;
    let log_path = logs_dir.join(format!("{}-{}.log", instance.id, unix_time_millis()));
    let log_file = File::create(&log_path).map_err(display_error)?;
    let log_file = Arc::new(Mutex::new(log_file));
    append_log(&log_file, &format!("{start_message}\n")).map_err(display_error)?;

    let mut child = match command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            if let Some(path) = native_directory.as_deref() {
                let _ = fs::remove_dir_all(path);
            }
            return Err(format!("could not start game: {error}"));
        }
    };
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let app_for_wait = app.clone();
    let database = state.database_pointer();
    let thread_instance_id = instance.id.clone();
    let thread_log_path = log_path.clone();
    let app_for_output = app.clone();
    let cleanup_native_directory = native_directory;
    let playtime_session_id = match state.database.start_play_session(&instance.id) {
        Ok(session_id) => session_id,
        Err(error) => {
            let _ = append_log(&log_file, &format!("Playtime tracking could not start: {error}\n"));
            None
        }
    };
    let secrets_to_redact = Arc::new(
        secrets_to_redact
            .iter()
            .filter(|secret| secret.len() >= 8)
            .map(|secret| (*secret).to_owned())
            .collect::<Vec<_>>(),
    );
    let mut readers = Vec::new();
    if let Some(stdout) = stdout {
        readers.push(stream_output(
            stdout,
            "stdout",
            instance.id.clone(),
            log_file.clone(),
            app_for_output.clone(),
            Arc::clone(&secrets_to_redact),
        ));
    }
    if let Some(stderr) = stderr {
        readers.push(stream_output(
            stderr,
            "stderr",
            instance.id.clone(),
            log_file.clone(),
            app_for_output,
            Arc::clone(&secrets_to_redact),
        ));
    }
    std::thread::spawn(move || {
        let status = if playtime_session_id.is_some() {
            let mut last_playtime_checkpoint = Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => break Ok(status),
                    Ok(None) => {
                        if last_playtime_checkpoint.elapsed() >= Duration::from_secs(10) {
                            if let (Some(database), Some(session_id)) = (&database, &playtime_session_id) {
                                let _ = database.update_play_session(session_id);
                            }
                            last_playtime_checkpoint = Instant::now();
                        }
                        std::thread::sleep(Duration::from_secs(1));
                    }
                    Err(error) => break Err(error),
                }
            }
        } else {
            child.wait()
        };
        for reader in readers {
            let _ = reader.join();
        }
        if let Some(path) = cleanup_native_directory {
            if let Err(error) = fs::remove_dir_all(&path) {
                let _ = append_log(
                    &log_file,
                    &format!("Could not remove temporary native library cache: {error}\n"),
                );
            }
        }
        let (success, code) = match status {
            Ok(status) => (status.success(), status.code()),
            Err(error) => {
                let _ = append_log(&log_file, &format!("Could not read process exit status: {error}\n"));
                (false, None)
            }
        };
        let event = LaunchExitEvent {
            instance_id: thread_instance_id.clone(),
            success,
            code,
            log_path: thread_log_path.to_string_lossy().into_owned(),
        };
        let _ = app_for_wait.emit("launch-exit", event);
        if let Some(database) = database {
            let _ = database.touch_instance(&thread_instance_id);
            if let Some(session_id) = playtime_session_id {
                if let Err(error) = database.finish_play_session(&session_id) {
                    let _ = append_log(&log_file, &format!("Playtime tracking could not finish: {error}\n"));
                }
            }
        }
    });

    let _ = state.database.touch_instance(&instance.id);
    Ok(LaunchStarted {
        pid,
        log_path: log_path.to_string_lossy().into_owned(),
    })
}

fn offline_profiles_enabled() -> bool {
    cfg!(feature = "dev-offline-profiles")
}

#[cfg(test)]
mod tests {
    use super::{copy_existing_horizon_mods, import_horizon_sound_assets, offline_profiles_enabled, redact_sensitive_output, set_launch_argument};
    use crate::models::InstanceEngine;
    use sha1::{Digest, Sha1};
    use std::fs;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    fn sha1_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha1::digest(bytes))
    }

    fn write_fixture_file(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn copies_existing_horizon_jars_without_replacing_or_removing_sources() {
        let test_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("horizon-fabric-copy-test-{}", Uuid::new_v4()));
        let source_mods = test_root.join("mods");
        let runtime_dir = test_root.join(".opnlauncher/fabric-java");
        let destination_mods = runtime_dir.join("mods");
        fs::create_dir_all(&source_mods).unwrap();
        fs::create_dir_all(&destination_mods).unwrap();
        fs::write(source_mods.join("fabric-api.jar"), b"source api bytes").unwrap();
        fs::write(source_mods.join("sodium.jar.disabled"), b"disabled sodium bytes").unwrap();
        fs::write(source_mods.join("notes.txt"), b"not a mod").unwrap();
        fs::write(destination_mods.join("fabric-api.jar"), b"existing destination").unwrap();

        let instance = crate::models::Instance {
            id: Uuid::new_v4().to_string(),
            name: "Fabric copy test".to_owned(),
            engine: InstanceEngine::Horizon,
            game_dir: test_root.to_string_lossy().into_owned(),
            version_id: Some("26.3".to_owned()),
            java_path: None,
            horizon_fabric_enabled: false,
            horizon_fabric_version_id: None,
            horizon_fabric_java_path: None,
            memory_mb: 2048,
            created_at: 0,
            last_played_at: None,
        };

        assert_eq!(copy_existing_horizon_mods(&instance, &runtime_dir).unwrap(), 1);
        assert_eq!(fs::read(source_mods.join("fabric-api.jar")).unwrap(), b"source api bytes");
        assert_eq!(fs::read(destination_mods.join("fabric-api.jar")).unwrap(), b"existing destination");
        assert_eq!(fs::read(destination_mods.join("sodium.jar.disabled")).unwrap(), b"disabled sodium bytes");
        assert!(!destination_mods.join("notes.txt").exists());
        fs::remove_dir_all(test_root).unwrap();
    }

    #[test]
    fn updates_java_window_arguments_for_the_selected_horizon_profile() {
        let mut arguments = vec!["--width".to_owned(), "1280".to_owned(), "--height".to_owned(), "720".to_owned()];
        set_launch_argument(&mut arguments, "--width", "1600".to_owned());
        set_launch_argument(&mut arguments, "--height", "900".to_owned());
        assert_eq!(arguments, ["--width", "1600", "--height", "900"]);
        set_launch_argument(&mut arguments, "--fullscreen", "true".to_owned());
        assert!(arguments.windows(2).any(|pair| pair == ["--fullscreen", "true"]));
    }

    #[test]
    fn complete_horizon_preparation_imports_sound_assets_before_checking() {
        let test_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("horizon-sounds-test-{}", Uuid::new_v4()));
        let prepare_root = test_root.join("prepare");
        let minecraft_dir = test_root.join("minecraft");
        let resources_dir = test_root.join("resources");
        let sounds_json = br#"{"test:event":{}}"#;
        let sound_file = b"OggS fixture sound";
        let sounds_hash = sha1_hex(sounds_json);
        let sound_hash = sha1_hex(sound_file);
        let asset_index = serde_json::to_vec(&serde_json::json!({
            "objects": {
                "minecraft/sounds.json": {"hash": sounds_hash, "size": sounds_json.len()},
                "minecraft/sounds/test/event.ogg": {"hash": sound_hash, "size": sound_file.len()}
            }
        }))
        .unwrap();
        let asset_index_hash = sha1_hex(&asset_index);
        let client_hash = sha1_hex(b"fixture client");
        let server_hash = sha1_hex(b"fixture server");
        let version_json = serde_json::to_vec(&serde_json::json!({
            "id": "26.3",
            "javaVersion": {"majorVersion": 25},
            "downloads": {
                "client": {"sha1": client_hash},
                "server": {"sha1": server_hash}
            },
            "assetIndex": {"id": "34", "sha1": asset_index_hash}
        }))
        .unwrap();
        let version_hash = sha1_hex(&version_json);
        let lock = serde_json::json!({
            "minecraft": {
                "version": "26.3",
                "java_major": 25,
                "manifest_sha1": version_hash,
                "client_sha1": client_hash,
                "server_sha1": server_hash
            }
        });
        write_fixture_file(
            &prepare_root.join("research-lock.json"),
            lock.to_string().as_bytes(),
        );
        write_fixture_file(
            &prepare_root.join(".local/minecraft/26.3/manifest.json"),
            &version_json,
        );
        write_fixture_file(
            &minecraft_dir.join("versions/26.3/26.3.json"),
            &version_json,
        );
        write_fixture_file(&minecraft_dir.join("assets/indexes/34.json"), &asset_index);
        for (hash, bytes) in [
            (sounds_hash, sounds_json.as_slice()),
            (sound_hash, sound_file.as_slice()),
        ] {
            write_fixture_file(
                &minecraft_dir
                    .join("assets/objects")
                    .join(&hash[..2])
                    .join(hash),
                bytes,
            );
        }

        import_horizon_sound_assets(
            &prepare_root,
            &minecraft_dir,
            &resources_dir,
            &|_| {},
        )
        .unwrap();

        assert_eq!(
            fs::read(resources_dir.join("assets/minecraft/sounds.json")).unwrap(),
            sounds_json
        );
        assert_eq!(
            fs::read(resources_dir.join("assets/minecraft/sounds/test/event.ogg")).unwrap(),
            sound_file
        );
        assert!(horizon_prepare::missing_extras(&resources_dir)
            .iter()
            .all(|(command, _)| *command != "sounds"));
        fs::remove_dir_all(test_root).unwrap();
    }

    #[test]
    fn developer_profiles_remain_enabled_in_release_builds_with_the_feature() {
        assert!(cfg!(feature = "dev-offline-profiles"));
        assert!(offline_profiles_enabled());
    }

    #[test]
    fn launch_output_redacts_minecraft_access_tokens() {
        let secrets = vec!["minecraft-session-token-value".to_owned()];
        let output = redact_sensitive_output(
            "Authentication token: minecraft-session-token-value",
            &secrets,
        );
        assert_eq!(output, "Authentication token: [REDACTED]");
    }
}

fn stream_output<R: Read + Send + 'static>(
    reader: R,
    stream: &'static str,
    instance_id: String,
    log_file: Arc<Mutex<File>>,
    app: AppHandle,
    secrets_to_redact: Arc<Vec<String>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let mut safe_line = redact_sensitive_output(&line, &secrets_to_redact);
                    if safe_line.len() > 8192 {
                        safe_line.truncate(8192);
                    }
                    let _ = append_log(&log_file, &format!("[{stream}] {safe_line}"));
                    let _ = app.emit(
                        "launch-log",
                        LaunchLogEvent {
                            instance_id: instance_id.clone(),
                            stream: stream.to_owned(),
                            text: safe_line.trim_end().to_owned(),
                        },
                    );
                }
            }
        }
    })
}

fn redact_sensitive_output(text: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .fold(text.to_owned(), |output, secret| output.replace(secret, "[REDACTED]"))
}

fn append_log(file: &Arc<Mutex<File>>, text: &str) -> std::io::Result<()> {
    let mut file = file.lock().map_err(|_| std::io::Error::other("log lock was poisoned"))?;
    file.write_all(text.as_bytes())?;
    file.flush()
}

fn unix_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

// The database itself lives for the process lifetime, so a cloned shared handle lets the
// process monitor update the last-played timestamp after the Tauri command returns.
impl AppState {
    fn database_pointer(&self) -> Option<Arc<Database>> {
        Some(Arc::clone(&self.database))
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let database = Database::open(&data_dir.join("opnlauncher.sqlite3"))
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(AppState {
                database: Arc::new(database),
                data_dir,
                active_downloads: Arc::new(Mutex::new(HashMap::new())),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_instances,
            get_playtime_tracking_enabled,
            set_playtime_tracking_enabled,
            get_playtime_summary,
            scan_installations,
            inspect_horizon_engine,
            list_official_versions,
            search_mod_projects,
            get_mod_project,
            get_mod_project_versions,
            get_all_mod_project_versions,
            inspect_instance_mod_context,
            list_installed_mods,
            set_mod_enabled,
            remove_mod_file,
            remove_mod_files,
            resolve_installed_mod_source,
            link_installed_mod_to_project,
            install_mod_version,
            switch_mod_version,
            install_minecraft_version,
            install_instance_setup,
            install_horizon_fabric_setup,
            cancel_download,
            inspect_installation,
            register_installation,
            create_instance,
            create_horizon_instance,
            prepare_horizon_resources,
            list_horizon_mods,
            get_horizon_fabric_bridge_status,
            set_horizon_fabric_bridge_enabled,
            set_horizon_fabric_code_bridge_enabled,
            set_horizon_fabric_runtime_enabled,
            install_horizon_mod,
            set_horizon_mod_enabled,
            remove_horizon_mod,
            rename_instance,
            forget_instance,
            open_instance_folder,
            set_instance_java,
            set_instance_memory,
            list_java_runtimes,
            inspect_java_runtime,
            list_loader_versions,
            list_installed_loader_profiles,
            install_fabric_loader,
            install_forge_loader,
            set_instance_version,
            list_offline_profiles,
            create_offline_profile,
            list_microsoft_accounts,
            add_microsoft_account,
            remove_microsoft_account,
            launch_instance,
            launch_horizon_instance,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run OpnLauncher");
}
