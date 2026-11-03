use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::{Client, Response};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use uuid::Uuid;

use crate::error::{LauncherError, Result};
use crate::launch;
use crate::minecraft::is_safe_id;

const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const MAX_METADATA_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_INSTALL_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_DOWNLOAD_FILES: usize = 100_000;
const WORKER_COUNT: usize = 8;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMinecraftVersion {
    pub id: String,
    pub kind: String,
    pub release_time: String,
    pub url: String,
    pub sha1: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub download_id: String,
    pub version_id: String,
    pub phase: String,
    pub current_file: String,
    pub completed_files: u64,
    pub total_files: u64,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

pub type ProgressSink = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestEntry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    release_time: String,
    url: String,
    sha1: Option<String>,
}

#[derive(Clone)]
enum DownloadSource {
    Url(String),
    Bytes(Arc<Vec<u8>>),
}

#[derive(Clone)]
struct DownloadFile {
    path: PathBuf,
    source: DownloadSource,
    sha1: Option<String>,
    size: Option<u64>,
}

struct ProgressState {
    completed_files: AtomicU64,
    downloaded_bytes: AtomicU64,
    current_file: Mutex<String>,
    phase: Mutex<String>,
    last_emit: Mutex<Instant>,
}

struct ProgressReporter {
    download_id: String,
    version_id: String,
    total_files: u64,
    total_bytes: u64,
    state: ProgressState,
    sink: ProgressSink,
}

struct AttemptFailure {
    message: String,
    transferred: u64,
}

struct TemporaryFile {
    path: PathBuf,
    file: Option<File>,
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

impl ProgressReporter {
    fn new(
        download_id: String,
        version_id: String,
        total_files: u64,
        total_bytes: u64,
        sink: ProgressSink,
    ) -> Self {
        Self {
            download_id,
            version_id,
            total_files,
            total_bytes,
            state: ProgressState {
                completed_files: AtomicU64::new(0),
                downloaded_bytes: AtomicU64::new(0),
                current_file: Mutex::new(String::new()),
                phase: Mutex::new("Preparing version files".to_owned()),
                last_emit: Mutex::new(Instant::now()),
            },
            sink,
        }
    }

    fn set_phase(&self, phase: &str) {
        if let Ok(mut current) = self.state.phase.lock() {
            *current = phase.to_owned();
        }
        self.emit(true);
    }

    fn set_current_file(&self, path: &Path) {
        if let Ok(mut current) = self.state.current_file.lock() {
            *current = path.to_string_lossy().into_owned();
        }
        self.emit(true);
    }

    fn add_bytes(&self, count: u64) {
        self.state.downloaded_bytes.fetch_add(count, Ordering::Relaxed);
        self.emit(false);
    }

    fn undo_bytes(&self, count: u64) {
        self.state.downloaded_bytes.fetch_sub(count, Ordering::Relaxed);
    }

    fn complete_file(&self) {
        self.state.completed_files.fetch_add(1, Ordering::Relaxed);
        self.emit(true);
    }

