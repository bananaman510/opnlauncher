use std::collections::{BTreeSet, HashMap};
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

use crate::database::{Database, ModInstallSource};
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
    pub source: Option<ModInstallSource>,
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
    version_id: String,
    file_name: String,
    url: String,
    sha1: Option<String>,
    size: Option<u64>,
    version_pinned: bool,
}

#[derive(Clone, Debug)]
struct ModArtifactIntegrity {
    file_name: String,
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

pub fn get_all_project_versions(provider: &str, project_id: &str, api_key: Option<&str>) -> Result<Vec<ModVersion>> {
    validate_project_id(provider, project_id)?;
    match provider {
        "modrinth" => {
            let client = api_client(false)?;
            let response = get_with_query(
                &client,
                &format!("{MODRINTH_API}/project/{project_id}/version"),
                &[("include_changelog", "false".to_owned())],
            )?
            .send()
            .map_err(network_error)?;
            let body = read_json(response)?;
            let items = body.as_array().ok_or_else(|| LauncherError::Unsupported("Modrinth returned an unexpected version response".to_owned()))?;
            if items.len() > 2_000 {
                return Err(LauncherError::Unsupported("Modrinth returned too many project versions".to_owned()));
            }
            Ok(items.iter().filter_map(|item| parse_modrinth_version(item, project_id)).collect())
        }
        "curseforge" => get_all_curseforge_versions(project_id, api_key),
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
    let files = match provider {
        "modrinth" => collect_modrinth_install_files(project_id, version_id, game_version, loader)?,
        "curseforge" => collect_curseforge_install_files(project_id, version_id, game_version, loader, api_key)?,
        _ => return Err(LauncherError::InvalidInput("unknown mod provider".to_owned())),
    };
    if files.is_empty() || files.len() > MAX_DEPENDENCIES + 1 {
        return Err(LauncherError::Unsupported("the selected mod has an unsupported dependency count".to_owned()));
    }
    let game_dir = canonical_game_directory(&instance)?;
    let mods_dir = ensure_mods_directory(&game_dir)?;
    let files = reuse_installed_dependencies(database, instance_id, provider, project_id, files, game_version, loader, api_key, &mods_dir)?;
    let mut files = files;
    files.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    let sources = install_sources(provider, &files);
    stage_and_install(files, &mods_dir, download_id, &cancelled, progress, None)?;
    database.save_mod_sources(instance_id, &sources)?;
    list_installed_mods(database, instance_id)
}

fn install_sources(provider: &str, files: &[InstallFile]) -> Vec<ModInstallSource> {
    files.iter().map(|file| ModInstallSource {
        file_name: file.file_name.clone(),
        provider: provider.to_owned(),
        project_id: file.project_id.clone(),
        version_id: Some(file.version_id.clone()),
    }).collect()
}

pub fn resolve_installed_mod_source(
    database: &Database,
    instance_id: &str,
    file_name: &str,
) -> Result<Option<ModInstallSource>> {
    validate_mod_file_name(file_name)?;
    let saved_source = database.get_mod_source(instance_id, file_name)?;
    if saved_source.as_ref().is_some_and(|source| source.version_id.is_some() || source.provider != "modrinth") {
        return Ok(saved_source);
    }
    let instance = database.get_instance(instance_id)?;
    let game_dir = canonical_game_directory(&instance)?;
    let mods_dir = game_dir.join("mods");
    let path = mods_dir.join(file_name);
    ensure_regular_file(&path)?;
    let hash = sha1_file(&path)?;
    let client = api_client(false)?;
    let url = format!("{MODRINTH_API}/version_file/{hash}");
    let response = get_with_query(&client, &url, &[("algorithm", "sha1".to_owned())])?
        .send()
        .map_err(network_error)?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(saved_source);
    }
    let version = read_json(response)?;
    let Some(project_id) = version.get("project_id").and_then(Value::as_str) else {
        return Ok(saved_source);
    };
    let Some(version_id) = version.get("id").and_then(Value::as_str) else {
        return Ok(saved_source);
    };
    validate_project_id("modrinth", project_id)?;
    validate_project_id("modrinth version", version_id)?;
    if saved_source.as_ref().is_some_and(|source| source.project_id != project_id) {
        return Ok(saved_source);
    }
    let project = get_modrinth_project(project_id)?;
    if !project.project_url.as_deref().is_some_and(|url| url.starts_with("https://modrinth.com/mod/")) {
        return Ok(saved_source);
    }
    let source = ModInstallSource {
        file_name: file_name.to_owned(),
        provider: "modrinth".to_owned(),
        project_id: project_id.to_owned(),
        version_id: Some(version_id.to_owned()),
    };
    database.save_mod_sources(instance_id, std::slice::from_ref(&source))?;
    Ok(Some(source))
}

pub fn link_installed_mod_to_project(
    database: &Database,
    instance_id: &str,
    file_name: &str,
    provider: &str,
    project_id: &str,
    api_key: Option<&str>,
) -> Result<ModInstallSource> {
    validate_mod_file_name(file_name)?;
    validate_project_id(provider, project_id)?;
    let instance = database.get_instance(instance_id)?;
    let game_dir = canonical_game_directory(&instance)?;
    let mods_dir = game_dir.join("mods");
    ensure_regular_file(&mods_dir.join(file_name))?;
    let project = get_project(provider, project_id, api_key)?;
    if provider == "modrinth"
        && !project.project_url.as_deref().is_some_and(|url| url.starts_with("https://modrinth.com/mod/"))
    {
        return Err(LauncherError::InvalidInput("choose a Modrinth mod project".to_owned()));
    }
    let source = ModInstallSource {
        file_name: file_name.to_owned(),
        provider: provider.to_owned(),
        project_id: project.id,
        version_id: None,
    };
    database.save_mod_sources(instance_id, std::slice::from_ref(&source))?;
    Ok(source)
}

pub fn switch_project_version(
    database: &Database,
    instance_id: &str,
    old_file_name: &str,
    provider: &str,
    project_id: &str,
    version_id: &str,
    api_key: Option<&str>,
    download_id: &str,
    cancelled: Arc<AtomicBool>,
    progress: ModProgressSink,
) -> Result<Vec<InstalledMod>> {
    validate_mod_file_name(old_file_name)?;
    validate_project_id(provider, project_id)?;
    validate_project_id(provider, version_id)?;
    Uuid::parse_str(download_id).map_err(|_| LauncherError::InvalidInput("download identifier is invalid".to_owned()))?;
    let source = database.get_mod_source(instance_id, old_file_name)?
        .ok_or_else(|| LauncherError::Unsupported("identify this installed mod's provider project before switching versions".to_owned()))?;
    if source.provider != provider || source.project_id != project_id {
        return Err(LauncherError::InvalidInput("the selected project does not match this installed mod".to_owned()));
    }
    if source.version_id.as_deref() == Some(version_id) {
        return list_installed_mods(database, instance_id);
    }
    let instance = database.get_instance(instance_id)?;
    let context = inspect_instance(database, instance_id)?;
    let game_version = context.minecraft_version.as_deref().ok_or_else(|| LauncherError::Unsupported("the selected instance has no readable Minecraft version metadata".to_owned()))?;
    let loader = context.loader.as_deref().ok_or_else(|| LauncherError::Unsupported("installing mods requires a loader-equipped instance".to_owned()))?;
    validate_context(game_version, loader)?;
    let files = match provider {
        "modrinth" => collect_modrinth_install_files(project_id, version_id, game_version, loader)?,
        "curseforge" => collect_curseforge_install_files(project_id, version_id, game_version, loader, api_key)?,
        _ => return Err(LauncherError::InvalidInput("unknown mod provider".to_owned())),
    };
    if files.is_empty() || files.len() > MAX_DEPENDENCIES + 1 {
        return Err(LauncherError::Unsupported("the selected mod has an unsupported dependency count".to_owned()));
    }
    let mut files = files;
    if old_file_name.to_ascii_lowercase().ends_with(".jar.disabled") {
        if let Some(root) = files.iter_mut().find(|file| file.project_id == project_id) {
            root.file_name.push_str(".disabled");
        }
    }
    let game_dir = canonical_game_directory(&instance)?;
    let mods_dir = ensure_mods_directory(&game_dir)?;
    let files = reuse_installed_dependencies(database, instance_id, provider, project_id, files, game_version, loader, api_key, &mods_dir)?;
    let mut files = files;
    files.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    let sources = install_sources(provider, &files);
    stage_and_install(files, &mods_dir, download_id, &cancelled, progress, Some((old_file_name, project_id)))?;
    database.replace_mod_sources(instance_id, old_file_name, &sources)?;
    list_installed_mods(database, instance_id)
}

pub fn remove_mod_files(database: &Database, instance_id: &str, file_names: &[String], confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(LauncherError::InvalidInput("confirm removal before deleting mod files".to_owned()));
    }
    if file_names.is_empty() || file_names.len() > 512 {
        return Err(LauncherError::InvalidInput("select between 1 and 512 mod files to remove".to_owned()));
    }
    let mut unique = BTreeSet::new();
    for file_name in file_names {
        validate_mod_file_name(file_name)?;
        if !unique.insert(file_name) {
            return Err(LauncherError::InvalidInput("a mod file was selected more than once".to_owned()));
        }
    }
    let instance = database.get_instance(instance_id)?;
    let game_dir = canonical_game_directory(&instance)?;
    let mods_dir = ensure_mods_directory(&game_dir)?;
    for file_name in file_names {
        ensure_regular_file(&mods_dir.join(file_name))?;
    }
    for file_name in file_names {
        fs::remove_file(mods_dir.join(file_name))?;
    }
    database.remove_mod_sources(instance_id, file_names)?;
    Ok(())
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
    let sources = database.list_mod_sources(instance_id)?.into_iter().map(|source| (source.file_name.clone(), source)).collect::<HashMap<_, _>>();
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
        let source = sources.get(&file_name).cloned();
        installed.push(InstalledMod {
            file_name,
            name,
            version,
            mod_id,
            source,
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
        let destination_name = format!("{file_name}.disabled");
        move_mod_file(&source, &mods_dir.join(&destination_name))?;
        if let Err(error) = database.rename_mod_source(instance_id, file_name, &destination_name) {
            let _ = fs::rename(mods_dir.join(&destination_name), &source);
            return Err(error);
        }
        return Ok(());
    };
    let destination = mods_dir.join(destination_name);
    move_mod_file(&source, &destination)?;
    if let Err(error) = database.rename_mod_source(instance_id, file_name, destination_name) {
        let _ = fs::rename(&destination, &source);
        return Err(error);
    }
    Ok(())
}

pub fn remove_mod(database: &Database, instance_id: &str, file_name: &str, confirmed: bool) -> Result<()> {
    remove_mod_files(database, instance_id, &[file_name.to_owned()], confirmed)
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
    collect_modrinth_version(&client, root, Some((project_id, root_title.as_str())), false, game_version, loader, &mut visited, &mut files, 0)?;
    Ok(files)
}

fn collect_modrinth_version(
    client: &Client,
    version: Value,
    expected_project: Option<(&str, &str)>,
    version_pinned: bool,
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
    let version_id = version.get("id").and_then(Value::as_str).ok_or_else(|| LauncherError::Unsupported("a mod version has no version identifier".to_owned()))?.to_owned();
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
    files.push(parse_modrinth_install_file(primary, &project_id, &title, &version_id, version_pinned)?);

    let dependencies = version.get("dependencies").and_then(Value::as_array).cloned().unwrap_or_default();
    for dependency in dependencies {
        if dependency.get("dependency_type").and_then(Value::as_str) != Some("required") {
            continue;
        }
        if files.len() >= MAX_DEPENDENCIES + 1 {
            return Err(LauncherError::Unsupported("this mod requires too many other projects".to_owned()));
        }
        let declared_project_id = dependency.get("project_id").and_then(Value::as_str);
        let (dependency_version, dependency_version_pinned) = if let Some(version_id) = dependency.get("version_id").and_then(Value::as_str) {
            let version = get_modrinth_version(client, version_id)?;
            if declared_project_id.is_some_and(|expected| version.get("project_id").and_then(Value::as_str) != Some(expected)) {
                return Err(LauncherError::Unsupported("a required dependency version does not match its declared project".to_owned()));
            }
            (version, true)
        } else if let Some(dependency_project) = dependency.get("project_id").and_then(Value::as_str) {
            let versions = get_modrinth_project_versions(client, dependency_project, game_version, loader)?;
            let version = versions.iter().find(|entry| entry.get("version_type").and_then(Value::as_str) == Some("release"))
                .or_else(|| versions.first())
                .cloned()
                .ok_or_else(|| LauncherError::Unsupported(format!("a required dependency ({dependency_project}) has no compatible version")))?;
            (version, false)
        } else {
            return Err(LauncherError::Unsupported("a required dependency has no downloadable project or version ID; install it manually from its author".to_owned()));
        };
        collect_modrinth_version(client, dependency_version, None, dependency_version_pinned, game_version, loader, visited, files, depth + 1)?;
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

fn parse_modrinth_install_file(file: &Value, project_id: &str, title: &str, version_id: &str, version_pinned: bool) -> Result<InstallFile> {
    let file_name = file.get("filename").and_then(Value::as_str).ok_or_else(|| LauncherError::Unsupported("a mod file has no filename".to_owned()))?.to_owned();
    validate_mod_file_name(&file_name)?;
    let url = file.get("url").and_then(Value::as_str).ok_or_else(|| LauncherError::Unsupported("a mod file has no download URL".to_owned()))?.to_owned();
    validate_download_url(&url, "modrinth")?;
    let sha1 = file.pointer("hashes.sha1").and_then(Value::as_str).map(str::to_owned);
    let size = file.get("size").and_then(Value::as_u64);
    if sha1.as_deref().is_none_or(|hash| !is_sha1(hash)) && size.is_none() {
        return Err(LauncherError::Unsupported("a mod file has no usable integrity metadata".to_owned()));
    }
    Ok(InstallFile { project_id: project_id.to_owned(), project_title: title.to_owned(), version_id: version_id.to_owned(), file_name, url, sha1, size, version_pinned })
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
    let mut files = vec![InstallFile { project_id: project_id.to_owned(), project_title: project.title, version_id: version.id.clone(), file_name, url, sha1, size, version_pinned: false }];
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
        files.push(InstallFile { project_id: dependency_id.to_owned(), project_title: mod_project.title, version_id: dep_id, file_name: dep_file_name, url: dependency_url, sha1: dep_sha1, size: dep_size, version_pinned: false });
    }
    Ok(files)
}

fn reuse_installed_dependencies(
    database: &Database,
    instance_id: &str,
    provider: &str,
    root_project_id: &str,
    files: Vec<InstallFile>,
    game_version: &str,
    loader: &str,
    api_key: Option<&str>,
    mods_dir: &Path,
) -> Result<Vec<InstallFile>> {
    let mut installed = list_installed_mods(database, instance_id)?;

    // Older installs may not have provider metadata yet. For Modrinth projects,
    // use its documented hash lookup only for local files that look like one of
    // the dependencies being installed.
    if provider == "modrinth" {
        for existing in installed.iter_mut().filter(|item| {
            item.source.as_ref().is_none_or(|source| source.provider == "modrinth" && source.version_id.is_none())
        }) {
            let relevant = files.iter().any(|file| {
                file.project_id != root_project_id
                    && (normalized_mod_name(&file.project_title) == normalized_mod_name(&existing.name)
                        || file.file_name.eq_ignore_ascii_case(mod_jar_name(&existing.file_name)))
            });
            if relevant {
                existing.source = resolve_installed_mod_source(database, instance_id, &existing.file_name)?;
            }
        }
    }

    let mut reused_project_ids = BTreeSet::new();
    for file in &files {
        if file.project_id == root_project_id {
            continue;
        }
        let matches_dependency = |item: &&InstalledMod| item.source.as_ref().is_some_and(|source| source.provider == provider && source.project_id == file.project_id);
        let Some(existing) = installed.iter().find(|item| item.enabled && matches_dependency(item)).or_else(|| installed.iter().find(|item| matches_dependency(item))) else {
            continue;
        };

        let Some(source) = existing.source.as_ref() else {
            continue;
        };
        let Some(version_id) = source.version_id.as_deref() else {
            return Err(LauncherError::Unsupported(format!(
                "OpnLauncher cannot verify the installed {} dependency version; identify or remove it, then try again",
                existing.name
            )));
        };
        let (compatible, artifacts) = installed_source_artifacts(source, version_id, game_version, loader, api_key)?;
        if !compatible {
            return Err(LauncherError::Unsupported(format!(
                "the installed {} version does not support Minecraft {} with {}; switch or remove it before installing this mod",
                existing.name, game_version, loader
            )));
        }
        let existing_path = mods_dir.join(&existing.file_name);
        ensure_regular_file(&existing_path)?;
        let verified = artifacts.iter().try_fold(false, |matched, artifact| {
            if matched { Ok(true) } else { artifact_matches_file(artifact, &existing_path, &existing.file_name) }
        })?;
        if can_reuse_dependency(file, source, &existing.name, existing.enabled, compatible, verified)? {
            reused_project_ids.insert(file.project_id.clone());
        }
    }
    Ok(filter_reused_dependencies(files, root_project_id, &reused_project_ids))
}

fn installed_source_artifacts(
    source: &ModInstallSource,
    version_id: &str,
    game_version: &str,
    loader: &str,
    api_key: Option<&str>,
) -> Result<(bool, Vec<ModArtifactIntegrity>)> {
    match source.provider.as_str() {
        "modrinth" => {
            let client = api_client(false)?;
            let version = get_modrinth_version(&client, version_id)?;
            if version.get("project_id").and_then(Value::as_str) != Some(source.project_id.as_str()) {
                return Ok((false, Vec::new()));
            }
            let compatible = version.get("game_versions").and_then(Value::as_array)
                .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(game_version)))
                && version.get("loaders").and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(loader)));
            let artifacts = version.get("files").and_then(Value::as_array).into_iter().flatten().filter_map(|file| {
                Some(ModArtifactIntegrity {
                    file_name: file.get("filename")?.as_str()?.to_owned(),
                    sha1: file.pointer("hashes.sha1").and_then(Value::as_str).map(str::to_owned),
                    size: file.get("size").and_then(Value::as_u64),
                })
            }).collect();
            Ok((compatible, artifacts))
        }
        "curseforge" => {
            let api_key = validate_curseforge_key(api_key)?;
            let project_id = validate_curseforge_id(&source.project_id)?;
            let file_id = validate_curseforge_id(version_id)?;
            let client = api_client(true)?;
            let body = read_json(client.get(format!("{CURSEFORGE_API}/mods/{project_id}/files/{file_id}"))
                .header("x-api-key", api_key).send().map_err(network_error)?)?;
            let file = body.get("data").ok_or_else(|| LauncherError::Unsupported("CurseForge returned an unexpected installed dependency response".to_owned()))?;
            let game_versions = string_list(file.get("gameVersions"));
            let compatible = game_versions.iter().any(|version| version == game_version)
                && game_versions.iter().any(|version| curseforge_loader_name(version) == Some(loader));
            let artifacts = file.get("fileName").and_then(Value::as_str).map(|file_name| {
                let sha1 = file.get("hashes").and_then(Value::as_array).into_iter().flatten()
                    .find(|hash| hash.get("algo").and_then(Value::as_u64) == Some(1))
                    .and_then(|hash| hash.get("value")).and_then(Value::as_str).map(str::to_owned);
                vec![ModArtifactIntegrity { file_name: file_name.to_owned(), sha1, size: file.get("fileLength").and_then(Value::as_u64) }]
            }).unwrap_or_default();
            Ok((compatible, artifacts))
        }
        _ => Ok((false, Vec::new())),
    }
}

