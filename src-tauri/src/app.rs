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
use crate::java;
use crate::launch::{build_launch_plan, LaunchRequest};
use crate::minecraft;
use crate::mods::{self, ModDownloadProgress};
use crate::models::{
    Instance, JavaRuntime, LaunchExitEvent, LaunchLogEvent, LaunchStarted, MinecraftInstallation,
    OfflineProfile,
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
    state.database.set_memory(&instance_id, memory_mb).map_err(display_error)
}

#[tauri::command]
fn list_java_runtimes() -> Vec<JavaRuntime> {
    java::discover_runtimes()
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

    let logs_dir = state.data_dir.join("logs");
    fs::create_dir_all(&logs_dir).map_err(display_error)?;
    let log_path = logs_dir.join(format!("{}-{}.log", instance.id, unix_time_millis()));
    let log_file = File::create(&log_path).map_err(display_error)?;
    let log_file = Arc::new(Mutex::new(log_file));
    append_log(&log_file, &format!("Starting {} ({}) with {}\n", instance.name, instance.version_id.as_deref().unwrap_or("no version"), java_path.display()))
        .map_err(display_error)?;

    let mut child = Command::new(&plan.executable)
        .args(&plan.arguments)
        .current_dir(&plan.working_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start Java: {error}"))?;
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
            list_official_versions,
            search_mod_projects,
            get_mod_project,
            get_mod_project_versions,
            inspect_instance_mod_context,
            list_installed_mods,
            set_mod_enabled,
            remove_mod_file,
            install_mod_version,
            install_minecraft_version,
            cancel_download,
            inspect_installation,
            register_installation,
            create_instance,
            rename_instance,
            forget_instance,
            set_instance_java,
            set_instance_memory,
            list_java_runtimes,
            list_offline_profiles,
            create_offline_profile,
            launch_instance,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run OpnLauncher");
}
