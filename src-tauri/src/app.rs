use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::database::Database;
use crate::downloads::{self, DownloadProgress, ProgressSink, RemoteMinecraftVersion};
use crate::horizon::{self, HorizonEngineStatus, HorizonMod};
use crate::java;
use crate::launch::{build_launch_plan, LaunchRequest};
use crate::loaders::{self, InstalledLoaderProfile, LoaderVersion};
use crate::minecraft;
use crate::mods::{self, ModDownloadProgress};
use crate::models::{
    Instance, InstanceEngine, JavaRuntime, LaunchExitEvent, LaunchLogEvent, LaunchStarted,
    MinecraftInstallation, OfflineProfile,
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

        let prepare_root = horizon_dir.join(format!("preparer-source-{}", &horizon::HORIZON_SOURCE_REVISION[..8]));
        horizon::stage_preparer_inputs(&bundled_source_root, &prepare_root).map_err(display_error)?;

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
fn create_offline_profile(
    name: String,
    state: State<'_, AppState>,
) -> std::result::Result<OfflineProfile, String> {
    if !offline_profiles_enabled() {
        return Err("development-only offline profiles are disabled in this build".to_owned());
    }
    state.database.create_offline_profile(&name).map_err(display_error)
}

#[tauri::command(rename_all = "camelCase")]
fn launch_instance(
    instance_id: String,
    profile_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> std::result::Result<LaunchStarted, String> {
    if !offline_profiles_enabled() {
        return Err("Minecraft launching requires an authenticated account in this build".to_owned());
    }
    let instance = state.database.get_instance(&instance_id).map_err(display_error)?;
    if instance.engine != InstanceEngine::Minecraft {
        return Err("Horizon profiles use the Horizon launch pipeline".to_owned());
    }
    let profile = state
        .database
        .get_offline_profile(&profile_id)
        .map_err(display_error)?;
    let discovered = java::discover_runtimes();
    let java_path = instance
        .java_path
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| discovered.first().map(|runtime| PathBuf::from(&runtime.path)))
        .ok_or_else(|| "No Java runtime was found. Install Java and set JAVA_HOME.".to_owned())?;
    let native_cache = state.data_dir.join("cache").join("natives");
    let plan = build_launch_plan(LaunchRequest {
        instance: &instance,
        profile: &profile,
        java_path: &java_path,
        native_cache: &native_cache,
        launcher_version: env!("CARGO_PKG_VERSION"),
    })
    .map_err(display_error)?;
    let mut command = Command::new(&plan.executable);
    command.args(&plan.arguments).current_dir(&plan.working_directory);
    start_game_process(
        command,
        &instance,
        &state,
        &app,
        &format!(
            "Starting {} ({}) with {}",
            instance.name,
            instance.version_id.as_deref().unwrap_or("no version"),
            java_path.display()
        ),
    )
}

#[tauri::command(rename_all = "camelCase")]
fn launch_horizon_instance(
    instance_id: String,
    profile_id: String,
    width: u32,
    height: u32,
    fullscreen: bool,
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
    let profile = state.database.get_offline_profile(&profile_id).map_err(display_error)?;
    let client_path = horizon_runtime_path(&app)?;
    let resources_path = state.data_dir.join("horizon/resources");
    let source_root = horizon_source_root(&app)?;
    let missing = horizon_prepare::check(&source_root, &resources_path).map_err(display_error)?;
    if !missing.is_empty() {
        return Err(format!("Horizon resources are not ready: {}", missing.join("; ")));
    }
    let command = horizon::offline_command(
        &instance,
        &profile,
        &client_path,
        &resources_path,
        width,
        height,
        fullscreen,
    )
    .map_err(display_error)?;
    start_game_process(
        command,
        &instance,
        &state,
        &app,
        &format!(
            "Starting Horizon {} offline with local profile {} (telemetry disabled)",
            instance.version_id.as_deref().unwrap_or("unknown"),
            profile.name
        ),
    )
}

fn start_game_process(
    mut command: Command,
    instance: &Instance,
    state: &AppState,
    app: &AppHandle,
    start_message: &str,
) -> std::result::Result<LaunchStarted, String> {
    let logs_dir = state.data_dir.join("logs");
    fs::create_dir_all(&logs_dir).map_err(display_error)?;
    let log_path = logs_dir.join(format!("{}-{}.log", instance.id, unix_time_millis()));
    let log_file = File::create(&log_path).map_err(display_error)?;
    let log_file = Arc::new(Mutex::new(log_file));
    append_log(&log_file, &format!("{start_message}\n")).map_err(display_error)?;

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start game: {error}"))?;
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let app_for_wait = app.clone();
    let database = state.database_pointer();
    let thread_instance_id = instance.id.clone();
    let thread_log_path = log_path.clone();
    let app_for_output = app.clone();
    let mut readers = Vec::new();
    if let Some(stdout) = stdout {
        readers.push(stream_output(
            stdout,
            "stdout",
            instance.id.clone(),
            log_file.clone(),
            app_for_output.clone(),
        ));
    }
    if let Some(stderr) = stderr {
        readers.push(stream_output(
            stderr,
            "stderr",
            instance.id.clone(),
            log_file.clone(),
            app_for_output,
        ));
    }
    std::thread::spawn(move || {
        let status = child.wait();
        for reader in readers {
            let _ = reader.join();
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
    use super::offline_profiles_enabled;

    #[test]
    fn developer_profiles_remain_enabled_in_release_builds_with_the_feature() {
        assert!(cfg!(feature = "dev-offline-profiles"));
        assert!(offline_profiles_enabled());
    }
}

fn stream_output<R: Read + Send + 'static>(
    reader: R,
    stream: &'static str,
    instance_id: String,
    log_file: Arc<Mutex<File>>,
    app: AppHandle,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if line.len() > 8192 {
                        line.truncate(8192);
                    }
                    let _ = append_log(&log_file, &format!("[{stream}] {line}"));
                    let _ = app.emit(
                        "launch-log",
                        LaunchLogEvent {
                            instance_id: instance_id.clone(),
                            stream: stream.to_owned(),
                            text: line.trim_end().to_owned(),
                        },
                    );
                }
            }
        }
    })
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
            cancel_download,
            inspect_installation,
            register_installation,
            create_instance,
            create_horizon_instance,
            prepare_horizon_resources,
            list_horizon_mods,
            install_horizon_mod,
            set_horizon_mod_enabled,
            remove_horizon_mod,
            rename_instance,
            forget_instance,
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
            launch_instance,
            launch_horizon_instance,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run OpnLauncher");
}