fn artifact_matches_file(artifact: &ModArtifactIntegrity, path: &Path, installed_name: &str) -> Result<bool> {
    let installed_name = mod_jar_name(installed_name);
    if !artifact.file_name.eq_ignore_ascii_case(installed_name) {
        return Ok(false);
    }
    if artifact.sha1.as_deref().is_none_or(|hash| !is_sha1(hash)) && artifact.size.is_none() {
        return Ok(false);
    }
    let metadata = fs::metadata(path)?;
    if artifact.size.is_some_and(|expected| metadata.len() != expected) {
        return Ok(false);
    }
    if let Some(expected) = artifact.sha1.as_deref().filter(|hash| is_sha1(hash)) {
        if !sha1_file(path)?.eq_ignore_ascii_case(expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn mod_jar_name(file_name: &str) -> &str {
    if file_name.to_ascii_lowercase().ends_with(".jar.disabled") {
        &file_name[..file_name.len() - ".disabled".len()]
    } else {
        file_name
    }
}

fn normalized_mod_name(value: &str) -> String {
    value.chars().filter(char::is_ascii_alphanumeric).flat_map(char::to_lowercase).collect()
}

fn filter_reused_dependencies(files: Vec<InstallFile>, root_project_id: &str, reused_project_ids: &BTreeSet<String>) -> Vec<InstallFile> {
    files.into_iter().filter(|file| file.project_id == root_project_id || !reused_project_ids.contains(&file.project_id)).collect()
}

fn can_reuse_dependency(
    file: &InstallFile,
    source: &ModInstallSource,
    installed_name: &str,
    enabled: bool,
    compatible: bool,
    integrity_verified: bool,
) -> Result<bool> {
    if !enabled {
        return Err(LauncherError::Unsupported(format!(
            "{} is required by this mod but is disabled in this instance; enable it and try again",
            installed_name
        )));
    }
    if file.version_pinned && source.version_id.as_deref() != Some(file.version_id.as_str()) {
        return Err(LauncherError::Unsupported(format!(
            "{} requires a specific version of {}, but a different version is already installed; switch or remove the installed dependency first",
            file.project_title, installed_name
        )));
    }
    if !compatible {
        return Err(LauncherError::Unsupported(format!(
            "the installed {} version does not support this instance's Minecraft version and mod loader; switch or remove it before installing this mod",
            installed_name
        )));
    }
    if !integrity_verified {
        return Err(LauncherError::Unsupported(format!(
            "the installed {} file no longer matches its provider's integrity data; reinstall or remove it before installing this mod",
            installed_name
        )));
    }
    Ok(true)
}

fn stage_and_install(
    files: Vec<InstallFile>,
    mods_dir: &Path,
    download_id: &str,
    cancelled: &Arc<AtomicBool>,
    progress: ModProgressSink,
    replacement: Option<(&str, &str)>,
) -> Result<()> {
    let total_files = u32::try_from(files.len()).map_err(|_| LauncherError::Unsupported("too many mod files".to_owned()))?;
    let total_bytes = files.iter().try_fold(0_u64, |sum, file| sum.checked_add(file.size.unwrap_or(0)).filter(|sum| *sum <= MAX_INSTALL_SIZE))
        .ok_or_else(|| LauncherError::Unsupported("required mod files exceed the 2 GiB install limit".to_owned()))?;
    let primary_title = files.first().map(|file| file.project_title.clone()).unwrap_or_else(|| "Mod".to_owned());
    let staging_dir = mods_dir.join(format!(".opnlauncher-staging-{download_id}"));
    let replacement_path = replacement.map(|(file_name, _)| mods_dir.join(file_name));
    if let (Some((file_name, _)), Some(path)) = (replacement, replacement_path.as_deref()) {
        validate_mod_file_name(file_name)?;
        ensure_regular_file(path)?;
    }
    fs::create_dir(&staging_dir)?;
    let mut staged = Vec::new();
    let backup_path = staging_dir.join(".previous-mod-file");
    let mut preserve_replacement = false;
    let result = (|| {
        let mut downloaded = 0_u64;
        for (index, file) in files.iter().enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                return Err(LauncherError::Unsupported("mod installation cancelled".to_owned()));
            }
            validate_mod_file_name(&file.file_name)?;
            let destination = mods_dir.join(&file.file_name);
            let is_replacement_root = replacement.is_some_and(|(_, project_id)| file.project_id == project_id);
            let is_original_file = replacement.is_some_and(|(file_name, _)| file_name == file.file_name);
            if path_exists_no_follow(&destination)? {
                ensure_regular_file(&destination)?;
                if file.sha1.as_deref().is_some_and(|expected| sha1_file(&destination).is_ok_and(|actual| actual.eq_ignore_ascii_case(expected))) {
                    if is_replacement_root && is_original_file {
                        preserve_replacement = true;
                    }
                    downloaded = downloaded.saturating_add(file.size.unwrap_or(fs::metadata(&destination)?.len()));
                    progress(ModDownloadProgress { download_id: download_id.to_owned(), project_title: file.project_title.clone(), phase: "Already installed".to_owned(), current_file: file.file_name.clone(), completed_files: index as u32 + 1, total_files, downloaded_bytes: downloaded, total_bytes });
                    continue;
                }
                if is_replacement_root && is_original_file {
                    // The old file is moved aside only after every new artifact has been downloaded and verified.
                } else {
                    return Err(LauncherError::Unsupported(format!("{} already exists in this instance's mods folder; remove or disable it before replacing it", file.file_name)));
                }
            }
            if let Some(alternate_name) = alternate_mod_state_name(&file.file_name) {
                let alternate = mods_dir.join(&alternate_name);
                if path_exists_no_follow(&alternate)? {
                    ensure_regular_file(&alternate)?;
                    return Err(LauncherError::Unsupported(format!("{} is already present in this instance's mods folder; re-enable or remove it before installing another copy", alternate_name)));
                }
            }
            let temp_file = staging_dir.join(&file.file_name);
            progress(ModDownloadProgress { download_id: download_id.to_owned(), project_title: file.project_title.clone(), phase: "Downloading mod files".to_owned(), current_file: file.file_name.clone(), completed_files: index as u32, total_files, downloaded_bytes: downloaded, total_bytes });
            let actual_size = download_and_verify(file, &temp_file, cancelled, download_id, total_files, index as u32, downloaded, total_bytes, &progress)?;
            downloaded = downloaded.saturating_add(actual_size);
            staged.push((temp_file, destination, file.file_name.clone()));
        }

        if cancelled.load(Ordering::Relaxed) {
            return Err(LauncherError::Unsupported("mod installation cancelled".to_owned()));
        }
        let mut replacement_backed_up = false;
        if !preserve_replacement {
            if let Some(original) = replacement_path.as_deref() {
                fs::rename(original, &backup_path)?;
                replacement_backed_up = true;
            }
        }
        let mut installed: Vec<PathBuf> = Vec::new();
        for (source, destination, file_name) in &staged {
            if cancelled.load(Ordering::Relaxed) {
                for moved in installed.iter().rev() { let _ = fs::remove_file(moved); }
                if replacement_backed_up {
                    if let Some(original) = replacement_path.as_deref() {
                        if let Err(error) = fs::rename(&backup_path, original) {
                            return Err(LauncherError::Unsupported(format!("installation was cancelled and the old mod could not be restored ({error}); it remains in {}", backup_path.display())));
                        }
                    }
                }
                return Err(LauncherError::Unsupported("mod installation cancelled".to_owned()));
            }
            if let Err(error) = fs::hard_link(source, destination) {
                for moved in installed.iter().rev() { let _ = fs::remove_file(moved); }
                if replacement_backed_up {
                    if let Some(original) = replacement_path.as_deref() {
                        if let Err(restore_error) = fs::rename(&backup_path, original) {
                            return Err(LauncherError::Unsupported(format!("installation failed and the old mod could not be restored ({restore_error}); it remains in {}", backup_path.display())));
                        }
                    }
                }
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    return Err(LauncherError::Unsupported(format!("{} appeared during installation; no files were replaced", file_name)));
                }
                return Err(error.into());
            }
            installed.push(destination.clone());
            if let Err(error) = fs::remove_file(source) {
                for moved in installed.iter().rev() { let _ = fs::remove_file(moved); }
                if replacement_backed_up {
                    if let Some(original) = replacement_path.as_deref() {
                        if let Err(restore_error) = fs::rename(&backup_path, original) {
                            return Err(LauncherError::Unsupported(format!("installation failed and the old mod could not be restored ({restore_error}); it remains in {}", backup_path.display())));
                        }
                    }
                }
                return Err(error.into());
            }
        }
        if replacement_backed_up {
            if let Err(error) = fs::remove_file(&backup_path) {
                for moved in installed.iter().rev() { let _ = fs::remove_file(moved); }
                if let Some(original) = replacement_path.as_deref() {
                    if let Err(restore_error) = fs::rename(&backup_path, original) {
                        return Err(LauncherError::Unsupported(format!("installation failed and the old mod could not be restored ({restore_error}); it remains in {}", backup_path.display())));
                    }
                }
                return Err(error.into());
            }
        }
        progress(ModDownloadProgress { download_id: download_id.to_owned(), project_title: primary_title, phase: "Installation complete".to_owned(), current_file: String::new(), completed_files: total_files, total_files, downloaded_bytes: downloaded, total_bytes });
        Ok(())
    })();
    if !backup_path.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
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

fn get_all_curseforge_versions(project_id: &str, api_key: Option<&str>) -> Result<Vec<ModVersion>> {
    let api_key = validate_curseforge_key(api_key)?;
    let id = validate_curseforge_id(project_id)?;
    let client = api_client(true)?;
    const PAGE_SIZE: u32 = 50;
    const MAX_PAGES: u32 = 10;
    let mut versions = Vec::new();
    for page in 0..MAX_PAGES {
        let index = page * PAGE_SIZE;
        let request = get_with_query(
            &client,
            &format!("{CURSEFORGE_API}/mods/{id}/files"),
            &[("pageSize", PAGE_SIZE.to_string()), ("index", index.to_string())],
        )?
        .header("x-api-key", api_key);
        let body = read_json(request.send().map_err(network_error)?)?;
        let files = body.get("data").and_then(Value::as_array).ok_or_else(|| LauncherError::Unsupported("CurseForge returned an unexpected version response".to_owned()))?;
        let count = files.len();
        versions.extend(files.iter().filter_map(|file| parse_curseforge_version(file, project_id, "", "")));
        let total = body.pointer("pagination.totalCount").and_then(Value::as_u64).unwrap_or(0);
        if count < PAGE_SIZE as usize || (total > 0 && u64::from(index) + count as u64 >= total) {
            break;
        }
    }
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
    let provider_versions = string_list(value.get("gameVersions"));
    let compatible = provider_versions.iter().any(|version| version == game_version);
    if !game_version.is_empty() && !compatible {
        return None;
    }
    let loaders = if loader.is_empty() {
        provider_versions.iter().filter_map(|version| curseforge_loader_name(version).map(str::to_owned)).collect::<Vec<_>>()
    } else {
        vec![loader.to_owned()]
    };
    if !loader.is_empty() && !loaders.iter().any(|available| available == loader) {
        return None;
    }
    let compatible_versions = provider_versions.into_iter().filter(|version| curseforge_loader_name(version).is_none()).collect();
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
        loaders,
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

fn curseforge_loader_name(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "fabric" => Some("fabric"),
        "forge" => Some("forge"),
        "quilt" => Some("quilt"),
        "neoforge" => Some("neoforge"),
        _ => None,
    }
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

fn alternate_mod_state_name(file_name: &str) -> Option<String> {
    let lower_name = file_name.to_ascii_lowercase();
    if lower_name.ends_with(".jar.disabled") {
        Some(file_name[..file_name.len() - ".disabled".len()].to_owned())
    } else if lower_name.ends_with(".jar") {
        Some(format!("{file_name}.disabled"))
    } else {
        None
    }
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

    #[test]
    fn compatible_verified_dependencies_are_reused_but_conflicts_are_not() {
        let mut dependency = InstallFile {
            project_id: "sodium".to_owned(),
            project_title: "Sodium".to_owned(),
            version_id: "selected-version".to_owned(),
            file_name: "sodium-fabric.jar".to_owned(),
            url: "https://cdn.modrinth.com/data/sodium.jar".to_owned(),
            sha1: Some("a".repeat(40)),
            size: Some(10),
            version_pinned: false,
        };
        let installed_source = ModInstallSource {
            file_name: "sodium-fabric-old.jar".to_owned(),
            provider: "modrinth".to_owned(),
            project_id: "sodium".to_owned(),
            version_id: Some("compatible-old-version".to_owned()),
        };

        assert!(can_reuse_dependency(&dependency, &installed_source, "Sodium", true, true, true).unwrap());
        assert!(can_reuse_dependency(&dependency, &installed_source, "Sodium", true, false, true).is_err());
        assert!(can_reuse_dependency(&dependency, &installed_source, "Sodium", true, true, false).is_err());
        assert!(can_reuse_dependency(&dependency, &installed_source, "Sodium", false, true, true).is_err());

        dependency.version_pinned = true;
        assert!(can_reuse_dependency(&dependency, &installed_source, "Sodium", true, true, true).is_err());
        dependency.version_id = "compatible-old-version".to_owned();
        assert!(can_reuse_dependency(&dependency, &installed_source, "Sodium", true, true, true).unwrap());
    }

    #[test]
    fn reused_dependency_filter_keeps_the_requested_mod() {
        let install_file = |project_id: &str| InstallFile {
            project_id: project_id.to_owned(),
            project_title: project_id.to_owned(),
            version_id: "version".to_owned(),
            file_name: format!("{project_id}.jar"),
            url: format!("https://cdn.modrinth.com/data/{project_id}.jar"),
            sha1: Some("a".repeat(40)),
            size: Some(10),
            version_pinned: false,
        };
        let files = vec![install_file("iris"), install_file("sodium")];
        let reused = BTreeSet::from(["sodium".to_owned()]);

        let pending = filter_reused_dependencies(files, "iris", &reused);

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].project_id, "iris");
    }

    #[test]
    fn disabled_filename_artifact_verification_strips_state_suffix() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("opnlauncher-mod-dependency-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("sodium-fabric.jar.disabled");
        let bytes = b"verified sodium artifact";
        fs::write(&path, bytes).unwrap();
        let artifact = ModArtifactIntegrity {
            file_name: "sodium-fabric.jar".to_owned(),
            sha1: Some(format!("{:x}", Sha1::digest(bytes))),
            size: Some(bytes.len() as u64),
        };

        assert!(artifact_matches_file(&artifact, &path, "sodium-fabric.jar.disabled").unwrap());
        fs::write(&path, b"altered sodium artifact").unwrap();
        assert!(!artifact_matches_file(&artifact, &path, "sodium-fabric.jar.disabled").unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn switching_to_a_new_filename_removes_the_previous_root_file() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("opnlauncher-mod-switch-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let old_path = root.join("old-sodium.jar");
        let new_path = root.join("sodium-fabric-new.jar");
        fs::write(&old_path, b"old mod bytes").unwrap();
        let new_bytes = b"verified replacement bytes";
        fs::write(&new_path, new_bytes).unwrap();
        let sha1 = format!("{:x}", Sha1::digest(new_bytes));
        let files = vec![InstallFile {
            project_id: "sodium".to_owned(),
            project_title: "Sodium".to_owned(),
            version_id: "new-version".to_owned(),
            file_name: "sodium-fabric-new.jar".to_owned(),
            url: "https://cdn.modrinth.com/data/sodium.jar".to_owned(),
            sha1: Some(sha1),
            size: Some(new_bytes.len() as u64),
            version_pinned: false,
        }];
        let cancelled = Arc::new(AtomicBool::new(false));
        let progress: ModProgressSink = Arc::new(|_| {});

        stage_and_install(files, &root, &Uuid::new_v4().to_string(), &cancelled, progress, Some(("old-sodium.jar", "sodium"))).unwrap();

        assert!(!old_path.exists());
        assert_eq!(fs::read(&new_path).unwrap(), new_bytes);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_enabled_and_disabled_mod_filename_pairs() {
        assert_eq!(alternate_mod_state_name("example.jar").as_deref(), Some("example.jar.disabled"));
        assert_eq!(alternate_mod_state_name("example.jar.disabled").as_deref(), Some("example.jar"));
    }

    #[test]
    fn batch_removal_deletes_only_the_confirmed_selected_mod_files() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("opnlauncher-mod-delete-{}", Uuid::new_v4()));
        let game_dir = root.join("game");
        let mods_dir = game_dir.join("mods");
        fs::create_dir_all(&mods_dir).unwrap();
        let selected = ["selected-one.jar", "selected-two.jar"];
        for file_name in selected.iter().chain(["keep.jar", "notes.txt"].iter()) {
            fs::write(mods_dir.join(file_name), b"test file").unwrap();
        }
        let database = Database::in_memory().unwrap();
        let instance = database.create_instance("Removal test", game_dir.to_str().unwrap(), None).unwrap();

        assert!(remove_mod_files(&database, &instance.id, &[selected[0].to_owned()], false).is_err());
        assert!(mods_dir.join(selected[0]).exists());
        remove_mod_files(&database, &instance.id, &selected.map(str::to_owned), true).unwrap();

        assert!(!mods_dir.join(selected[0]).exists());
        assert!(!mods_dir.join(selected[1]).exists());
        assert!(mods_dir.join("keep.jar").exists());
        assert!(mods_dir.join("notes.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
