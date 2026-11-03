use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use uuid::Uuid;
use zip::ZipArchive;

use crate::database::Database;
use crate::error::{LauncherError, Result};
use crate::minecraft::is_safe_id;
use crate::models::Instance;

const MODRINTH_API: &str = "https://api.modrinth.com/v2";
const CURSEFORGE_API: &str = "https://api.curseforge.com/v1";
const MODRINTH_IMAGE_HOST: &str = "cdn.modrinth.com";
const MAX_API_RESPONSE: u64 = 16 * 1024 * 1024;
const MAX_MOD_FILE: u64 = 768 * 1024 * 1024;
const MAX_INSTALL_SIZE: u64 = 2 * 1024 * 1024 * 1024;
const MAX_DEPENDENCIES: usize = 32;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModProject {
    pub provider: String,
    pub id: String,
    pub title: String,
    pub author: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub project_url: Option<String>,
    pub gallery: Vec<ModGalleryImage>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModGalleryImage {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModSearchResult {
    pub projects: Vec<ModProject>,
    pub total_hits: u64,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModDependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub file_name: Option<String>,
    pub dependency_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModVersionFile {
    pub name: String,
    pub size: Option<u64>,
    pub sha1: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModVersion {
    pub provider: String,
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub release_type: String,
    pub date_published: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ModVersionFile>,
    pub dependencies: Vec<ModDependency>,
    pub changelog: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModInstanceContext {
    pub instance_id: String,
    pub instance_name: String,
    pub game_dir: String,
    pub minecraft_version: Option<String>,
    pub loader: Option<String>,
    pub version_id: Option<String>,
    pub shared_game_directory: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub file_name: String,
    pub name: String,
    pub version: Option<String>,
    pub mod_id: Option<String>,
    pub enabled: bool,
    pub size: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModDownloadProgress {
    pub download_id: String,
    pub project_title: String,
    pub phase: String,
    pub current_file: String,
    pub completed_files: u32,
    pub total_files: u32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug)]
struct InstallFile {
    project_id: String,
    project_title: String,
    file_name: String,
    url: String,
    sha1: Option<String>,
    size: Option<u64>,
}

pub type ModProgressSink = Arc<dyn Fn(ModDownloadProgress) + Send + Sync>;

pub fn search_projects(
    provider: &str,
    query: &str,
    game_version: &str,
    loader: &str,
    sort: &str,
    offset: u32,
    api_key: Option<&str>,
) -> Result<ModSearchResult> {
    validate_context(game_version, loader)?;
    let query = query.trim();
    if query.len() > 160 {
        return Err(LauncherError::InvalidInput("search text is too long".to_owned()));
    }
    let offset = offset.min(10_000);
    match provider {
        "modrinth" => search_modrinth(query, game_version, loader, sort, offset),
        "curseforge" => search_curseforge(query, game_version, loader, sort, offset, api_key),
        _ => Err(LauncherError::InvalidInput("unknown mod provider".to_owned())),
    }
}

pub fn get_project(provider: &str, project_id: &str, api_key: Option<&str>) -> Result<ModProject> {
    match provider {
        "modrinth" => get_modrinth_project(project_id),
        "curseforge" => get_curseforge_project(project_id, api_key),
        _ => Err(LauncherError::InvalidInput("unknown mod provider".to_owned())),
    }
}

pub fn get_project_versions(
    provider: &str,
    project_id: &str,
    game_version: &str,
    loader: &str,
    api_key: Option<&str>,
) -> Result<Vec<ModVersion>> {
    validate_context(game_version, loader)?;
    match provider {
        "modrinth" => get_modrinth_versions(project_id, game_version, loader),
        "curseforge" => get_curseforge_versions(project_id, game_version, loader, api_key),
        _ => Err(LauncherError::InvalidInput("unknown mod provider".to_owned())),
    }
}

pub fn install_project_version(
    database: &Database,
    instance_id: &str,
    provider: &str,
    project_id: &str,
    version_id: &str,
    api_key: Option<&str>,
    download_id: &str,
    cancelled: Arc<AtomicBool>,
    progress: ModProgressSink,
) -> Result<Vec<InstalledMod>> {
    Uuid::parse_str(download_id).map_err(|_| LauncherError::InvalidInput("download identifier is invalid".to_owned()))?;
    validate_project_id(provider, project_id)?;
    if version_id.is_empty() || version_id.len() > 128 || !version_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')) {
        return Err(LauncherError::InvalidInput("mod version identifier is invalid".to_owned()));
    }
    let instance = database.get_instance(instance_id)?;
    let context = inspect_instance(database, instance_id)?;
    let game_version = context.minecraft_version.as_deref().ok_or_else(|| LauncherError::Unsupported("the selected instance has no readable Minecraft version metadata".to_owned()))?;
    let loader = context.loader.as_deref().ok_or_else(|| LauncherError::Unsupported("installing mods requires an existing Fabric, Forge, NeoForge, or Quilt instance".to_owned()))?;
    validate_context(game_version, loader)?;
    let mut files = match provider {
        "modrinth" => collect_modrinth_install_files(project_id, version_id, game_version, loader)?,
        "curseforge" => collect_curseforge_install_files(project_id, version_id, game_version, loader, api_key)?,
        _ => return Err(LauncherError::InvalidInput("unknown mod provider".to_owned())),
    };
    if files.is_empty() || files.len() > MAX_DEPENDENCIES + 1 {
        return Err(LauncherError::Unsupported("the selected mod has an unsupported dependency count".to_owned()));
    }
    files.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    let game_dir = canonical_game_directory(&instance)?;
    let mods_dir = ensure_mods_directory(&game_dir)?;
    stage_and_install(files, &mods_dir, download_id, &cancelled, progress)?;
    list_installed_mods(database, instance_id)
}

pub fn inspect_instance(database: &Database, instance_id: &str) -> Result<ModInstanceContext> {
    let instance = database.get_instance(instance_id)?;
    let game_dir = canonical_game_directory(&instance)?;
    let mut minecraft_version = None;
    let mut loader = None;
    if let Some(version_id) = instance.version_id.as_deref() {
        if is_safe_id(version_id) {
            let metadata_path = game_dir
                .join("versions")
                .join(version_id)
                .join(format!("{version_id}.json"));
            if let Ok(real_path) = fs::canonicalize(&metadata_path) {
                if real_path.starts_with(&game_dir) {
                    if let Ok(bytes) = fs::read(&real_path) {
                        if let Ok(metadata) = serde_json::from_slice::<Value>(&bytes) {
                            if metadata.get("id").and_then(Value::as_str) == Some(version_id) {
                                minecraft_version = metadata
                                    .get("inheritsFrom")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned)
                                    .or_else(|| Some(version_id.to_owned()));
                                loader = detect_loader(&metadata, version_id);
                            }
                        }
                    }
                }
            }
        }
    }
    let shared_game_directory = database
        .list_instances()?
        .into_iter()
        .filter(|other| other.id != instance.id)
        .filter_map(|other| fs::canonicalize(other.game_dir).ok())
        .any(|other_dir| other_dir == game_dir);
    Ok(ModInstanceContext {
        instance_id: instance.id,
        instance_name: instance.name,
        game_dir: game_dir.to_string_lossy().into_owned(),
        minecraft_version,
        loader,
        version_id: instance.version_id,
        shared_game_directory,
    })
}

pub fn list_installed_mods(database: &Database, instance_id: &str) -> Result<Vec<InstalledMod>> {
    let instance = database.get_instance(instance_id)?;
    let game_dir = canonical_game_directory(&instance)?;
    let mods_dir = game_dir.join("mods");
    let Ok(metadata) = fs::symlink_metadata(&mods_dir) else {
        return Ok(Vec::new());
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LauncherError::Unsupported("the instance mods path is not a regular directory".to_owned()));
    }
    let mut installed = Vec::new();
    for entry in fs::read_dir(&mods_dir)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        if !entry_type.is_file() || entry_type.is_symlink() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let enabled = file_name.to_ascii_lowercase().ends_with(".jar");
        let lower_name = file_name.to_ascii_lowercase();
        if !enabled && !lower_name.ends_with(".jar.disabled") {
            continue;
        }
        let file_metadata = entry.metadata()?;
        let (name, version, mod_id) = read_jar_identity(&entry.path(), &file_name);
        installed.push(InstalledMod {
            file_name,
            name,
            version,
            mod_id,
            enabled,
            size: file_metadata.len(),
        });
        if installed.len() > 512 {
            return Err(LauncherError::Unsupported("this mods folder contains more than 512 mod files".to_owned()));
        }
    }
    installed.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(installed)
}

pub fn set_mod_enabled(database: &Database, instance_id: &str, file_name: &str, enabled: bool) -> Result<()> {
    validate_mod_file_name(file_name)?;
    let instance = database.get_instance(instance_id)?;
    let game_dir = canonical_game_directory(&instance)?;
    let mods_dir = ensure_mods_directory(&game_dir)?;
    let source = mods_dir.join(file_name);
    ensure_regular_file(&source)?;
    let lower_name = file_name.to_ascii_lowercase();
    let destination_name = if enabled {
        if !lower_name.ends_with(".jar.disabled") {
            return Err(LauncherError::InvalidInput("this mod file is already enabled".to_owned()));
        }
        &file_name[..file_name.len() - ".disabled".len()]
    } else {
        if !lower_name.ends_with(".jar") {
            return Err(LauncherError::InvalidInput("only enabled .jar files can be disabled".to_owned()));
        }
        return move_mod_file(&source, &mods_dir.join(format!("{file_name}.disabled")));
    };
    move_mod_file(&source, &mods_dir.join(destination_name))
}

pub fn remove_mod(database: &Database, instance_id: &str, file_name: &str, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(LauncherError::InvalidInput("confirm removal before deleting a mod file".to_owned()));
    }
    validate_mod_file_name(file_name)?;
    let instance = database.get_instance(instance_id)?;
    let game_dir = canonical_game_directory(&instance)?;
    let mods_dir = ensure_mods_directory(&game_dir)?;
    let path = mods_dir.join(file_name);
    ensure_regular_file(&path)?;
    fs::remove_file(path)?;
    Ok(())
}