    fn emit(&self, force: bool) {
        let should_emit = self
            .state
            .last_emit
            .lock()
            .map(|mut last| {
                if force || last.elapsed() >= Duration::from_millis(125) {
                    *last = Instant::now();
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if !should_emit {
            return;
        }
        let phase = self
            .state
            .phase
            .lock()
            .map(|value| value.clone())
            .unwrap_or_else(|_| "Downloading".to_owned());
        let current_file = self
            .state
            .current_file
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default();
        (self.sink)(DownloadProgress {
            download_id: self.download_id.clone(),
            version_id: self.version_id.clone(),
            phase,
            current_file,
            completed_files: self.state.completed_files.load(Ordering::Relaxed),
            total_files: self.total_files,
            downloaded_bytes: self.state.downloaded_bytes.load(Ordering::Relaxed),
            total_bytes: self.total_bytes,
        });
    }
}

pub fn available_versions() -> Result<Vec<RemoteMinecraftVersion>> {
    let client = build_client()?;
    let bytes = fetch_bytes(&client, VERSION_MANIFEST_URL)?;
    parse_version_manifest(&bytes)
}

pub fn install_version(
    version_id: &str,
    install_root: &Path,
    download_id: &str,
    cancelled: Arc<AtomicBool>,
    sink: ProgressSink,
) -> Result<()> {
    install_version_with_policy(version_id, install_root, download_id, cancelled, sink, true)
}

pub fn install_version_for_instance(
    version_id: &str,
    install_root: &Path,
    download_id: &str,
    cancelled: Arc<AtomicBool>,
    sink: ProgressSink,
) -> Result<()> {
    install_version_with_policy(version_id, install_root, download_id, cancelled, sink, false)
}

fn install_version_with_policy(
    version_id: &str,
    install_root: &Path,
    download_id: &str,
    cancelled: Arc<AtomicBool>,
    sink: ProgressSink,
    require_empty_root: bool,
) -> Result<()> {
    if !is_safe_id(version_id) {
        return Err(LauncherError::InvalidInput(
            "Minecraft version identifier is invalid".to_owned(),
        ));
    }
    Uuid::parse_str(download_id).map_err(|_| {
        LauncherError::InvalidInput("download identifier is invalid".to_owned())
    })?;
    if require_empty_root {
        validate_empty_root(install_root)?;
    } else {
        validate_install_root(install_root)?;
    }

    let client = build_client()?;
    let manifest = parse_version_manifest(&fetch_bytes(&client, VERSION_MANIFEST_URL)?)?;
    let selected = manifest
        .iter()
        .find(|version| version.id == version_id)
        .ok_or_else(|| LauncherError::NotFound(format!("official Minecraft version {version_id}")))?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(LauncherError::Unsupported("installation cancelled".to_owned()));
    }

    let version_bytes = fetch_bytes(&client, &selected.url)?;
    verify_sha1(&version_bytes, selected.sha1.as_deref(), "version metadata")?;
    let metadata: Value = serde_json::from_slice(&version_bytes)?;
    if metadata.get("id").and_then(Value::as_str) != Some(version_id) {
        return Err(LauncherError::Unsupported(
            "downloaded version metadata has a mismatched identifier".to_owned(),
        ));
    }

    let asset_index = metadata
        .get("assetIndex")
        .ok_or_else(|| LauncherError::Unsupported("version metadata has no asset index".to_owned()))?;
    let asset_index_id = asset_index
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| LauncherError::Unsupported("version asset index has no identifier".to_owned()))?;
    if !is_safe_id(asset_index_id) {
        return Err(LauncherError::Unsupported(
            "version asset index identifier is invalid".to_owned(),
        ));
    }
    let asset_index_url = asset_index
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| LauncherError::Unsupported("version asset index has no download URL".to_owned()))?;
    let asset_index_sha1 = asset_index
        .get("sha1")
        .and_then(Value::as_str)
        .ok_or_else(|| LauncherError::Unsupported("version asset index has no checksum".to_owned()))?;
    let asset_index_bytes = fetch_bytes(&client, asset_index_url)?;
    verify_sha1(&asset_index_bytes, Some(asset_index_sha1), "asset index")?;
    let asset_index_json: Value = serde_json::from_slice(&asset_index_bytes)?;

    let mut files = BTreeMap::new();
    let mut virtual_assets = Vec::new();
    add_payload(
        &mut files,
        PathBuf::from("versions").join(version_id).join(format!("{version_id}.json")),
        Arc::new(version_bytes),
        selected.sha1.clone(),
    )?;
    add_payload(
        &mut files,
        PathBuf::from("assets/indexes").join(format!("{asset_index_id}.json")),
        Arc::new(asset_index_bytes),
        Some(asset_index_sha1.to_ascii_lowercase()),
    )?;

    add_client_download(&metadata, version_id, &mut files)?;
    add_library_downloads(&metadata, &mut files)?;
    add_logging_download(&metadata, &mut files)?;
    add_asset_downloads(&asset_index_json, &mut files, &mut virtual_assets)?;

    if files.is_empty() || files.len() > MAX_DOWNLOAD_FILES {
        return Err(LauncherError::Unsupported(
            "version contains an unsupported number of download files".to_owned(),
        ));
    }
    let total_bytes = files.values().try_fold(0_u64, |total, file| {
        let size = match &file.source {
            DownloadSource::Bytes(bytes) => bytes.len() as u64,
            DownloadSource::Url(_) => file.size.unwrap_or(0),
        };
        total.checked_add(size).filter(|sum| *sum <= MAX_INSTALL_BYTES)
    });
    let total_bytes = total_bytes.ok_or_else(|| {
        LauncherError::Unsupported("version exceeds the 16 GiB installation size limit".to_owned())
    })?;
    let total_files = files.len() as u64;
    let reporter = Arc::new(ProgressReporter::new(
        download_id.to_owned(),
        version_id.to_owned(),
        total_files,
        total_bytes,
        sink,
    ));
    reporter.set_phase("Downloading version files");
    download_files(&client, files.into_values().collect(), install_root, &cancelled, reporter.clone())?;

    if cancelled.load(Ordering::Relaxed) {
        return Err(LauncherError::Unsupported("installation cancelled".to_owned()));
    }
    if asset_index_json.get("virtual").and_then(Value::as_bool) == Some(true) {
        reporter.set_phase("Preparing legacy game assets");
        let assets_id = metadata
            .get("assets")
            .and_then(Value::as_str)
            .unwrap_or(asset_index_id);
        create_virtual_assets(install_root, assets_id, &virtual_assets)?;
    }
    reporter.set_phase("Installation complete");
    Ok(())
}

fn build_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("OpnLauncher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(15 * 60))
        .redirect(Policy::custom(|attempt| {
            if attempt.url().scheme() != "https" {
                return attempt.error("download redirects must use HTTPS");
            }
            if attempt.previous().len() >= 5 {
                return attempt.stop();
            }
            attempt.follow()
        }))
        .build()
        .map_err(|error| LauncherError::Unsupported(format!("cannot initialize secure downloads: {error}")))
}

fn parse_version_manifest(bytes: &[u8]) -> Result<Vec<RemoteMinecraftVersion>> {
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err(LauncherError::Unsupported("version manifest is too large".to_owned()));
    }
    let manifest: VersionManifest = serde_json::from_slice(bytes)?;
    if manifest.versions.is_empty() || manifest.versions.len() > MAX_DOWNLOAD_FILES {
        return Err(LauncherError::Unsupported("version manifest has an invalid entry count".to_owned()));
    }
    let mut seen = std::collections::BTreeSet::new();
    manifest
        .versions
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            if !is_safe_id(&entry.id) {
                return Err(LauncherError::Unsupported(
                    format!("version manifest entry {index} has an unsafe identifier: {:?}", entry.id),
                ));
            }
            if !seen.insert(entry.id.to_lowercase()) {
                return Err(LauncherError::Unsupported(format!(
                    "version manifest entry {index} duplicates identifier {:?}",
                    entry.id
                )));
            }
            if entry.sha1.as_deref().is_some_and(|hash| !is_sha1(hash)) {
                return Err(LauncherError::Unsupported(format!(
                    "version {} has an invalid metadata checksum",
                    entry.id
                )));
            }
            Ok(RemoteMinecraftVersion {
                id: entry.id,
                kind: entry.kind,
                release_time: entry.release_time,
                url: secure_url(&entry.url)?,
                sha1: entry.sha1.map(|hash| hash.to_ascii_lowercase()),
            })
        })
        .collect()
}

fn fetch_bytes(client: &Client, url: &str) -> Result<Vec<u8>> {
    let url = secure_url(url)?;
    let response = client
        .get(url)
        .send()
        .and_then(Response::error_for_status)
        .map_err(|error| LauncherError::Unsupported(format!("download request failed: {error}")))?;
    read_limited(response, MAX_METADATA_BYTES)
}

fn read_limited(mut response: Response, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(LauncherError::Unsupported("remote metadata exceeded its size limit".to_owned()));
    }
    Ok(bytes)
}

fn secure_url(raw: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(raw).map_err(|_| {
        LauncherError::Unsupported("download metadata contains an invalid URL".to_owned())
    })?;
    if url.username() != "" || url.password().is_some() || url.host_str().is_none() {
        return Err(LauncherError::Unsupported(
            "download URL contains unsupported credentials or host data".to_owned(),
        ));
    }
    if url.scheme() == "http" && is_known_mojang_host(url.host_str().unwrap_or_default()) {
        url.set_scheme("https").map_err(|_| {
            LauncherError::Unsupported("could not secure an official download URL".to_owned())
        })?;
    }
    if url.scheme() != "https" {
        return Err(LauncherError::Unsupported(
            "downloads must use HTTPS".to_owned(),
        ));
    }
    Ok(url.to_string())
}

fn is_known_mojang_host(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "piston-meta.mojang.com"
            | "launchermeta.mojang.com"
            | "launcher.mojang.com"
            | "libraries.minecraft.net"
            | "resources.download.minecraft.net"
    )
}

fn add_client_download(
    metadata: &Value,
    version_id: &str,
    files: &mut BTreeMap<PathBuf, DownloadFile>,
) -> Result<()> {
    let client = metadata
        .get("downloads")
        .and_then(|downloads| downloads.get("client"))
        .ok_or_else(|| LauncherError::Unsupported("version metadata has no client download".to_owned()))?;
    let url = client
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| LauncherError::Unsupported("version client download has no URL".to_owned()))?;
    let sha1 = optional_sha1(client.get("sha1"))?;
    let size = optional_size(client.get("size"))?;
    if sha1.is_none() && size.is_none() {
        return Err(LauncherError::Unsupported(
            "version client file has no checksum or expected size".to_owned(),
        ));
    }
    add_url(
        files,
        PathBuf::from("versions").join(version_id).join(format!("{version_id}.jar")),
        url,
        sha1,
        size,
    )
}