fn search_modrinth(query: &str, game_version: &str, loader: &str, sort: &str, offset: u32) -> Result<ModSearchResult> {
    let client = api_client(false)?;
    let mut facets = vec![vec!["project_type:mod".to_owned()], vec![format!("versions:{game_version}")], vec![format!("categories:{loader}")]];
    if loader == "neoforge" {
        facets[2] = vec!["categories:neoforge".to_owned()];
    }
    let facets = serde_json::to_string(&facets)?;
    let index = match sort {
        "downloads" => "downloads",
        "updated" => "updated",
        "newest" => "newest",
        _ => "relevance",
    };
    let response = get_with_query(
        &client,
        &format!("{MODRINTH_API}/search"),
        &[
            ("query", query.to_owned()),
            ("facets", facets),
            ("index", index.to_owned()),
            ("limit", "24".to_owned()),
            ("offset", offset.to_string()),
        ],
    )?
        .send()
        .map_err(network_error)?;
    let body = read_json(response)?;
    let hits = body.get("hits").and_then(Value::as_array).ok_or_else(|| LauncherError::Unsupported("Modrinth returned an unexpected search response".to_owned()))?;
    let mut projects = Vec::with_capacity(hits.len());
    for hit in hits {
        if let Some(project) = parse_modrinth_project(hit, "mod") {
            projects.push(project);
        }
    }
    Ok(ModSearchResult {
        projects,
        total_hits: body.get("total_hits").and_then(Value::as_u64).unwrap_or(0),
        offset,
        limit: 24,
    })
}

fn get_modrinth_project(project_id: &str) -> Result<ModProject> {
    validate_project_id("modrinth", project_id)?;
    let client = api_client(false)?;
    let body = read_json(client.get(format!("{MODRINTH_API}/project/{project_id}")).send().map_err(network_error)?)?;
    parse_modrinth_project(&body, "mod").ok_or_else(|| LauncherError::Unsupported("Modrinth returned an unexpected project response".to_owned()))
}

fn get_modrinth_versions(project_id: &str, game_version: &str, loader: &str) -> Result<Vec<ModVersion>> {
    validate_project_id("modrinth", project_id)?;
    let client = api_client(false)?;
    let loaders = serde_json::to_string(&[loader])?;
    let game_versions = serde_json::to_string(&[game_version])?;
    let response = get_with_query(
        &client,
        &format!("{MODRINTH_API}/project/{project_id}/version"),
        &[
            ("loaders", loaders),
            ("game_versions", game_versions),
            ("include_changelog", "true".to_owned()),
        ],
    )?
        .send()
        .map_err(network_error)?;
    let body = read_json(response)?;
    let items = body.as_array().ok_or_else(|| LauncherError::Unsupported("Modrinth returned an unexpected version response".to_owned()))?;
    let versions = items.iter().filter_map(|item| parse_modrinth_version(item, project_id)).collect();
    Ok(versions)
}

fn collect_modrinth_install_files(project_id: &str, version_id: &str, game_version: &str, loader: &str) -> Result<Vec<InstallFile>> {
    let client = api_client(false)?;
    let root = get_modrinth_version(&client, version_id)?;
    if root.get("project_id").and_then(Value::as_str) != Some(project_id) {
        return Err(LauncherError::InvalidInput("selected version does not belong to the selected project".to_owned()));
    }
    let root_title = get_modrinth_project(project_id)?.title;
    let mut files = Vec::new();
    let mut visited = BTreeSet::new();
    collect_modrinth_version(&client, root, Some((project_id, root_title.as_str())), game_version, loader, &mut visited, &mut files, 0)?;
    Ok(files)
}