fn add_library_downloads(metadata: &Value, files: &mut BTreeMap<PathBuf, DownloadFile>) -> Result<()> {
    let Some(libraries) = metadata.get("libraries").and_then(Value::as_array) else {
        return Ok(());
    };
    for library in libraries {
        if !launch::rules_allow(library.get("rules")) {
            continue;
        }
        let downloads = library.get("downloads");
        let base_url = library
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("https://libraries.minecraft.net/");
        let coordinate = library.get("name").and_then(Value::as_str);
        if let Some(artifact) = downloads.and_then(|value| value.get("artifact")) {
            let relative = artifact
                .get("path")
                .and_then(Value::as_str)
                .map(safe_relative_path)
                .transpose()?
                .or_else(|| coordinate.and_then(|value| launch::maven_path(value).ok()))
                .ok_or_else(|| LauncherError::Unsupported("library artifact has no safe path".to_owned()))?;
            let url = match artifact.get("url").and_then(Value::as_str) {
                Some(url) => url.to_owned(),
                None => repository_url(base_url, &relative)?,
            };
            add_url(
                files,
                PathBuf::from("libraries").join(relative),
                &url,
                optional_sha1(artifact.get("sha1"))?,
                optional_size(artifact.get("size"))?,
            )?;
        } else if let Some(coordinate) = coordinate {
            let relative = launch::maven_path(coordinate)?;
            let url = repository_url(base_url, &relative)?;
            add_url(files, PathBuf::from("libraries").join(relative), &url, None, None)?;
        }

        if let Some((classifier, artifact)) = native_artifact(library) {
            let relative = artifact
                .get("path")
                .and_then(Value::as_str)
                .map(safe_relative_path)
                .transpose()?
                .ok_or_else(|| LauncherError::Unsupported(format!("native library {classifier} has no safe path")))?;
            let url = artifact
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| LauncherError::Unsupported(format!("native library {classifier} has no URL")))?;
            add_url(
                files,
                PathBuf::from("libraries").join(relative),
                url,
                optional_sha1(artifact.get("sha1"))?,
                optional_size(artifact.get("size"))?,
            )?;
        }
    }
    Ok(())
}

fn native_artifact(library: &Value) -> Option<(String, &Value)> {
    let os = if cfg!(target_os = "macos") { "osx" } else { std::env::consts::OS };
    let template = library.get("natives")?.get(os)?.as_str()?;
    let arch = if cfg!(target_pointer_width = "64") { "64" } else { "32" };
    let classifier = template.replace("${arch}", arch);
    let artifact = library.get("downloads")?.get("classifiers")?.get(&classifier)?;
    Some((classifier, artifact))
}

fn add_logging_download(metadata: &Value, files: &mut BTreeMap<PathBuf, DownloadFile>) -> Result<()> {
    let Some(file) = metadata.get("logging").and_then(|logging| logging.get("client"))
        .and_then(|client| client.get("file")) else { return Ok(()) };
    let Some(id) = file.get("id").and_then(Value::as_str) else { return Ok(()) };
    if !is_safe_id(id) {
        return Err(LauncherError::Unsupported("logging configuration identifier is invalid".to_owned()));
    }
    let url = file.get("url").and_then(Value::as_str)
        .ok_or_else(|| LauncherError::Unsupported("logging configuration has no URL".to_owned()))?;
    add_url(
        files,
        PathBuf::from("assets/log_configs").join(id),
        url,
        optional_sha1(file.get("sha1"))?,
        optional_size(file.get("size"))?,
    )
}

fn add_asset_downloads(
    asset_index: &Value,
    files: &mut BTreeMap<PathBuf, DownloadFile>,
    virtual_assets: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<()> {
    let objects = asset_index
        .get("objects")
        .and_then(Value::as_object)
        .ok_or_else(|| LauncherError::Unsupported("asset index has no object map".to_owned()))?;
    if objects.len() > MAX_DOWNLOAD_FILES {
        return Err(LauncherError::Unsupported("asset index contains too many files".to_owned()));
    }
    let is_virtual = asset_index.get("virtual").and_then(Value::as_bool) == Some(true);
    for (logical_name, object) in objects {
        let hash = object
            .get("hash")
            .and_then(Value::as_str)
            .ok_or_else(|| LauncherError::Unsupported("asset index object has no hash".to_owned()))?;
        if !is_sha1(hash) {
            return Err(LauncherError::Unsupported("asset index contains an invalid object hash".to_owned()));
        }
        let size = object
            .get("size")
            .and_then(Value::as_u64)
            .ok_or_else(|| LauncherError::Unsupported("asset index object has no valid size".to_owned()))?;
        let hash = hash.to_ascii_lowercase();
        let relative = PathBuf::from("assets/objects").join(&hash[..2]).join(&hash);
        let url = format!("https://resources.download.minecraft.net/{}/{}", &hash[..2], hash);
        add_url(files, relative.clone(), &url, Some(hash), Some(size))?;
        if is_virtual {
            virtual_assets.push((safe_relative_path(logical_name)?, relative));
        }
    }
    Ok(())
}

fn add_url(
    files: &mut BTreeMap<PathBuf, DownloadFile>,
    path: PathBuf,
    url: &str,
    sha1: Option<String>,
    size: Option<u64>,
) -> Result<()> {
    let path = safe_relative_path(&path.to_string_lossy())?;
    let url = secure_url(url)?;
    validate_file_metadata(sha1.as_deref(), size)?;
    insert_file(
        files,
        DownloadFile {
            path,
            source: DownloadSource::Url(url),
            sha1,
            size,
        },
    )
}

fn add_payload(
    files: &mut BTreeMap<PathBuf, DownloadFile>,
    path: PathBuf,
    bytes: Arc<Vec<u8>>,
    sha1: Option<String>,
) -> Result<()> {
    let path = safe_relative_path(&path.to_string_lossy())?;
    validate_file_metadata(sha1.as_deref(), Some(bytes.len() as u64))?;
    insert_file(
        files,
        DownloadFile {
            path,
            source: DownloadSource::Bytes(bytes.clone()),
            sha1,
            size: Some(bytes.len() as u64),
        },
    )
}

fn insert_file(files: &mut BTreeMap<PathBuf, DownloadFile>, file: DownloadFile) -> Result<()> {
    if let Some(existing) = files.get(&file.path) {
        if existing.sha1 != file.sha1 || existing.size != file.size {
            return Err(LauncherError::Unsupported(format!(
                "version metadata defines conflicting files at {}",
                file.path.display()
            )));
        }
        return Ok(());
    }
    files.insert(file.path.clone(), file);
    Ok(())
}

fn validate_file_metadata(sha1: Option<&str>, size: Option<u64>) -> Result<()> {
    if sha1.is_some_and(|hash| !is_sha1(hash)) {
        return Err(LauncherError::Unsupported("download metadata contains an invalid SHA-1".to_owned()));
    }
    if size.is_some_and(|size| size > MAX_FILE_BYTES) {
        return Err(LauncherError::Unsupported("download file exceeds the 2 GiB file-size limit".to_owned()));
    }
    if sha1.is_none() && size.is_none() {
        return Err(LauncherError::Unsupported(
            "download metadata has neither a checksum nor an expected size".to_owned(),
        ));
    }
    Ok(())
}

fn optional_sha1(value: Option<&Value>) -> Result<Option<String>> {
    let Some(value) = value.and_then(Value::as_str) else { return Ok(None) };
    if !is_sha1(value) {
        return Err(LauncherError::Unsupported("download metadata contains an invalid SHA-1".to_owned()));
    }
    Ok(Some(value.to_ascii_lowercase()))
}

fn optional_size(value: Option<&Value>) -> Result<Option<u64>> {
    match value {
        None => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            LauncherError::Unsupported("download metadata contains an invalid file size".to_owned())
        }),
    }
}