fn collect_modrinth_version(
    client: &Client,
    version: Value,
    expected_project: Option<(&str, &str)>,
    game_version: &str,
    loader: &str,
    visited: &mut BTreeSet<String>,
    files: &mut Vec<InstallFile>,
    depth: usize,
) -> Result<()> {
    if depth > 8 || files.len() > MAX_DEPENDENCIES {
        return Err(LauncherError::Unsupported("the required mod dependency chain is too large".to_owned()));
    }
    if version.get("game_versions").and_then(Value::as_array).is_none_or(|versions| !versions.iter().any(|version| version.as_str() == Some(game_version)))
        || version.get("loaders").and_then(Value::as_array).is_none_or(|loaders| !loaders.iter().any(|entry| entry.as_str() == Some(loader)))
    {
        return Err(LauncherError::Unsupported("a selected mod version does not match this instance's Minecraft version and loader".to_owned()));
    }
    let project_id = version.get("project_id").and_then(Value::as_str).ok_or_else(|| LauncherError::Unsupported("a mod version has no project identifier".to_owned()))?.to_owned();
    if let Some((expected_id, _)) = expected_project {
        if project_id != expected_id {
            return Err(LauncherError::InvalidInput("selected version does not belong to the selected project".to_owned()));
        }
    }
    if !visited.insert(project_id.clone()) {
        return Ok(());
    }
    let title = if let Some((_, title)) = expected_project { title.to_owned() } else { get_modrinth_project(&project_id)?.title };
    let files_value = version.get("files").and_then(Value::as_array).ok_or_else(|| LauncherError::Unsupported("the selected mod version has no files".to_owned()))?;
    let primary = files_value.iter().find(|file| file.get("primary").and_then(Value::as_bool) == Some(true)).or_else(|| files_value.first()).ok_or_else(|| LauncherError::Unsupported("the selected mod version has no downloadable file".to_owned()))?;
    files.push(parse_modrinth_install_file(primary, &project_id, &title)?);

    let dependencies = version.get("dependencies").and_then(Value::as_array).cloned().unwrap_or_default();
    for dependency in dependencies {
        if dependency.get("dependency_type").and_then(Value::as_str) != Some("required") {
            continue;
        }
        if files.len() >= MAX_DEPENDENCIES + 1 {
            return Err(LauncherError::Unsupported("this mod requires too many other projects".to_owned()));
        }
        let declared_project_id = dependency.get("project_id").and_then(Value::as_str);
        let dependency_version = if let Some(version_id) = dependency.get("version_id").and_then(Value::as_str) {
            let version = get_modrinth_version(client, version_id)?;
            if declared_project_id.is_some_and(|expected| version.get("project_id").and_then(Value::as_str) != Some(expected)) {
                return Err(LauncherError::Unsupported("a required dependency version does not match its declared project".to_owned()));
            }
            version
        } else if let Some(dependency_project) = dependency.get("project_id").and_then(Value::as_str) {
            let versions = get_modrinth_project_versions(client, dependency_project, game_version, loader)?;
            versions.iter().find(|entry| entry.get("version_type").and_then(Value::as_str) == Some("release"))
                .or_else(|| versions.first())
                .cloned()
                .ok_or_else(|| LauncherError::Unsupported(format!("a required dependency ({dependency_project}) has no compatible version")))?
        } else {
            return Err(LauncherError::Unsupported("a required dependency has no downloadable project or version ID; install it manually from its author".to_owned()));
        };
        collect_modrinth_version(client, dependency_version, None, game_version, loader, visited, files, depth + 1)?;
    }
    Ok(())
}

fn get_modrinth_version(client: &Client, version_id: &str) -> Result<Value> {
    validate_project_id("Modrinth version", version_id)?;
    read_json(client.get(format!("{MODRINTH_API}/version/{version_id}")).send().map_err(network_error)?)
}

fn get_modrinth_project_versions(client: &Client, project_id: &str, game_version: &str, loader: &str) -> Result<Vec<Value>> {
    validate_project_id("Modrinth", project_id)?;
    let loaders = serde_json::to_string(&[loader])?;
    let game_versions = serde_json::to_string(&[game_version])?;
    let response = get_with_query(
        client,
        &format!("{MODRINTH_API}/project/{project_id}/version"),
        &[
            ("loaders", loaders),
            ("game_versions", game_versions),
            ("include_changelog", "false".to_owned()),
        ],
    )?
        .send().map_err(network_error)?;
    read_json(response)?.as_array().cloned().ok_or_else(|| LauncherError::Unsupported("Modrinth returned an unexpected dependency response".to_owned()))
}

fn parse_modrinth_install_file(file: &Value, project_id: &str, title: &str) -> Result<InstallFile> {
    let file_name = file.get("filename").and_then(Value::as_str).ok_or_else(|| LauncherError::Unsupported("a mod file has no filename".to_owned()))?.to_owned();
    validate_mod_file_name(&file_name)?;
    let url = file.get("url").and_then(Value::as_str).ok_or_else(|| LauncherError::Unsupported("a mod file has no download URL".to_owned()))?.to_owned();
    validate_download_url(&url, "modrinth")?;
    let sha1 = file.pointer("hashes.sha1").and_then(Value::as_str).map(str::to_owned);
    let size = file.get("size").and_then(Value::as_u64);
    if sha1.as_deref().is_none_or(|hash| !is_sha1(hash)) && size.is_none() {
        return Err(LauncherError::Unsupported("a mod file has no usable integrity metadata".to_owned()));
    }
    Ok(InstallFile { project_id: project_id.to_owned(), project_title: title.to_owned(), file_name, url, sha1, size })
}

fn collect_curseforge_install_files(project_id: &str, file_id: &str, game_version: &str, loader: &str, api_key: Option<&str>) -> Result<Vec<InstallFile>> {
    let api_key = validate_curseforge_key(api_key)?;
    let project = get_curseforge_project(project_id, Some(api_key))?;
    let version = get_curseforge_versions(project_id, game_version, loader, Some(api_key))?.into_iter().find(|version| version.id == file_id)
        .ok_or_else(|| LauncherError::Unsupported("the chosen CurseForge file does not match this instance's Minecraft version and loader".to_owned()))?;
    let client = api_client(true)?;
    let file_body = read_json(client.get(format!("{CURSEFORGE_API}/mods/{project_id}/files/{file_id}"))
        .header("x-api-key", api_key).send().map_err(network_error)?)?;
    let file = file_body.get("data").ok_or_else(|| LauncherError::Unsupported("CurseForge returned an unexpected file response".to_owned()))?;
    let url_body = read_json(client.get(format!("{CURSEFORGE_API}/mods/{project_id}/files/{file_id}/download-url"))
        .header("x-api-key", api_key).send().map_err(network_error)?)?;
    let url = url_body.get("data").and_then(Value::as_str).ok_or_else(|| LauncherError::Unsupported("CurseForge did not provide a download link for this file".to_owned()))?.to_owned();
    validate_download_url(&url, "curseforge")?;
    let file_name = file
        .get("fileName")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| version.files.first().map(|file| file.name.clone()))
        .ok_or_else(|| LauncherError::Unsupported("the CurseForge file has no filename".to_owned()))?;
    validate_mod_file_name(&file_name)?;
    let sha1 = file.get("hashes").and_then(Value::as_array).into_iter().flatten().find(|hash| hash.get("algo").and_then(Value::as_u64) == Some(1)).and_then(|hash| hash.get("value")).and_then(Value::as_str).map(str::to_owned);
    let size = file.get("fileLength").and_then(Value::as_u64);
    if sha1.as_deref().is_none_or(|hash| !is_sha1(hash)) && size.is_none() {
        return Err(LauncherError::Unsupported("the CurseForge file has no usable integrity metadata".to_owned()));
    }
    let mut files = vec![InstallFile { project_id: project_id.to_owned(), project_title: project.title, file_name, url, sha1, size }];
    let required = version.dependencies.iter().filter(|dependency| dependency.dependency_type == "required").filter_map(|dependency| dependency.project_id.as_deref()).collect::<Vec<_>>();
    if required.len() > MAX_DEPENDENCIES {
        return Err(LauncherError::Unsupported("this mod requires too many other projects".to_owned()));
    }
    let mut visited = BTreeSet::from([project_id.to_owned()]);
    for dependency_id in required {
        if !visited.insert(dependency_id.to_owned()) { continue; }
        let versions = get_curseforge_versions(dependency_id, game_version, loader, Some(api_key))?;
        let dependency = versions.iter().find(|version| version.release_type == "release")
            .or_else(|| versions.first())
            .cloned()
            .ok_or_else(|| LauncherError::Unsupported(format!("required CurseForge dependency {dependency_id} has no compatible file")))?;
        let mod_project = get_curseforge_project(dependency_id, Some(api_key))?;
        let dep_id = dependency.id.clone();
        let dependency_file_body = read_json(client.get(format!("{CURSEFORGE_API}/mods/{dependency_id}/files/{dep_id}"))
            .header("x-api-key", api_key).send().map_err(network_error)?)?;
        let dependency_file = dependency_file_body.get("data").ok_or_else(|| LauncherError::Unsupported("CurseForge returned an unexpected dependency file response".to_owned()))?;
        let dependency_url_body = read_json(client.get(format!("{CURSEFORGE_API}/mods/{dependency_id}/files/{dep_id}/download-url"))
            .header("x-api-key", api_key).send().map_err(network_error)?)?;
        let dependency_url = dependency_url_body.get("data").and_then(Value::as_str).ok_or_else(|| LauncherError::Unsupported("CurseForge did not provide a dependency download link".to_owned()))?.to_owned();
        validate_download_url(&dependency_url, "curseforge")?;
        let dep_file_name = dependency_file.get("fileName").and_then(Value::as_str).ok_or_else(|| LauncherError::Unsupported("a required dependency file has no filename".to_owned()))?.to_owned();
        validate_mod_file_name(&dep_file_name)?;
        let dep_sha1 = dependency_file.get("hashes").and_then(Value::as_array).into_iter().flatten().find(|hash| hash.get("algo").and_then(Value::as_u64) == Some(1)).and_then(|hash| hash.get("value")).and_then(Value::as_str).map(str::to_owned);
        let dep_size = dependency_file.get("fileLength").and_then(Value::as_u64);
        files.push(InstallFile { project_id: dependency_id.to_owned(), project_title: mod_project.title, file_name: dep_file_name, url: dependency_url, sha1: dep_sha1, size: dep_size });
    }
    Ok(files)
}

fn stage_and_install(files: Vec<InstallFile>, mods_dir: &Path, download_id: &str, cancelled: &Arc<AtomicBool>, progress: ModProgressSink) -> Result<()> {
    let total_files = u32::try_from(files.len()).map_err(|_| LauncherError::Unsupported("too many mod files".to_owned()))?;
    let total_bytes = files.iter().try_fold(0_u64, |sum, file| sum.checked_add(file.size.unwrap_or(0)).filter(|sum| *sum <= MAX_INSTALL_SIZE))
        .ok_or_else(|| LauncherError::Unsupported("required mod files exceed the 2 GiB install limit".to_owned()))?;
    let primary_title = files.first().map(|file| file.project_title.clone()).unwrap_or_else(|| "Mod".to_owned());
    let staging_dir = mods_dir.join(format!(".opnlauncher-staging-{download_id}"));
    fs::create_dir(&staging_dir)?;
    let mut staged = Vec::new();
    let result = (|| {
        let mut downloaded = 0_u64;
        for (index, file) in files.iter().enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                return Err(LauncherError::Unsupported("mod installation cancelled".to_owned()));
            }
            validate_mod_file_name(&file.file_name)?;
            let destination = mods_dir.join(&file.file_name);
            if path_exists_no_follow(&destination)? {
                ensure_regular_file(&destination)?;
                if file.sha1.as_deref().is_some_and(|expected| sha1_file(&destination).is_ok_and(|actual| actual.eq_ignore_ascii_case(expected))) {
                    downloaded = downloaded.saturating_add(file.size.unwrap_or(fs::metadata(&destination)?.len()));
                    progress(ModDownloadProgress { download_id: download_id.to_owned(), project_title: file.project_title.clone(), phase: "Already installed".to_owned(), current_file: file.file_name.clone(), completed_files: index as u32 + 1, total_files, downloaded_bytes: downloaded, total_bytes });
                    continue;
                }
                return Err(LauncherError::Unsupported(format!("{} already exists in this instance's mods folder; remove or disable it before replacing it", file.file_name)));
            }
            let disabled_copy = mods_dir.join(format!("{}.disabled", file.file_name));
            if path_exists_no_follow(&disabled_copy)? {
                ensure_regular_file(&disabled_copy)?;
                return Err(LauncherError::Unsupported(format!("{} is already present but disabled; re-enable or remove it before installing another copy", file.file_name)));
            }
            let temp_file = staging_dir.join(&file.file_name);
            progress(ModDownloadProgress { download_id: download_id.to_owned(), project_title: file.project_title.clone(), phase: "Downloading mod files".to_owned(), current_file: file.file_name.clone(), completed_files: index as u32, total_files, downloaded_bytes: downloaded, total_bytes });
            let actual_size = download_and_verify(file, &temp_file, cancelled, download_id, total_files, index as u32, downloaded, total_bytes, &progress)?;
            downloaded = downloaded.saturating_add(actual_size);
            staged.push((temp_file, destination, file.file_name.clone()));
        }
        let mut installed: Vec<PathBuf> = Vec::new();
        for (source, destination, file_name) in &staged {
            if cancelled.load(Ordering::Relaxed) {
                for moved in installed.iter().rev() { let _ = fs::remove_file(moved); }
                return Err(LauncherError::Unsupported("mod installation cancelled".to_owned()));
            }
            if let Err(error) = fs::hard_link(source, destination) {
                for moved in installed.iter().rev() { let _ = fs::remove_file(moved); }
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    return Err(LauncherError::Unsupported(format!("{} appeared during installation; no files were replaced", file_name)));
                }
                return Err(error.into());
            }
            installed.push(destination.clone());
            if let Err(error) = fs::remove_file(source) {
                for moved in installed.iter().rev() { let _ = fs::remove_file(moved); }
                return Err(error.into());
            }
        }
        progress(ModDownloadProgress { download_id: download_id.to_owned(), project_title: primary_title, phase: "Installation complete".to_owned(), current_file: String::new(), completed_files: total_files, total_files, downloaded_bytes: downloaded, total_bytes });
        Ok(())
    })();
    let _ = fs::remove_dir_all(&staging_dir);
    result
}