fn verify_sha1(bytes: &[u8], expected: Option<&str>, label: &str) -> Result<()> {
    if let Some(expected) = expected {
        if !is_sha1(expected) || sha1_hex(bytes) != expected.to_ascii_lowercase() {
            return Err(LauncherError::Unsupported(format!("{label} failed its SHA-1 check")));
        }
    }
    Ok(())
}

fn sha1_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha1::digest(bytes))
}

fn is_sha1(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || value.contains(':')
        || value.contains('\0')
        || path.is_absolute()
        || path.components().any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(LauncherError::Unsupported(format!("unsafe download path {value}")));
    }
    Ok(path.to_path_buf())
}

fn repository_url(base: &str, relative: &Path) -> Result<String> {
    let base = secure_url(base)?;
    let relative = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    secure_url(&format!("{}/{}", base.trim_end_matches('/'), relative))
}

fn download_files(
    client: &Client,
    files: Vec<DownloadFile>,
    root: &Path,
    cancelled: &AtomicBool,
    progress: Arc<ProgressReporter>,
) -> Result<()> {
    let queue = Arc::new(Mutex::new(VecDeque::from(files)));
    let failed = Arc::new(AtomicBool::new(false));
    let failure_message = Arc::new(Mutex::new(None::<String>));
    let worker_count = WORKER_COUNT.min(queue.lock().map(|items| items.len()).unwrap_or(0));
    thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let failed = Arc::clone(&failed);
            let failure_message = Arc::clone(&failure_message);
            let client = client.clone();
            let progress = Arc::clone(&progress);
            scope.spawn(move || loop {
                if cancelled.load(Ordering::Relaxed) || failed.load(Ordering::Relaxed) {
                    break;
                }
                let item = queue.lock().ok().and_then(|mut queue| queue.pop_front());
                let Some(item) = item else { break };
                progress.set_current_file(&item.path);
                match download_one(&client, &item, root, cancelled, &failed, &progress) {
                    Ok(()) => progress.complete_file(),
                    Err(error) => {
                        if let Ok(mut message) = failure_message.lock() {
                            if message.is_none() {
                                *message = Some(error.to_string());
                            }
                        }
                        failed.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            });
        }
    });

    if cancelled.load(Ordering::Relaxed) {
        return Err(LauncherError::Unsupported("installation cancelled".to_owned()));
    }
    if let Some(message) = failure_message.lock().ok().and_then(|message| message.clone()) {
        return Err(LauncherError::Unsupported(message));
    }
    if progress.state.completed_files.load(Ordering::Relaxed) != progress.total_files {
        return Err(LauncherError::Unsupported("installation stopped before all files were verified".to_owned()));
    }
    Ok(())
}