fn download_and_verify(file: &InstallFile, destination: &Path, cancelled: &AtomicBool, download_id: &str, total_files: u32, index: u32, base_bytes: u64, total_bytes: u64, progress: &ModProgressSink) -> Result<u64> {
    let client = download_client()?;
    let response = client.get(&file.url).send().map_err(network_error)?;
    if !response.status().is_success() {
        return Err(LauncherError::Unsupported(format!("the provider could not download {} (HTTP {})", file.file_name, response.status().as_u16())));
    }
    if response.content_length().is_some_and(|length| length > MAX_MOD_FILE || base_bytes.saturating_add(length) > MAX_INSTALL_SIZE) {
        return Err(LauncherError::Unsupported("mod files exceed the per-file or 2 GiB installation size limit".to_owned()));
    }
    let mut response = response;
    let mut output = OpenOptions::new().write(true).create_new(true).open(destination)?;
    let mut digest = Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(LauncherError::Unsupported("mod installation cancelled".to_owned()));
        }
        let read = response.read(&mut buffer).map_err(|_| LauncherError::Unsupported("the provider download ended unexpectedly".to_owned()))?;
        if read == 0 { break; }
        size = size
            .checked_add(read as u64)
            .filter(|size| *size <= MAX_MOD_FILE && base_bytes.saturating_add(*size) <= MAX_INSTALL_SIZE)
            .ok_or_else(|| LauncherError::Unsupported("mod files exceed the per-file or 2 GiB installation size limit".to_owned()))?;
        output.write_all(&buffer[..read])?;
        digest.update(&buffer[..read]);
        progress(ModDownloadProgress { download_id: download_id.to_owned(), project_title: file.project_title.clone(), phase: "Downloading mod files".to_owned(), current_file: file.file_name.clone(), completed_files: index, total_files, downloaded_bytes: base_bytes.saturating_add(size), total_bytes });
    }
    output.sync_all()?;
    if file.size.is_some_and(|expected| expected != size) {
        return Err(LauncherError::Unsupported(format!("{} did not match the provider's declared file size", file.file_name)));
    }
    let actual = format!("{:x}", digest.finalize());
    if file.sha1.as_deref().is_some_and(|expected| !actual.eq_ignore_ascii_case(expected)) {
        return Err(LauncherError::Unsupported(format!("{} failed its SHA-1 integrity check", file.file_name)));
    }
    if file.size.is_none() && file.sha1.is_none() {
        return Err(LauncherError::Unsupported(format!("{} has no integrity data", file.file_name)));
    }
    Ok(size)
}

fn validate_download_url(url: &str, provider: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|_| LauncherError::Unsupported("the provider supplied an invalid download URL".to_owned()))?;
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let allowed = parsed.scheme() == "https" && parsed.username().is_empty() && parsed.password().is_none() && match provider {
        "modrinth" => host == MODRINTH_IMAGE_HOST,
        "curseforge" => host == "edge.forgecdn.net" || host.ends_with(".forgecdn.net"),
        _ => false,
    };
    if !allowed {
        return Err(LauncherError::Unsupported("the provider supplied a download URL outside its trusted HTTPS file hosts".to_owned()));
    }
    Ok(())
}

fn is_sha1(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha1_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 { break; }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn search_curseforge(query: &str, game_version: &str, loader: &str, sort: &str, offset: u32, api_key: Option<&str>) -> Result<ModSearchResult> {
    let api_key = validate_curseforge_key(api_key)?;
    let client = api_client(true)?;
    let mut query_values = vec![
        ("gameId", "432".to_owned()),
        ("classId", "6".to_owned()),
        ("pageSize", "24".to_owned()),
        ("index", offset.to_string()),
        ("sortOrder", "desc".to_owned()),
    ];
    if !query.is_empty() {
        query_values.push(("searchFilter", query.to_owned()));
    }
    if !game_version.is_empty() {
        query_values.push(("gameVersion", game_version.to_owned()));
    }
    if let Some(loader_type) = curseforge_loader_id(loader) {
        query_values.push(("modLoaderType", loader_type.to_string()));
    }
    let sort_field = match sort {
        "downloads" => "6",
        "updated" | "newest" => "3",
        _ => "2",
    };
    query_values.push(("sortField", sort_field.to_owned()));
    let request = get_with_query(&client, &format!("{CURSEFORGE_API}/mods/search"), &query_values)?
        .header("x-api-key", api_key);
    let body = read_json(request.send().map_err(network_error)?)?;
    let results = body.get("data").and_then(Value::as_array).ok_or_else(|| LauncherError::Unsupported("CurseForge returned an unexpected search response".to_owned()))?;
    let projects = results.iter().filter_map(parse_curseforge_project).collect();
    let total_hits = body.pointer("pagination.totalCount").and_then(Value::as_u64).unwrap_or(0);
    Ok(ModSearchResult { projects, total_hits, offset, limit: 24 })
}

fn get_curseforge_project(project_id: &str, api_key: Option<&str>) -> Result<ModProject> {
    let api_key = validate_curseforge_key(api_key)?;
    let id = validate_curseforge_id(project_id)?;
    let client = api_client(true)?;
    let body = read_json(client
        .get(format!("{CURSEFORGE_API}/mods/{id}"))
        .header("x-api-key", api_key)
        .send()
        .map_err(network_error)?)?;
    parse_curseforge_project(body.get("data").unwrap_or(&Value::Null)).ok_or_else(|| LauncherError::Unsupported("CurseForge returned an unexpected project response".to_owned()))
}

fn get_curseforge_versions(project_id: &str, game_version: &str, loader: &str, api_key: Option<&str>) -> Result<Vec<ModVersion>> {
    let api_key = validate_curseforge_key(api_key)?;
    let id = validate_curseforge_id(project_id)?;
    let client = api_client(true)?;
    let mut query_values = vec![("pageSize", "50".to_owned())];
    if !game_version.is_empty() {
        query_values.push(("gameVersion", game_version.to_owned()));
    }
    if let Some(loader_type) = curseforge_loader_id(loader) {
        query_values.push(("modLoaderType", loader_type.to_string()));
    }
    let request = get_with_query(&client, &format!("{CURSEFORGE_API}/mods/{id}/files"), &query_values)?
        .header("x-api-key", api_key);
    let body = read_json(request.send().map_err(network_error)?)?;
    let files = body.get("data").and_then(Value::as_array).ok_or_else(|| LauncherError::Unsupported("CurseForge returned an unexpected version response".to_owned()))?;
    let versions = files.iter().filter_map(|file| parse_curseforge_version(file, project_id, game_version, loader)).collect();
    Ok(versions)
}

fn parse_modrinth_project(value: &Value, default_type: &str) -> Option<ModProject> {
    let id = value.get("project_id").or_else(|| value.get("id"))?.as_str()?.to_owned();
    let title = value.get("title")?.as_str()?.to_owned();
    let project_type = value.get("project_type").and_then(Value::as_str).unwrap_or(default_type);
    let slug = value.get("slug").and_then(Value::as_str).unwrap_or(&id).to_owned();
    let mut gallery = value.get("gallery").and_then(Value::as_array).into_iter().flatten().filter_map(|item| {
        let url = item.as_str().or_else(|| item.get("url")?.as_str())?;
        safe_image_url(url).then(|| ModGalleryImage { url: url.to_owned(), title: item.get("title").and_then(Value::as_str).map(str::to_owned), description: item.get("description").and_then(Value::as_str).map(str::to_owned) })
    }).collect::<Vec<_>>();
    if let Some(featured_url) = value.get("featured_gallery").and_then(Value::as_str).filter(|url| safe_image_url(url)) {
        if let Some(index) = gallery.iter().position(|image| image.url == featured_url) {
            let featured = gallery.remove(index);
            gallery.insert(0, featured);
        } else {
            gallery.insert(0, ModGalleryImage { url: featured_url.to_owned(), title: None, description: None });
        }
    }
    Some(ModProject {
        provider: "modrinth".to_owned(),
        id,
        title,
        author: value.get("author").or_else(|| value.get("team"))?.as_str()?.to_owned(),
        summary: value.get("description")?.as_str()?.to_owned(),
        icon_url: value.get("icon_url").and_then(Value::as_str).filter(|url| safe_image_url(url)).map(str::to_owned),
        downloads: value.get("downloads").and_then(Value::as_u64).unwrap_or(0),
        categories: string_list(value.get("categories")),
        game_versions: string_list(value.get("versions").or_else(|| value.get("game_versions"))),
        loaders: string_list(value.get("loaders")),
        project_url: Some(format!("https://modrinth.com/{project_type}/{slug}")),
        gallery,
    })
}

fn parse_modrinth_version(value: &Value, default_project_id: &str) -> Option<ModVersion> {
    let id = value.get("id")?.as_str()?.to_owned();
    let project_id = value.get("project_id").and_then(Value::as_str).unwrap_or(default_project_id).to_owned();
    let files = value.get("files").and_then(Value::as_array).into_iter().flatten().filter_map(|file| {
        let name = file.get("filename")?.as_str()?.to_owned();
        let hashes = file.get("hashes");
        Some(ModVersionFile {
            name,
            size: file.get("size").and_then(Value::as_u64),
            sha1: hashes.and_then(|hashes| hashes.get("sha1")).and_then(Value::as_str).map(str::to_owned),
        })
    }).collect();
    let dependencies = value.get("dependencies").and_then(Value::as_array).into_iter().flatten().filter_map(|dependency| Some(ModDependency {
        project_id: dependency.get("project_id").and_then(Value::as_str).map(str::to_owned),
        version_id: dependency.get("version_id").and_then(Value::as_str).map(str::to_owned),
        file_name: dependency.get("file_name").and_then(Value::as_str).map(str::to_owned),
        dependency_type: dependency.get("dependency_type").and_then(Value::as_str).unwrap_or("unknown").to_owned(),
    })).collect();
    Some(ModVersion {
        provider: "modrinth".to_owned(),
        id,
        project_id,
        name: value.get("name").and_then(Value::as_str).unwrap_or("Mod version").to_owned(),
        version_number: value.get("version_number").and_then(Value::as_str).unwrap_or("unknown").to_owned(),
        release_type: value.get("version_type").and_then(Value::as_str).unwrap_or("release").to_owned(),
        date_published: value.get("date_published").and_then(Value::as_str).unwrap_or("").to_owned(),
        game_versions: string_list(value.get("game_versions")),
        loaders: string_list(value.get("loaders")),
        files,
        dependencies,
        changelog: value.get("changelog").and_then(Value::as_str).map(str::to_owned),
    })
}

fn parse_curseforge_project(value: &Value) -> Option<ModProject> {
    let id = value.get("id")?.as_u64()?.to_string();
    let logo_url = value.pointer("logo.url").and_then(Value::as_str).filter(|url| safe_image_url(url)).map(str::to_owned);
    let gallery = value.get("screenshots").and_then(Value::as_array).into_iter().flatten().filter_map(|image| {
        let url = image.get("url")?.as_str()?;
        safe_image_url(url).then(|| ModGalleryImage { url: url.to_owned(), title: image.get("title").and_then(Value::as_str).map(str::to_owned), description: image.get("description").and_then(Value::as_str).map(str::to_owned) })
    }).collect();
    Some(ModProject {
        provider: "curseforge".to_owned(),
        id,
        title: value.get("name")?.as_str()?.to_owned(),
        author: value.pointer("authors.0.name").and_then(Value::as_str).unwrap_or("CurseForge creator").to_owned(),
        summary: value.get("summary").and_then(Value::as_str).unwrap_or("").to_owned(),
        icon_url: logo_url,
        downloads: value.get("downloadCount").and_then(Value::as_u64).unwrap_or(0),
        categories: value.get("categories").and_then(Value::as_array).into_iter().flatten().filter_map(|category| category.get("name").and_then(Value::as_str).map(str::to_owned)).collect(),
        game_versions: value.pointer("latestFilesIndexes").and_then(Value::as_array).into_iter().flatten().filter_map(|file| file.get("gameVersion").and_then(Value::as_str).map(str::to_owned)).collect::<BTreeSet<_>>().into_iter().collect(),
        loaders: Vec::new(),
        project_url: value.pointer("links.websiteUrl").and_then(Value::as_str).filter(|url| safe_project_url(url, "curseforge")).map(str::to_owned),
        gallery,
    })
}

fn parse_curseforge_version(value: &Value, project_id: &str, game_version: &str, loader: &str) -> Option<ModVersion> {
    let id = value.get("id")?.as_u64()?.to_string();
    let compatible_versions = string_list(value.get("gameVersions"));
    let compatible = compatible_versions.iter().any(|version| version == game_version);
    if !game_version.is_empty() && !compatible {
        return None;
    }
    let file_name = value.get("fileName").and_then(Value::as_str)?.to_owned();
    let size = value.get("fileLength").and_then(Value::as_u64);
    let sha1 = value.get("hashes").and_then(Value::as_array).into_iter().flatten().find(|hash| hash.get("algo").and_then(Value::as_u64) == Some(1)).and_then(|hash| hash.get("value")).and_then(Value::as_str).map(str::to_owned);
    let dependencies = value.get("dependencies").and_then(Value::as_array).into_iter().flatten().filter_map(|dependency| {
        let relation = dependency.get("relationType").and_then(Value::as_u64).unwrap_or(0);
        (relation == 3).then(|| ModDependency {
            project_id: dependency.get("modId").and_then(Value::as_u64).map(|id| id.to_string()),
            version_id: None,
            file_name: None,
            dependency_type: "required".to_owned(),
        })
    }).collect();
    Some(ModVersion {
        provider: "curseforge".to_owned(),
        id,
        project_id: project_id.to_owned(),
        name: value.get("displayName").and_then(Value::as_str).unwrap_or(&file_name).to_owned(),
        version_number: value.get("displayName").and_then(Value::as_str).unwrap_or(&file_name).to_owned(),
        release_type: match value.get("releaseType").and_then(Value::as_u64).unwrap_or(1) { 3 => "alpha", 2 => "beta", _ => "release" }.to_owned(),
        date_published: value.get("fileDate").and_then(Value::as_str).unwrap_or("").to_owned(),
        game_versions: compatible_versions,
        loaders: vec![loader.to_owned()],
        files: vec![ModVersionFile { name: file_name, size, sha1 }],
        dependencies,
        changelog: None,
    })
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
}

fn validate_context(game_version: &str, loader: &str) -> Result<()> {
    if !game_version.is_empty() && (!is_safe_id(game_version) || game_version.len() > 64) {
        return Err(LauncherError::InvalidInput("Minecraft version is invalid".to_owned()));
    }
    if !matches!(loader, "fabric" | "forge" | "neoforge" | "quilt") {
        return Err(LauncherError::InvalidInput("supported loaders are Fabric, Forge, NeoForge, and Quilt".to_owned()));
    }
    Ok(())
}

fn validate_project_id(provider: &str, project_id: &str) -> Result<()> {
    if project_id.is_empty() || project_id.len() > 128 || !project_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')) {
        return Err(LauncherError::InvalidInput(format!("{provider} project identifier is invalid")));
    }
    Ok(())
}