fn download_one(
    client: &Client,
    item: &DownloadFile,
    root: &Path,
    cancelled: &AtomicBool,
    failed: &AtomicBool,
    progress: &ProgressReporter,
) -> Result<()> {
    let target = safe_target(root, &item.path)?;
    if target.exists() {
        if verify_existing(&target, item)? {
            if let Some(size) = item.size {
                progress.add_bytes(size);
            }
            return Ok(());
        }
        return Err(LauncherError::Unsupported(format!(
            "existing file {} failed integrity checks",
            item.path.display()
        )));
    }

    let mut last_error = String::new();
    for attempt in 0..3 {
        if cancelled.load(Ordering::Relaxed) || failed.load(Ordering::Relaxed) {
            return Err(LauncherError::Unsupported("installation cancelled".to_owned()));
        }
        match download_attempt(client, item, &target, cancelled, failed, progress) {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = error.message;
                progress.undo_bytes(error.transferred);
                if attempt < 2 && !cancelled.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(250 * (attempt + 1) as u64));
                }
            }
        }
    }
    Err(LauncherError::Unsupported(format!(
        "could not download {} after three attempts: {last_error}",
        item.path.display()
    )))
}

fn download_attempt(
    client: &Client,
    item: &DownloadFile,
    target: &Path,
    cancelled: &AtomicBool,
    failed: &AtomicBool,
    progress: &ProgressReporter,
) -> std::result::Result<(), AttemptFailure> {
    let mut temp = temporary_file(target).map_err(|error| AttemptFailure {
        message: error.to_string(),
        transferred: 0,
    })?;
    let mut reader: Box<dyn Read> = match &item.source {
        DownloadSource::Url(url) => {
            let url = secure_url(url).map_err(|error| AttemptFailure { message: error.to_string(), transferred: 0 })?;
            let response = client
                .get(url)
                .send()
                .and_then(Response::error_for_status)
                .map_err(|error| AttemptFailure { message: error.to_string(), transferred: 0 })?;
            Box::new(response)
        }
        DownloadSource::Bytes(bytes) => Box::new(Cursor::new(bytes.as_ref().clone())),
    };
    let mut hasher = Sha1::new();
    let mut transferred = 0_u64;
    let limit = item.size.unwrap_or(MAX_FILE_BYTES).min(MAX_FILE_BYTES);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancelled.load(Ordering::Relaxed) || failed.load(Ordering::Relaxed) {
            return Err(AttemptFailure { message: "installation cancelled".to_owned(), transferred });
        }
        let read = reader.read(&mut buffer).map_err(|error| AttemptFailure {
            message: error.to_string(),
            transferred,
        })?;
        if read == 0 {
            break;
        }
        let read = read as u64;
        if transferred.saturating_add(read) > limit {
            return Err(AttemptFailure {
                message: "download exceeded its declared size limit".to_owned(),
                transferred,
            });
        }
        temp.file.as_mut().expect("temporary file remains open")
            .write_all(&buffer[..read as usize])
            .map_err(|error| AttemptFailure { message: error.to_string(), transferred })?;
        hasher.update(&buffer[..read as usize]);
        transferred += read;
        progress.add_bytes(read);
    }
    if item.size.is_some_and(|size| size != transferred) {
        return Err(AttemptFailure {
            message: format!("downloaded size mismatch (expected {}, received {transferred})", item.size.unwrap_or_default()),
            transferred,
        });
    }
    let digest = format!("{:x}", hasher.finalize());
    if item.sha1.as_deref().is_some_and(|expected| expected != digest) {
        return Err(AttemptFailure { message: "downloaded file failed its SHA-1 check".to_owned(), transferred });
    }
    temp.file.as_mut().expect("temporary file remains open")
        .sync_all()
        .map_err(|error| AttemptFailure { message: error.to_string(), transferred })?;
    temp.file.take();
    fs::hard_link(&temp.path, target).map_err(|error| AttemptFailure {
        message: error.to_string(),
        transferred,
    })?;
    Ok(())
}

fn temporary_file(target: &Path) -> std::io::Result<TemporaryFile> {
    let parent = target.parent().ok_or_else(|| std::io::Error::other("download target has no parent"))?;
    for _ in 0..4 {
        let name = format!(".opn-{}.part", Uuid::new_v4());
        let path = parent.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok(TemporaryFile { path, file: Some(file) }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::other("could not allocate a temporary download file"))
}

fn verify_existing(path: &Path, item: &DownloadFile) -> Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(LauncherError::Unsupported("download target is not a regular file".to_owned()));
    }
    if item.size.is_some_and(|size| size != metadata.len()) {
        return Ok(false);
    }
    if let Some(expected) = item.sha1.as_deref() {
        let mut file = File::open(path)?;
        let mut hasher = Sha1::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 { break; }
            hasher.update(&buffer[..read]);
        }
        return Ok(format!("{:x}", hasher.finalize()) == expected);
    }
    Ok(item.size.is_some())
}