fn validate_curseforge_id(project_id: &str) -> Result<u64> {
    project_id.parse::<u64>().ok().filter(|id| *id > 0).ok_or_else(|| LauncherError::InvalidInput("CurseForge project identifier is invalid".to_owned()))
}

fn validate_curseforge_key(api_key: Option<&str>) -> Result<&str> {
    let key = api_key.map(str::trim).filter(|key| !key.is_empty()).ok_or_else(|| LauncherError::Unsupported("CurseForge API access needs an API key from its developer console. Enter it in the Mods page for this session.".to_owned()))?;
    if !(16..=512).contains(&key.len()) || key.chars().any(char::is_control) {
        return Err(LauncherError::InvalidInput("CurseForge API key format is invalid".to_owned()));
    }
    Ok(key)
}

fn curseforge_loader_id(loader: &str) -> Option<u8> {
    match loader { "forge" => Some(1), "fabric" => Some(4), "quilt" => Some(5), "neoforge" => Some(6), _ => None }
}

fn api_client(curseforge: bool) -> Result<Client> {
    let redirect = if curseforge {
        Policy::none()
    } else {
        Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() >= 3 || url.scheme() != "https" || url.host_str() != Some("api.modrinth.com") {
                attempt.stop()
            } else {
                attempt.follow()
            }
        })
    };
    Client::builder()
        .user_agent(concat!("OpnLauncher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(45))
        .redirect(redirect)
        .build()
        .map_err(|_| LauncherError::Unsupported("could not initialize the secure mod provider connection".to_owned()))
}

fn get_with_query(client: &Client, endpoint: &str, parameters: &[(&str, String)]) -> Result<RequestBuilder> {
    let mut url = reqwest::Url::parse(endpoint)
        .map_err(|_| LauncherError::Unsupported("could not construct a secure provider request".to_owned()))?;
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in parameters {
            pairs.append_pair(key, value);
        }
    }
    Ok(client.get(url))
}

fn read_json(response: Response) -> Result<Value> {
    let status = response.status();
    if status.as_u16() == 429 {
        return Err(LauncherError::Unsupported("the mod provider is rate limiting requests. Wait a moment and try again".to_owned()));
    }
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(LauncherError::Unsupported("the provider denied this request. If you are using CurseForge, check your API key access".to_owned()));
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(LauncherError::NotFound("the selected mod or version is no longer available".to_owned()));
    }
    if !status.is_success() {
        return Err(LauncherError::Unsupported(format!("the mod provider returned HTTP {}", status.as_u16())));
    }
    if response.content_length().is_some_and(|length| length > MAX_API_RESPONSE) {
        return Err(LauncherError::Unsupported("the mod provider response is too large".to_owned()));
    }
    let mut limited = response.take(MAX_API_RESPONSE + 1);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_API_RESPONSE {
        return Err(LauncherError::Unsupported("the mod provider response is too large".to_owned()));
    }
    serde_json::from_slice(&bytes).map_err(Into::into)
}

fn network_error(_error: reqwest::Error) -> LauncherError {
    LauncherError::Unsupported("could not connect securely to the mod provider. Check your connection and try again".to_owned())
}

fn safe_image_url(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else { return false; };
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() { return false; }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    host == MODRINTH_IMAGE_HOST || host.ends_with(".forgecdn.net")
}

fn safe_project_url(value: &str, provider: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else { return false; };
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() { return false; }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    match provider {
        "modrinth" => host == "modrinth.com" || host == "www.modrinth.com",
        "curseforge" => host == "curseforge.com" || host == "www.curseforge.com",
        _ => false,
    }
}

fn download_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("OpnLauncher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .redirect(Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() >= 3 || !is_trusted_download_host(url) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| LauncherError::Unsupported("could not initialize the secure mod download connection".to_owned()))
}

fn is_trusted_download_host(url: &reqwest::Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    host == MODRINTH_IMAGE_HOST || host == "forgecdn.net" || host.ends_with(".forgecdn.net")
}

fn detect_loader(metadata: &Value, version_id: &str) -> Option<String> {
    let id = version_id.to_ascii_lowercase();
    let main_class = metadata.get("mainClass").and_then(Value::as_str).unwrap_or_default().to_ascii_lowercase();
    let libraries = metadata.get("libraries").and_then(Value::as_array).into_iter().flatten().filter_map(|library| library.get("name").and_then(Value::as_str)).map(str::to_ascii_lowercase).collect::<Vec<_>>();
    let names = std::iter::once(id.as_str()).chain(std::iter::once(main_class.as_str())).chain(libraries.iter().map(String::as_str));
    let joined = names.collect::<Vec<_>>().join(" ");
    if joined.contains("neoforge") { Some("neoforge".to_owned()) }
    else if joined.contains("fabric-loader") || joined.contains("net.fabricmc:fabric-loader") { Some("fabric".to_owned()) }
    else if joined.contains("quilt-loader") || joined.contains("org.quiltmc:quilt-loader") { Some("quilt".to_owned()) }
    else if joined.contains("forge") { Some("forge".to_owned()) }
    else { None }
}