fn safe_target(root: &Path, relative: &Path) -> Result<PathBuf> {
    let relative = safe_relative_path(&relative.to_string_lossy())?;
    let root_metadata = fs::symlink_metadata(root)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(LauncherError::Unsupported("instance directory is not a safe directory".to_owned()));
    }
    let mut parent = root.to_path_buf();
    if let Some(relative_parent) = relative.parent() {
        for component in relative_parent.components() {
            let Component::Normal(part) = component else {
                return Err(LauncherError::Unsupported("download path contains an unsafe component".to_owned()));
            };
            parent.push(part);
            match fs::symlink_metadata(&parent) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(LauncherError::Unsupported("download path crosses an unsafe directory".to_owned()));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    match fs::create_dir(&parent) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            let metadata = fs::symlink_metadata(&parent)?;
                            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                                return Err(LauncherError::Unsupported(
                                    "download path crosses an unsafe directory".to_owned(),
                                ));
                            }
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(root.join(relative))
}

fn validate_empty_root(root: &Path) -> Result<()> {
    validate_install_root(root)?;
    validate_root_contents(fs::read_dir(root)?.next().is_none(), true)
}

fn validate_install_root(root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LauncherError::Unsupported("installation directory is not a safe directory".to_owned()));
    }
    Ok(())
}

fn validate_root_contents(is_empty: bool, require_empty: bool) -> Result<()> {
    if require_empty && !is_empty {
        return Err(LauncherError::Unsupported("installation directory must be empty".to_owned()));
    }
    Ok(())
}

fn create_virtual_assets(root: &Path, assets_id: &str, entries: &[(PathBuf, PathBuf)]) -> Result<()> {
    if !is_safe_id(assets_id) {
        return Err(LauncherError::Unsupported("legacy assets identifier is invalid".to_owned()));
    }
    for (logical, object) in entries {
        let destination = PathBuf::from("assets/virtual").join(assets_id).join(logical);
        let destination = safe_target(root, &destination)?;
        let source = safe_target(root, object)?;
        let source_metadata = fs::symlink_metadata(&source)?;
        if source_metadata.file_type().is_symlink() || !source_metadata.is_file() {
            return Err(LauncherError::Unsupported("legacy asset source is not a regular file".to_owned()));
        }
        match fs::symlink_metadata(&destination) {
            Ok(_) => {
                return Err(LauncherError::Unsupported(
                    "legacy asset destination already exists".to_owned(),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        fs::copy(source, destination)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_validates_official_version_manifest_entries() {
        let bytes = r#"{"versions":[{"id":"1.21.4","type":"release","releaseTime":"2024-12-03T00:00:00+00:00","url":"https://piston-meta.mojang.com/version.json","sha1":"0000000000000000000000000000000000000000"},{"id":"3D Shareware v1.34","type":"snapshot","releaseTime":"2019-04-01T00:00:00+00:00","url":"https://piston-meta.mojang.com/3D%20Shareware%20v1.34.json","sha1":"0000000000000000000000000000000000000000"},{"id":"20w14∞","type":"snapshot","releaseTime":"2020-04-01T00:00:00+00:00","url":"https://piston-meta.mojang.com/20w14%E2%88%9E.json","sha1":"0000000000000000000000000000000000000000"}]}"#;
        let versions = parse_version_manifest(bytes.as_bytes()).unwrap();
        assert_eq!(versions[0].id, "1.21.4");
        assert_eq!(versions[0].kind, "release");
        assert_eq!(versions[1].id, "3D Shareware v1.34");
        assert_eq!(versions[2].id, "20w14∞");
    }

    #[test]
    fn manifest_rejects_unsafe_ids_urls_and_hashes() {
        for bytes in [
            br#"{"versions":[{"id":"../escape","type":"release","releaseTime":"now","url":"https://piston-meta.mojang.com/v.json"}]}"#.as_slice(),
            br#"{"versions":[{"id":"CON","type":"release","releaseTime":"now","url":"https://piston-meta.mojang.com/v.json"}]}"#,
            br#"{"versions":[{"id":"Release","type":"release","releaseTime":"now","url":"https://piston-meta.mojang.com/one.json"},{"id":"release","type":"snapshot","releaseTime":"now","url":"https://piston-meta.mojang.com/two.json"}]}"#,
            br#"{"versions":[{"id":"1.0","type":"release","releaseTime":"now","url":"http://example.com/v.json"}]}"#,
            br#"{"versions":[{"id":"1.0","type":"release","releaseTime":"now","url":"https://example.com/v.json","sha1":"bad"}]}"#,
        ] {
            assert!(parse_version_manifest(bytes).is_err());
        }
    }

    #[test]
    fn only_new_instances_require_an_empty_install_folder() {
        assert!(validate_root_contents(false, false).is_ok());
        assert!(validate_root_contents(false, true).is_err());
        assert!(validate_root_contents(true, true).is_ok());
    }

    #[test]
    fn only_upgrades_http_for_known_mojang_hosts() {
        assert_eq!(secure_url("http://libraries.minecraft.net/").unwrap(), "https://libraries.minecraft.net/");
        assert!(secure_url("http://example.com/file.jar").is_err());
        assert!(secure_url("https://user:pass@example.com/file.jar").is_err());
    }

    #[test]
    fn rejects_archive_style_paths_and_accepts_maven_paths() {
        assert!(safe_relative_path("../../outside.jar").is_err());
        assert!(safe_relative_path("C:\\outside.jar").is_err());
        assert_eq!(launch::maven_path("org.example:demo:1.2.3").unwrap(), PathBuf::from("org/example/demo/1.2.3/demo-1.2.3.jar"));
    }

    #[test]
    fn verifies_upstream_sha1_and_file_metadata() {
        assert_eq!(sha1_hex(b"hello world"), "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
        assert!(validate_file_metadata(Some("2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"), Some(11)).is_ok());
        assert!(validate_file_metadata(None, None).is_err());
        assert!(verify_sha1(b"hello world", Some("0000000000000000000000000000000000000000"), "fixture").is_err());
    }

    #[test]
    fn creates_versioned_client_and_library_download_paths() {
        let mut files = BTreeMap::new();
        let checksum = "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed";
        let os = if cfg!(target_os = "macos") { "osx" } else { std::env::consts::OS };
        let classifier = format!("natives-{os}");
        let native_filename = format!("lwjgl-3.3.3-{classifier}.jar");
        let mut metadata = serde_json::json!({
            "downloads": {
                "client": {
                    "url": "https://piston-data.mojang.com/client.jar",
                    "sha1": checksum,
                    "size": 11
                }
            },
            "libraries": [{
                "name": "org.lwjgl:lwjgl:3.3.3",
                "downloads": {
                    "artifact": {
                        "path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar",
                        "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar",
                        "sha1": checksum,
                        "size": 11
                    },
                    "classifiers": {}
                },
                "natives": {}
            }]
        });
        metadata["libraries"][0]["downloads"]["classifiers"][&classifier] = serde_json::json!({
            "path": format!("org/lwjgl/lwjgl/3.3.3/{native_filename}"),
            "url": format!("https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.3/{native_filename}"),
            "sha1": checksum,
            "size": 11
        });
        metadata["libraries"][0]["natives"][os] = serde_json::json!(classifier);

        add_client_download(&metadata, "1.21.4", &mut files).unwrap();
        add_library_downloads(&metadata, &mut files).unwrap();

        assert!(files.contains_key(Path::new("versions/1.21.4/1.21.4.jar")));
        assert!(files.contains_key(Path::new("libraries/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar")));
        assert!(files.contains_key(Path::new(&format!(
            "libraries/org/lwjgl/lwjgl/3.3.3/{native_filename}"
        ))));
    }

    #[test]
    fn maps_asset_hashes_to_official_object_paths_and_rejects_traversal() {
        let checksum = "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed";
        let index = serde_json::json!({
            "virtual": true,
            "objects": { "icons/icon.png": { "hash": checksum, "size": 11 } }
        });
        let mut files = BTreeMap::new();
        let mut virtual_assets = Vec::new();
        add_asset_downloads(&index, &mut files, &mut virtual_assets).unwrap();
        assert!(files.contains_key(Path::new(&format!("assets/objects/2a/{checksum}"))));
        assert_eq!(
            virtual_assets,
            vec![(PathBuf::from("icons/icon.png"), PathBuf::from(format!("assets/objects/2a/{checksum}")))]
        );

        let traversal = serde_json::json!({
            "virtual": true,
            "objects": { "../outside": { "hash": checksum, "size": 11 } }
        });
        assert!(add_asset_downloads(&traversal, &mut BTreeMap::new(), &mut Vec::new()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_legacy_asset_symlink_destinations() {
        use std::os::unix::fs::symlink;

        let test_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("asset-symlink-test-{}", Uuid::new_v4()));
        let root = test_root.join("instance");
        let destination = root.join("assets/virtual/legacy/icons/icon.png");
        let external_target = test_root.join("must-not-be-created");
        fs::create_dir_all(root.join("assets/objects/2a")).unwrap();
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(root.join("assets/objects/2a/hash"), b"asset").unwrap();
        symlink(&external_target, &destination).unwrap();

        assert!(create_virtual_assets(
            &root,
            "legacy",
            &[(PathBuf::from("icons/icon.png"), PathBuf::from("assets/objects/2a/hash"))]
        )
        .is_err());
        assert!(!external_target.exists());

        fs::remove_dir_all(test_root).unwrap();
    }

    #[test]
    fn creates_shared_download_directories_safely_under_concurrency() {
        let test_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("parallel-download-path-test-{}", Uuid::new_v4()));
        let root = test_root.join("instance");
        fs::create_dir_all(&root).unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(16));

        thread::scope(|scope| {
            for index in 0..16 {
                let barrier = Arc::clone(&barrier);
                let root = &root;
                scope.spawn(move || {
                    barrier.wait();
                    let relative = PathBuf::from(format!("libraries/shared/{index}.jar"));
                    safe_target(root, &relative).unwrap();
                });
            }
        });

        assert!(root.join("libraries/shared").is_dir());
        fs::remove_dir_all(test_root).unwrap();
    }
}