fn canonical_game_directory(instance: &Instance) -> Result<PathBuf> {
    let game_dir = fs::canonicalize(&instance.game_dir).map_err(|_| LauncherError::NotFound(format!("game folder for {} is unavailable", instance.name)))?;
    if !game_dir.is_dir() {
        return Err(LauncherError::Unsupported("the instance game folder is not a directory".to_owned()));
    }
    Ok(game_dir)
}

fn ensure_mods_directory(game_dir: &Path) -> Result<PathBuf> {
    let mods_dir = game_dir.join("mods");
    match fs::symlink_metadata(&mods_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(LauncherError::Unsupported("the instance mods path is not a regular directory".to_owned()))
        }
        Ok(_) => Ok(mods_dir),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&mods_dir)?;
            Ok(mods_dir)
        }
        Err(error) => Err(error.into()),
    }
}

fn validate_mod_file_name(file_name: &str) -> Result<()> {
    let path = Path::new(file_name);
    let valid_component = path.components().count() == 1 && matches!(path.components().next(), Some(Component::Normal(_)));
    let lower = file_name.to_ascii_lowercase();
    if !valid_component || file_name.len() > 240 || file_name.chars().any(|character| character.is_control() || matches!(character, '/' | '\\' | ':')) || !(lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
        return Err(LauncherError::InvalidInput("mod file name is invalid".to_owned()));
    }
    Ok(())
}

fn ensure_regular_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(LauncherError::Unsupported("mod file must be a regular file".to_owned()));
    }
    Ok(())
}

fn move_mod_file(source: &Path, destination: &Path) -> Result<()> {
    if path_exists_no_follow(destination)? {
        return Err(LauncherError::Unsupported("a mod file with that name already exists".to_owned()));
    }
    fs::rename(source, destination)?;
    Ok(())
}

fn path_exists_no_follow(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn read_jar_identity(path: &Path, file_name: &str) -> (String, Option<String>, Option<String>) {
    let fallback = file_name.strip_suffix(".disabled").unwrap_or(file_name).strip_suffix(".jar").unwrap_or(file_name).to_owned();
    let Ok(file) = File::open(path) else { return (fallback, None, None); };
    let Ok(mut archive) = ZipArchive::new(file) else { return (fallback, None, None); };
    let Ok(manifest) = archive.by_name("fabric.mod.json") else { return (fallback, None, None); };
    let mut bytes = Vec::new();
    if manifest.take(2 * 1024 * 1024).read_to_end(&mut bytes).is_err() { return (fallback, None, None); }
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else { return (fallback, None, None); };
    (
        value.get("name").and_then(Value::as_str).unwrap_or(&fallback).to_owned(),
        value.get("version").and_then(Value::as_str).map(str::to_owned),
        value.get("id").and_then(Value::as_str).map(str::to_owned),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mod_loaders_from_version_metadata() {
        let fabric = serde_json::json!({"mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient"});
        let forge = serde_json::json!({"libraries":[{"name":"net.minecraftforge:forge:1.20.1-47.2.0"}]});
        let neo = serde_json::json!({"libraries":[{"name":"net.neoforged:neoforge:21.1.0"}]});
        assert_eq!(detect_loader(&fabric, "fabric-loader-0.16.0-1.21.1").as_deref(), Some("fabric"));
        assert_eq!(detect_loader(&forge, "1.20.1-forge-47.2.0").as_deref(), Some("forge"));
        assert_eq!(detect_loader(&neo, "neoforge-21.1.0-1.21.1").as_deref(), Some("neoforge"));
        assert_eq!(detect_loader(&serde_json::json!({}), "1.21.1"), None);
    }

    #[test]
    fn rejects_unsafe_mod_names_and_provider_images() {
        assert!(validate_mod_file_name("../outside.jar").is_err());
        assert!(validate_mod_file_name("nested/file.jar").is_err());
        assert!(validate_mod_file_name(r"..\outside.jar").is_err());
        assert!(validate_mod_file_name("C:outside.jar").is_err());
        assert!(validate_mod_file_name("sample.jar").is_ok());
        assert!(safe_image_url("https://cdn.modrinth.com/data/a/icon.png"));
        assert!(safe_image_url("https://media.forgecdn.net/files/a/screen.png"));
        assert!(!safe_image_url("http://cdn.modrinth.com/data/a/icon.png"));
        assert!(!safe_image_url("https://example.invalid/icon.png"));
        assert!(safe_project_url("https://www.curseforge.com/minecraft/mc-mods/example", "curseforge"));
        assert!(!safe_project_url("https://example.invalid/", "curseforge"));
    }

    #[test]
    fn parses_modrinth_search_and_project_gallery_images() {
        let search_hit = serde_json::json!({
            "project_id": "example",
            "project_type": "mod",
            "title": "Example Mod",
            "author": "Example Author",
            "description": "A sample project.",
            "icon_url": "https://cdn.modrinth.com/data/example/icon.png",
            "gallery": [
                "https://cdn.modrinth.com/data/example/images/first.png",
                "https://cdn.modrinth.com/data/example/images/featured.png",
                "https://example.invalid/outside.png"
            ],
            "featured_gallery": "https://cdn.modrinth.com/data/example/images/featured.png"
        });
        let parsed = parse_modrinth_project(&search_hit, "mod").unwrap();
        assert_eq!(parsed.gallery.len(), 2);
        assert_eq!(parsed.gallery[0].url, "https://cdn.modrinth.com/data/example/images/featured.png");
        assert_eq!(parsed.gallery[1].url, "https://cdn.modrinth.com/data/example/images/first.png");

        let project_detail = serde_json::json!({
            "id": "example",
            "project_type": "mod",
            "title": "Example Mod",
            "team": "Example Author",
            "description": "A sample project.",
            "gallery": [{"url":"https://cdn.modrinth.com/data/example/images/detail.png", "title":"In game"}]
        });
        let parsed = parse_modrinth_project(&project_detail, "mod").unwrap();
        assert_eq!(parsed.gallery.len(), 1);
        assert_eq!(parsed.gallery[0].title.as_deref(), Some("In game"));
    }

    #[test]
    fn curseforge_file_versions_must_match_the_instance_game_version() {
        let file = serde_json::json!({
            "id": 101,
            "fileName": "example.jar",
            "gameVersions": ["1.20.1"],
            "fileLength": 1024,
            "dependencies": [{"modId": 202, "relationType": 3}]
        });
        let compatible = parse_curseforge_version(&file, "100", "1.20.1", "forge").unwrap();
        assert_eq!(compatible.loaders, ["forge"]);
        assert_eq!(compatible.dependencies[0].project_id.as_deref(), Some("202"));
        assert!(parse_curseforge_version(&file, "100", "1.19.4", "forge").is_none());
    }

    #[test]
    fn provider_request_query_values_are_encoded() {
        let client = Client::builder().build().unwrap();
        let request = get_with_query(
            &client,
            "https://api.modrinth.com/v2/search",
            &[("query", "sodium + lithium".to_owned()), ("offset", "24".to_owned())],
        ).unwrap().build().unwrap();
        assert_eq!(request.url().query(), Some("query=sodium+%2B+lithium&offset=24"));
    }
}
