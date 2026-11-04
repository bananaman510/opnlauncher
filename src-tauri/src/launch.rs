use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use regex::Regex;
use serde_json::{Map, Value};
use zip::ZipArchive;

use crate::error::{LauncherError, Result};
use crate::java;
use crate::minecraft::is_safe_id;
use crate::models::{Instance, LaunchProfile};

pub struct LaunchPlan {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    /// Native libraries are private to each launch so concurrent processes never
    /// replace files that another process is using.
    pub native_directory: Option<PathBuf>,
}

pub struct LaunchRequest<'a> {
    pub instance: &'a Instance,
    pub profile: &'a LaunchProfile,
    pub java_path: &'a Path,
    pub native_cache: &'a Path,
    pub launcher_version: &'a str,
    pub prepare_natives: bool,
}

const MAX_NATIVE_ARCHIVE_ENTRIES: usize = 100_000;
const MAX_NATIVE_ENTRY_BYTES: u64 = 512 * 1024 * 1024;
const MAX_NATIVE_EXPANDED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ASSET_INDEX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ASSET_INDEX_OBJECTS: usize = 100_000;

#[derive(Clone, Copy)]
struct NativeArchiveLimits {
    entries: usize,
    entry_bytes: u64,
    total_bytes: u64,
}

const NATIVE_ARCHIVE_LIMITS: NativeArchiveLimits = NativeArchiveLimits {
    entries: MAX_NATIVE_ARCHIVE_ENTRIES,
    entry_bytes: MAX_NATIVE_ENTRY_BYTES,
    total_bytes: MAX_NATIVE_EXPANDED_BYTES,
};

pub fn build_launch_plan(request: LaunchRequest<'_>) -> Result<LaunchPlan> {
    let game_dir = PathBuf::from(&request.instance.game_dir);
    if !game_dir.is_dir() {
        return Err(LauncherError::NotFound(format!(
            "game directory {}",
            game_dir.display()
        )));
    }
    let version_id = request
        .instance
        .version_id
        .as_deref()
        .ok_or_else(|| LauncherError::InvalidInput("select a Minecraft version first".to_owned()))?;
    if !is_safe_id(version_id) {
        return Err(LauncherError::InvalidInput(
            "version identifier contains unsafe path characters".to_owned(),
        ));
    }
    if !is_safe_id(&request.instance.id) {
        return Err(LauncherError::InvalidInput(
            "instance identifier is invalid".to_owned(),
        ));
    }
    crate::database::validate_offline_name(&request.profile.name)?;
    uuid::Uuid::parse_str(&request.profile.uuid).map_err(|_| {
        LauncherError::InvalidInput("offline profile UUID is invalid".to_owned())
    })?;

    let metadata = resolve_version_metadata(&game_dir, version_id)?;
    let java_runtime = java::inspect_runtime(request.java_path)?;
    let required_java = required_java_major(&metadata, version_id);
    if java_runtime.major_version < required_java {
        return Err(LauncherError::Unsupported(format!(
            "Minecraft {version_id} needs Java {required_java} or newer; {} is Java {}",
            request.java_path.display(),
            java_runtime.major_version
        )));
    }

    let assets_dir = game_dir.join("assets");
    let asset_index = metadata
        .get("assetIndex")
        .and_then(|index| index.get("id"))
        .and_then(Value::as_str)
        .or_else(|| metadata.get("assets").and_then(Value::as_str))
        .ok_or_else(|| LauncherError::Unsupported("version has no asset index id".to_owned()))?;
    if !is_safe_id(asset_index) {
        return Err(LauncherError::Unsupported(
            "version asset index identifier is invalid".to_owned(),
        ));
    }
    let asset_index_path = assets_dir.join("indexes").join(format!("{asset_index}.json"));
    if !asset_index_path.is_file() {
        return Err(LauncherError::NotFound(format!(
            "asset index {}",
            asset_index_path.display()
        )));
    }
    let game_assets_id = metadata
        .get("assets")
        .and_then(Value::as_str)
        .filter(|id| is_safe_id(id))
        .unwrap_or(asset_index);
    let legacy_assets_dir = assets_dir.join("virtual").join(game_assets_id);
    let game_assets_dir = if legacy_assets_dir.is_dir() {
        legacy_assets_dir
    } else {
        assets_dir.clone()
    };

    let base_jar_id = base_version_id(&game_dir, version_id)?;
    let version_jar_id = metadata
        .get("jar")
        .and_then(Value::as_str)
        .filter(|id| is_safe_id(id))
        .unwrap_or(&base_jar_id);
    let game_jar = game_dir
        .join("versions")
        .join(version_jar_id)
        .join(format!("{version_jar_id}.jar"));
    if !game_jar.is_file() {
        return Err(LauncherError::NotFound(format!(
            "Minecraft client JAR {}",
            game_jar.display()
        )));
    }

    let native_dir = request
        .native_cache
        .join(&request.instance.id)
        .join(version_id)
        .join(uuid::Uuid::new_v4().to_string());
    let mut class_path = Vec::new();
    let mut native_jars = Vec::new();
    for library in metadata
        .get("libraries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !rules_allow(library.get("rules")) {
            continue;
        }
        if let Some(path) = library_artifact_path(library, &game_dir)? {
            class_path.push(path);
        }
        if let Some(path) = native_artifact_path(library, &game_dir)? {
            native_jars.push(path);
        }
    }
    class_path.push(game_jar);
    let class_path = class_path
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(if cfg!(windows) { ";" } else { ":" });

    let mut substitutions = Map::new();
    substitutions.insert("auth_player_name".to_owned(), Value::String(request.profile.name.clone()));
    substitutions.insert("version_name".to_owned(), Value::String(version_id.to_owned()));
    substitutions.insert("game_directory".to_owned(), Value::String(game_dir.to_string_lossy().into_owned()));
    substitutions.insert("assets_root".to_owned(), Value::String(assets_dir.to_string_lossy().into_owned()));
    substitutions.insert("game_assets".to_owned(), Value::String(game_assets_dir.to_string_lossy().into_owned()));
    substitutions.insert("assets_index_name".to_owned(), Value::String(asset_index.to_owned()));
    substitutions.insert("auth_uuid".to_owned(), Value::String(request.profile.uuid.clone()));
    substitutions.insert("auth_access_token".to_owned(), Value::String(request.profile.access_token.clone()));
    substitutions.insert("auth_session".to_owned(), Value::String(String::new()));
    substitutions.insert("user_properties".to_owned(), Value::String("{}".to_owned()));
    substitutions.insert("user_type".to_owned(), Value::String(request.profile.user_type.clone()));
    substitutions.insert("version_type".to_owned(), Value::String(
        metadata.get("type").and_then(Value::as_str).unwrap_or("custom").to_owned(),
    ));
    substitutions.insert("natives_directory".to_owned(), Value::String(native_dir.to_string_lossy().into_owned()));
    substitutions.insert("launcher_name".to_owned(), Value::String("OpnLauncher".to_owned()));
    substitutions.insert("launcher_version".to_owned(), Value::String(request.launcher_version.to_owned()));
    substitutions.insert("classpath".to_owned(), Value::String(class_path));
    substitutions.insert("classpath_separator".to_owned(), Value::String(if cfg!(windows) { ";" } else { ":" }.to_owned()));
    substitutions.insert("library_directory".to_owned(), Value::String(game_dir.join("libraries").to_string_lossy().into_owned()));
    substitutions.insert("auth_xuid".to_owned(), Value::String(request.profile.xuid.clone()));
    substitutions.insert("clientid".to_owned(), Value::String(String::new()));
    substitutions.insert("resolution_width".to_owned(), Value::String("1280".to_owned()));
    substitutions.insert("resolution_height".to_owned(), Value::String("720".to_owned()));
    if let Some(path) = logging_config_path(&metadata, &game_dir) {
        substitutions.insert("path".to_owned(), Value::String(path.to_string_lossy().into_owned()));
    }

    let mut arguments = Vec::new();
    if let Some(jvm_args) = metadata
        .get("arguments")
        .and_then(|args| args.get("jvm"))
        .and_then(Value::as_array)
    {
        arguments.extend(expand_argument_nodes(jvm_args, &substitutions)?);
    }
    let memory_flag = format!("-Xmx{}m", request.instance.memory_mb);
    if !arguments.iter().any(|argument| argument.starts_with("-Xmx")) {
        arguments.push(memory_flag);
    }
    if !arguments.iter().any(|argument| argument.starts_with("-Xms")) {
        arguments.push("-Xms512m".to_owned());
    }
    arguments.push("-Djava.library.path=${natives_directory}".replace(
        "${natives_directory}",
        substitutions["natives_directory"].as_str().unwrap_or_default(),
    ));
    arguments.extend(["-cp".to_owned(), substitutions["classpath"].as_str().unwrap_or_default().to_owned()]);

    let main_class = metadata
        .get("mainClass")
        .and_then(Value::as_str)
        .ok_or_else(|| LauncherError::Unsupported("version metadata has no mainClass".to_owned()))?;
    arguments.push(main_class.to_owned());
    if let Some(game_args) = metadata
        .get("arguments")
        .and_then(|args| args.get("game"))
        .and_then(Value::as_array)
    {
        arguments.extend(expand_argument_nodes(game_args, &substitutions)?);
    } else if let Some(legacy) = metadata.get("minecraftArguments").and_then(Value::as_str) {
        let tokens = shlex::split(legacy).ok_or_else(|| {
            LauncherError::Unsupported("legacy game arguments contain invalid quoting".to_owned())
        })?;
        arguments.extend(tokens.iter().map(|token| substitute(token, &substitutions)));
    } else {
        return Err(LauncherError::Unsupported(
            "version metadata contains no game arguments".to_owned(),
        ));
    }
    if let Some(unresolved) = arguments.iter().find(|argument| argument.contains("${")) {
        return Err(LauncherError::Unsupported(format!(
            "version metadata uses an unsupported argument placeholder in {unresolved}"
        )));
    }

    let native_directory = if request.prepare_natives {
        extract_native_archives(&native_jars, &native_dir)?;
        Some(native_dir)
    } else {
        None
    };

    Ok(LaunchPlan {
        executable: request.java_path.to_path_buf(),
        arguments,
        working_directory: game_dir,
        native_directory,
    })
}

pub(crate) fn resolve_version_metadata(game_dir: &Path, selected_id: &str) -> Result<Value> {
    let (chain, missing) = read_version_metadata_chain(game_dir, selected_id, false)?;
    debug_assert!(missing.is_none());
    merge_version_metadata(chain)
}

/// Return the base Minecraft version inherited by a vanilla or loader profile.
/// If a parent metadata file is missing, its identifier is still known from the
/// child's `inheritsFrom` field and can be repaired from Mojang's manifest.
pub(crate) fn base_version_id(game_dir: &Path, selected_id: &str) -> Result<String> {
    let (chain, missing) = read_version_metadata_chain(game_dir, selected_id, true)?;
    if let Some(path) = missing {
        return path
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| is_safe_id(value))
            .map(str::to_owned)
            .ok_or_else(|| LauncherError::Unsupported("missing version metadata has an invalid identifier".to_owned()));
    }
    chain
        .last()
        .and_then(|metadata| metadata.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| LauncherError::Unsupported("version metadata chain is empty".to_owned()))
}

/// List missing files required by a selected Java Minecraft profile.
/// The result includes official asset objects and loader libraries, not only the
/// files checked while building the Java command line.
pub(crate) fn missing_minecraft_files(game_dir: &Path, selected_id: &str) -> Result<Vec<PathBuf>> {
    let (chain, missing_metadata) = read_version_metadata_chain(game_dir, selected_id, true)?;
    if let Some(path) = missing_metadata {
        let relative = path.strip_prefix(game_dir).map_err(|_| {
            LauncherError::Unsupported("missing version metadata escapes the instance folder".to_owned())
        })?;
        return Ok(vec![relative.to_path_buf()]);
    }
    let metadata = merge_version_metadata(chain)?;
    let mut missing = Vec::new();

    let base_jar_id = base_version_id(game_dir, selected_id)?;
    let jar_id = metadata
        .get("jar")
        .and_then(Value::as_str)
        .filter(|id| is_safe_id(id))
        .unwrap_or(&base_jar_id);
    add_missing_file(
        game_dir,
        PathBuf::from("versions").join(jar_id).join(format!("{jar_id}.jar")),
        &mut missing,
    )?;

    for library in metadata
        .get("libraries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !rules_allow(library.get("rules")) {
            continue;
        }
        if let Some(path) = library_artifact_candidate(library, game_dir)? {
            add_missing_absolute_file(game_dir, path, &mut missing)?;
        }
        if let Some(path) = native_artifact_candidate(library, game_dir)? {
            add_missing_absolute_file(game_dir, path, &mut missing)?;
        }
    }

    let asset_id = metadata
        .get("assetIndex")
        .and_then(|index| index.get("id"))
        .and_then(Value::as_str)
        .or_else(|| metadata.get("assets").and_then(Value::as_str))
        .ok_or_else(|| LauncherError::Unsupported("version has no asset index id".to_owned()))?;
    if !is_safe_id(asset_id) {
        return Err(LauncherError::Unsupported("version asset index identifier is invalid".to_owned()));
    }
    let index_relative = PathBuf::from("assets/indexes").join(format!("{asset_id}.json"));
    let missing_before_index = missing.len();
    add_missing_file(game_dir, index_relative.clone(), &mut missing)?;
    if missing.len() != missing_before_index {
        return Ok(missing);
    }
    let index_path = game_dir.join(&index_relative);
    match fs::symlink_metadata(&index_path)? {
        file if file.len() > MAX_ASSET_INDEX_BYTES => {
            return Err(LauncherError::Unsupported("asset index exceeds its size limit".to_owned()));
        }
        _ => {}
    }
    let index_bytes = fs::read(&index_path)?;
    let index: Value = serde_json::from_slice(&index_bytes)?;
    let objects = index
        .get("objects")
        .and_then(Value::as_object)
        .ok_or_else(|| LauncherError::Unsupported("asset index has no object map".to_owned()))?;
    if objects.len() > MAX_ASSET_INDEX_OBJECTS {
        return Err(LauncherError::Unsupported("asset index contains too many objects".to_owned()));
    }
    let is_virtual = index.get("virtual").and_then(Value::as_bool) == Some(true);
    let virtual_id = metadata
        .get("assets")
        .and_then(Value::as_str)
        .filter(|id| is_safe_id(id))
        .unwrap_or(asset_id);
    for (logical_name, object) in objects {
        let hash = object
            .get("hash")
            .and_then(Value::as_str)
            .filter(|hash| hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or_else(|| LauncherError::Unsupported("asset index contains an invalid object hash".to_owned()))?
            .to_ascii_lowercase();
        let object_relative = PathBuf::from("assets/objects").join(&hash[..2]).join(&hash);
        add_missing_file(game_dir, object_relative.clone(), &mut missing)?;
        if is_virtual {
            let logical = safe_relative_path(logical_name)?;
            add_missing_file(
                game_dir,
                PathBuf::from("assets/virtual").join(virtual_id).join(logical),
                &mut missing,
            )?;
        }
    }

    if let Some(file) = metadata
        .get("logging")
        .and_then(|logging| logging.get("client"))
        .and_then(|client| client.get("file"))
    {
        if let Some(id) = file.get("id").and_then(Value::as_str) {
            if !is_safe_id(id) {
                return Err(LauncherError::Unsupported("logging configuration identifier is invalid".to_owned()));
            }
            add_missing_file(game_dir, PathBuf::from("assets/log_configs").join(id), &mut missing)?;
        }
    }
    Ok(missing)
}

fn read_version_metadata_chain(
    game_dir: &Path,
    selected_id: &str,
    allow_missing: bool,
) -> Result<(Vec<Value>, Option<PathBuf>)> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut current = selected_id.to_owned();
    loop {
        if !is_safe_id(&current) || !seen.insert(current.clone()) {
            return Err(LauncherError::Unsupported(
                "version inheritance is cyclic or contains an invalid identifier".to_owned(),
            ));
        }
        let path = game_dir
            .join("versions")
            .join(&current)
            .join(format!("{current}.json"));
        let data = match fs::symlink_metadata(&path) {
            Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((chain, Some(path)));
            }
            Err(error) => return Err(LauncherError::Unsupported(format!("cannot read version metadata {}: {error}", path.display()))),
            Ok(file) if file.file_type().is_symlink() || !file.is_file() => {
                return Err(LauncherError::Unsupported(format!("version metadata {} is not a regular file", path.display())));
            }
            Ok(_) => fs::read(&path)?,
        };
        let metadata: Value = serde_json::from_slice(&data)?;
        if metadata.get("id").and_then(Value::as_str) != Some(current.as_str()) {
            return Err(LauncherError::Unsupported(format!(
                "version metadata id does not match {current}"
            )));
        }
        let parent = metadata.get("inheritsFrom").and_then(Value::as_str).map(str::to_owned);
        chain.push(metadata);
        if let Some(parent_id) = parent {
            current = parent_id;
        } else {
            break;
        }
    }
    Ok((chain, None))
}

fn merge_version_metadata(mut chain: Vec<Value>) -> Result<Value> {
    let mut resolved = chain.pop().ok_or_else(|| LauncherError::Unsupported("version metadata chain is empty".to_owned()))?;
    while let Some(child) = chain.pop() {
        resolved = merge_metadata(resolved, child);
    }
    Ok(resolved)
}

fn add_missing_file(game_dir: &Path, relative: PathBuf, missing: &mut Vec<PathBuf>) -> Result<()> {
    let relative = safe_relative_path(&relative.to_string_lossy())?;
    add_missing_absolute_file(game_dir, game_dir.join(relative), missing)
}

fn add_missing_absolute_file(game_dir: &Path, path: PathBuf, missing: &mut Vec<PathBuf>) -> Result<()> {
    let relative = path.strip_prefix(game_dir).map_err(|_| {
        LauncherError::Unsupported("required game file escapes the instance folder".to_owned())
    })?;
    let relative = safe_relative_path(&relative.to_string_lossy())?;
    let components = relative.components().collect::<Vec<_>>();
    let mut current = game_dir.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(part) = component else {
            return Err(LauncherError::Unsupported("required game file has an unsafe path".to_owned()));
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                push_missing_path(relative, missing);
                return Ok(());
            }
            Err(error) => return Err(error.into()),
            Ok(metadata) if metadata.file_type().is_symlink() => {
                push_missing_path(relative, missing);
                return Ok(());
            }
            Ok(metadata) if index + 1 < components.len() && !metadata.is_dir() => {
                push_missing_path(relative, missing);
                return Ok(());
            }
            Ok(metadata) if index + 1 == components.len() && !metadata.is_file() => {
                push_missing_path(relative, missing);
                return Ok(());
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

fn push_missing_path(relative: PathBuf, missing: &mut Vec<PathBuf>) {
    if !missing.contains(&relative) {
        missing.push(relative);
    }
}

fn merge_metadata(mut parent: Value, child: Value) -> Value {
    let (Some(parent_object), Some(child_object)) = (parent.as_object_mut(), child.as_object()) else {
        return child;
    };
    for (key, value) in child_object {
        match key.as_str() {
            "arguments" => {
                let parent_arguments = parent_object.entry(key.clone()).or_insert_with(|| Value::Object(Map::new()));
                if let (Some(parent_arguments), Some(child_arguments)) =
                    (parent_arguments.as_object_mut(), value.as_object())
                {
                    for name in ["jvm", "game"] {
                        if let Some(child_values) = child_arguments.get(name).and_then(Value::as_array) {
                            let mut combined = parent_arguments
                                .get(name)
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default();
                            combined.extend(child_values.iter().cloned());
                            parent_arguments.insert(name.to_owned(), Value::Array(combined));
                        }
                    }
                }
            }
            "libraries" => {
                let mut libraries = parent_object
                    .get("libraries")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if let Some(child_libraries) = value.as_array() {
                    for child_library in child_libraries {
                        let child_name = child_library.get("name").and_then(Value::as_str);
                        if let Some(index) = libraries.iter().position(|library| {
                            library.get("name").and_then(Value::as_str) == child_name && child_name.is_some()
                        }) {
                            libraries[index] = child_library.clone();
                        } else {
                            libraries.push(child_library.clone());
                        }
                    }
                }
                parent_object.insert(key.clone(), Value::Array(libraries));
            }
            "inheritsFrom" => {}
            _ => {
                parent_object.insert(key.clone(), value.clone());
            }
        }
    }
    parent
}

fn required_java_major(metadata: &Value, version_id: &str) -> u32 {
    if let Some(major) = metadata
        .get("javaVersion")
        .and_then(|version| version.get("majorVersion"))
        .and_then(Value::as_u64)
        .and_then(|major| u32::try_from(major).ok())
    {
        return major;
    }
    let numeric = version_id
        .split('-')
        .next()
        .unwrap_or(version_id)
        .split('.')
        .collect::<Vec<_>>();
    let minor = numeric.get(1).and_then(|part| part.parse::<u32>().ok()).unwrap_or_default();
    let patch = numeric.get(2).and_then(|part| part.parse::<u32>().ok()).unwrap_or_default();
    if minor > 20 || (minor == 20 && patch >= 5) {
        21
    } else if minor >= 18 {
        17
    } else if minor == 17 {
        16
    } else {
        8
    }
}

pub(crate) fn rules_allow(rules: Option<&Value>) -> bool {
    let Some(rules) = rules else { return true };
    let Some(rules) = rules.as_array() else { return false };
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule) {
            allowed = rule.get("action").and_then(Value::as_str) == Some("allow");
        }
    }
    allowed
}

fn rule_matches(rule: &Value) -> bool {
    if let Some(os) = rule.get("os") {
        let current_name = if cfg!(target_os = "macos") { "osx" } else { std::env::consts::OS };
        if os.get("name").and_then(Value::as_str).is_some_and(|name| name != current_name) {
            return false;
        }
        if let Some(arch) = os.get("arch").and_then(Value::as_str) {
            let current = std::env::consts::ARCH;
            if arch == "x86" && current != "x86" && current != "i686" {
                return false;
            }
            if arch == "arm64" && current != "aarch64" && current != "arm64" {
                return false;
            }
            if arch != "x86" && arch != "arm64" && arch != current {
                return false;
            }
        }
        if let Some(version) = os.get("version").and_then(Value::as_str) {
            let Some(current_version) = system_os_version() else { return false };
            if !wildcard_matches(version, &current_version) {
                return false;
            }
        }
    }
    if let Some(features) = rule.get("features").and_then(Value::as_object) {
        if features.values().any(|expected| expected.as_bool() != Some(false)) {
            return false;
        }
    }
    true
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    Regex::new(pattern).is_ok_and(|regex| regex.is_match(value))
}

fn system_os_version() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        return fs::read_to_string("/proc/sys/kernel/osrelease")
            .ok()
            .map(|version| version.trim().to_owned());
    }
    #[cfg(target_os = "macos")]
    {
        return std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }
    #[cfg(target_os = "windows")]
    {
        return std::env::var("OS_VERSION").ok();
    }
}

fn library_artifact_path(library: &Value, game_dir: &Path) -> Result<Option<PathBuf>> {
    let path = library_artifact_candidate(library, game_dir)?;
    if let Some(path) = path.as_ref() {
        if !path.is_file() {
            return Err(LauncherError::NotFound(format!("library {}", path.display())));
        }
    }
    Ok(path)
}

fn library_artifact_candidate(library: &Value, game_dir: &Path) -> Result<Option<PathBuf>> {
    let artifact = library.get("downloads").and_then(|d| d.get("artifact"));
    if let Some(path) = artifact.and_then(|item| item.get("path")).and_then(Value::as_str) {
        let relative = safe_relative_path(path)?;
        let absolute = game_dir.join("libraries").join(relative);
        return Ok(Some(absolute));
    }
    let Some(coordinate) = library.get("name").and_then(Value::as_str) else {
        return Ok(None);
    };
    let relative = maven_path(coordinate)?;
    let absolute = game_dir.join("libraries").join(relative);
    Ok(Some(absolute))
}

fn native_artifact_path(library: &Value, game_dir: &Path) -> Result<Option<PathBuf>> {
    let path = native_artifact_candidate(library, game_dir)?;
    if let Some(path) = path.as_ref() {
        if !path.is_file() {
            return Err(LauncherError::NotFound(format!("native library {}", path.display())));
        }
    }
    Ok(path)
}

fn native_artifact_candidate(library: &Value, game_dir: &Path) -> Result<Option<PathBuf>> {
    let os = if cfg!(target_os = "macos") { "osx" } else { std::env::consts::OS };
    let Some(template) = library
        .get("natives")
        .and_then(|natives| natives.get(os))
        .and_then(Value::as_str)
    else { return Ok(None) };
    let arch = if cfg!(target_pointer_width = "64") { "64" } else { "32" };
    let classifier = template.replace("${arch}", arch);
    let path = library
        .get("downloads")
        .and_then(|downloads| downloads.get("classifiers"))
        .and_then(|classifiers| classifiers.get(&classifier))
        .and_then(|artifact| artifact.get("path"))
        .and_then(Value::as_str)
        .map(safe_relative_path)
        .transpose()?;
    let Some(path) = path else { return Ok(None) };
    Ok(Some(game_dir.join("libraries").join(path)))
}

pub(crate) fn maven_path(coordinate: &str) -> Result<PathBuf> {
    let mut coordinate_parts = coordinate.split('@');
    let without_extension = coordinate_parts.next().unwrap_or(coordinate);
    let extension = coordinate_parts.next().unwrap_or("jar");
    let parts = without_extension.split(':').collect::<Vec<_>>();
    if parts.len() < 3 || parts.iter().any(|part| part.is_empty()) {
        return Err(LauncherError::Unsupported(format!("invalid library coordinate {coordinate}")));
    }
    let mut path = PathBuf::new();
    for segment in parts[0].split('.') {
        path.push(segment);
    }
    path.push(parts[1]);
    path.push(parts[2]);
    let classifier = parts.get(3).map(|classifier| format!("-{classifier}")).unwrap_or_default();
    path.push(format!("{}-{}{}.{}", parts[1], parts[2], classifier, extension));
    safe_relative_path(&path.to_string_lossy())
}

fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(LauncherError::Unsupported(format!("unsafe relative path {value}")));
    }
    Ok(path.to_path_buf())
}

fn extract_native_archives(archives: &[PathBuf], destination: &Path) -> Result<PathBuf> {
    extract_native_archives_with_limits(archives, destination, NATIVE_ARCHIVE_LIMITS)
}

fn extract_native_archives_with_limits(
    archives: &[PathBuf],
    destination: &Path,
    limits: NativeArchiveLimits,
) -> Result<PathBuf> {
    let parent = destination.parent().ok_or_else(|| {
        LauncherError::Unsupported("native cache path has no parent directory".to_owned())
    })?;
    fs::create_dir_all(parent)?;
    fs::create_dir(destination)?;
    let mut seen_files = HashSet::new();
    let mut entry_count = 0usize;
    let mut expanded = 0u64;
    let result = archives.iter().try_for_each(|archive| {
        extract_native_archive(
            archive,
            destination,
            &mut seen_files,
            &mut entry_count,
            &mut expanded,
            limits,
        )
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result?;
    Ok(destination.to_path_buf())
}

fn extract_native_archive(
    archive_path: &Path,
    destination: &Path,
    seen_files: &mut HashSet<PathBuf>,
    entry_count: &mut usize,
    expanded: &mut u64,
    limits: NativeArchiveLimits,
) -> Result<()> {
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        LauncherError::Unsupported(format!("invalid native archive {}: {error}", archive_path.display()))
    })?;
    for index in 0..archive.len() {
        *entry_count = entry_count.checked_add(1).ok_or_else(|| {
            LauncherError::Unsupported("native archive entry count overflowed".to_owned())
        })?;
        if *entry_count > limits.entries {
            return Err(LauncherError::Unsupported(
                "native libraries have too many archive entries".to_owned(),
            ));
        }
        let mut entry = archive.by_index(index).map_err(|error| {
            LauncherError::Unsupported(format!("cannot read native archive entry: {error}"))
        })?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(LauncherError::Unsupported("native archive contains an unsafe path".to_owned()));
        };
        if entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err(LauncherError::Unsupported("native archive contains a symbolic link".to_owned()));
        }
        if relative.components().any(|component| !matches!(component, Component::Normal(_))) {
            return Err(LauncherError::Unsupported("native archive contains an unsafe path".to_owned()));
        }
        if relative.starts_with("META-INF") {
            continue;
        }
        if entry.is_dir() {
            fs::create_dir_all(destination.join(relative))?;
            continue;
        }
        if !seen_files.insert(relative.clone()) {
            return Err(LauncherError::Unsupported(format!(
                "native archives contain a duplicate output path: {}",
                relative.display()
            )));
        }
        if entry.size() > limits.entry_bytes {
            return Err(LauncherError::Unsupported(format!(
                "native archive entry exceeds the {} MiB limit",
                limits.entry_bytes / (1024 * 1024)
            )));
        }
        let output = destination.join(relative);
        let parent = output.parent().ok_or_else(|| {
            LauncherError::Unsupported("invalid native archive output path".to_owned())
        })?;
        fs::create_dir_all(parent)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        let mut limited = (&mut entry).take(limits.entry_bytes + 1);
        let written = std::io::copy(&mut limited, &mut file)?;
        if written > limits.entry_bytes {
            return Err(LauncherError::Unsupported(
                "native archive entry expanded beyond its allowed size".to_owned(),
            ));
        }
        *expanded = expanded.checked_add(written).ok_or_else(|| {
            LauncherError::Unsupported("native archive expanded size overflowed".to_owned())
        })?;
        if *expanded > limits.total_bytes {
            return Err(LauncherError::Unsupported(
                "native archives exceed the per-launch expanded-size limit".to_owned(),
            ));
        }
        file.flush()?;
    }
    Ok(())
}

fn expand_argument_nodes(nodes: &[Value], substitutions: &Map<String, Value>) -> Result<Vec<String>> {
    let mut arguments = Vec::new();
    for node in nodes {
        if let Some(value) = node.as_str() {
            arguments.push(substitute(value, substitutions));
        } else if let Some(object) = node.as_object() {
            if !rules_allow(object.get("rules")) {
                continue;
            }
            let Some(value) = object.get("value") else { continue };
            match value {
                Value::String(value) => arguments.push(substitute(value, substitutions)),
                Value::Array(values) => {
                    for value in values {
                        let Some(value) = value.as_str() else {
                            return Err(LauncherError::Unsupported(
                                "version argument value array contains a non-string".to_owned(),
                            ));
                        };
                        arguments.push(substitute(value, substitutions));
                    }
                }
                _ => return Err(LauncherError::Unsupported("version argument value has an unsupported type".to_owned())),
            }
        } else {
            return Err(LauncherError::Unsupported("version argument has an unsupported type".to_owned()));
        }
    }
    Ok(arguments)
}

fn substitute(argument: &str, substitutions: &Map<String, Value>) -> String {
    let mut expanded = argument.to_owned();
    for (key, value) in substitutions {
        if let Some(value) = value.as_str() {
            expanded = expanded.replace(&format!("${{{key}}}"), value);
        }
    }
    expanded
}

fn logging_config_path(metadata: &Value, game_dir: &Path) -> Option<PathBuf> {
    let id = metadata
        .get("logging")?
        .get("client")?
        .get("file")?
        .get("id")?
        .as_str()?;
    if !is_safe_id(id) {
        return None;
    }
    let path = game_dir.join("assets/log_configs").join(id);
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn native_archive(root: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = root.join(name);
        let mut archive = ZipWriter::new(File::create(&path).unwrap());
        for (entry, bytes) in entries {
            archive
                .start_file(*entry, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
        path
    }

    fn test_root() -> PathBuf {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-launch-repair")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_profile_fixture(root: &Path) {
        let base = root.join("versions/1.20.1");
        let fabric = root.join("versions/fabric-loader-0.15.0-1.20.1");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&fabric).unwrap();
        fs::write(
            base.join("1.20.1.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": "1.20.1",
                "assetIndex": { "id": "5" },
                "assets": "5",
                "libraries": []
            })).unwrap(),
        ).unwrap();
        fs::write(
            fabric.join("fabric-loader-0.15.0-1.20.1.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": "fabric-loader-0.15.0-1.20.1",
                "inheritsFrom": "1.20.1",
                "libraries": [{
                    "name": "net.fabricmc:fabric-loader:0.15.0",
                    "downloads": { "artifact": { "path": "net/fabricmc/fabric-loader/0.15.0/fabric-loader-0.15.0.jar" } }
                }]
            })).unwrap(),
        ).unwrap();
        let index = root.join("assets/indexes/5.json");
        fs::create_dir_all(index.parent().unwrap()).unwrap();
        fs::write(index, br#"{"objects":{"lang/en_us.json":{"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":1}}}"#).unwrap();
    }

    #[test]
    fn finds_missing_client_asset_and_loader_files_for_inherited_profile() {
        let root = test_root();
        write_profile_fixture(&root);
        let selected = "fabric-loader-0.15.0-1.20.1";
        assert_eq!(base_version_id(&root, selected).unwrap(), "1.20.1");
        let missing = missing_minecraft_files(&root, selected).unwrap();
        assert!(missing.contains(&PathBuf::from("versions/1.20.1/1.20.1.jar")));
        assert!(missing.contains(&PathBuf::from("libraries/net/fabricmc/fabric-loader/0.15.0/fabric-loader-0.15.0.jar")));
        assert!(missing.contains(&PathBuf::from("assets/objects/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")));

        for relative in missing {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"x").unwrap();
        }
        assert!(missing_minecraft_files(&root, selected).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_a_missing_asset_index_for_repair() {
        let root = test_root();
        write_profile_fixture(&root);
        fs::remove_file(root.join("assets/indexes/5.json")).unwrap();
        let missing = missing_minecraft_files(&root, "fabric-loader-0.15.0-1.20.1").unwrap();
        assert!(missing.contains(&PathBuf::from("assets/indexes/5.json")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn identifies_missing_inherited_metadata_and_keeps_its_base_version_id() {
        let root = test_root();
        write_profile_fixture(&root);
        fs::remove_file(root.join("versions/1.20.1/1.20.1.json")).unwrap();
        let selected = "fabric-loader-0.15.0-1.20.1";
        assert_eq!(base_version_id(&root, selected).unwrap(), "1.20.1");
        assert_eq!(
            missing_minecraft_files(&root, selected).unwrap(),
            vec![PathBuf::from("versions/1.20.1/1.20.1.json")]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn metadata_merge_inherits_arguments_and_replaces_duplicate_libraries() {
        let parent: Value = serde_json::from_str(
            r#"{"mainClass":"Parent","arguments":{"jvm":["-Done"],"game":["parent"]},"libraries":[{"name":"a"},{"name":"b"}]}"#,
        )
        .unwrap();
        let child: Value = serde_json::from_str(
            r#"{"mainClass":"Child","arguments":{"game":["child"]},"libraries":[{"name":"b","version":2}] }"#,
        )
        .unwrap();
        let resolved = merge_metadata(parent, child);
        assert_eq!(resolved["mainClass"], "Child");
        assert_eq!(resolved["arguments"]["jvm"][0], "-Done");
        assert_eq!(resolved["arguments"]["game"].as_array().unwrap().len(), 2);
        assert_eq!(resolved["libraries"].as_array().unwrap().len(), 2);
        assert_eq!(resolved["libraries"][1]["version"], 2);
    }

    #[test]
    fn filters_version_rules_and_expands_placeholders() {
        let rules: Value = serde_json::from_str(
            r#"[{"action":"allow","os":{"name":"linux"}},{"action":"disallow","os":{"name":"windows"}}]"#,
        )
        .unwrap();
        assert!(rules_allow(Some(&rules)));
        let mut substitutions = Map::new();
        substitutions.insert("name".to_owned(), Value::String("LocalPlayer".to_owned()));
        assert_eq!(substitute("--username ${name}", &substitutions), "--username LocalPlayer");
    }

    #[test]
    fn rejects_unsafe_maven_and_archive_paths() {
        assert!(maven_path("org.example:game:1.0").is_ok());
        assert!(safe_relative_path("../../escape.jar").is_err());
        assert!(safe_relative_path("/tmp/absolute.jar").is_err());
    }

    #[test]
    fn native_archive_expansion_limits_remove_partial_cache_output() {
        let root = test_root();
        let archive = native_archive(&root, "native.zip", &[("lib/example.so", b"12345")]);
        let destination = root.join("cache");
        let limits = NativeArchiveLimits {
            entries: 10,
            entry_bytes: 8,
            total_bytes: 4,
        };
        let error = extract_native_archives_with_limits(&[archive], &destination, limits)
            .unwrap_err();
        assert!(error.to_string().contains("expanded-size limit"));
        assert!(!destination.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_archives_reject_duplicate_output_paths() {
        let root = test_root();
        let first = native_archive(&root, "first.zip", &[("lib/example.so", b"one")]);
        let second = native_archive(&root, "second.zip", &[("lib/example.so", b"two")]);
        let destination = root.join("cache");
        let error = extract_native_archives_with_limits(
            &[first, second],
            &destination,
            NATIVE_ARCHIVE_LIMITS,
        )
        .unwrap_err();
        assert!(error.to_string().contains("duplicate output path"));
        assert!(!destination.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_archives_allow_shared_directory_entries() {
        let root = test_root();
        let mut archives = Vec::new();
        for name in ["first.zip", "second.zip"] {
            let path = root.join(name);
            let mut archive = ZipWriter::new(File::create(&path).unwrap());
            archive
                .add_directory("common/", SimpleFileOptions::default())
                .unwrap();
            archive
                .start_file(format!("common/{name}.so"), SimpleFileOptions::default())
                .unwrap();
            archive.write_all(name.as_bytes()).unwrap();
            archive.finish().unwrap();
            archives.push(path);
        }
        let destination = root.join("cache");
        extract_native_archives_with_limits(
            &archives,
            &destination,
            NATIVE_ARCHIVE_LIMITS,
        )
        .unwrap();
        assert!(destination.join("common/first.zip.so").is_file());
        assert!(destination.join("common/second.zip.so").is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn matches_upstream_os_version_regular_expressions() {
        assert!(wildcard_matches("^6\\.[0-9]+", "6.12.4"));
        assert!(!wildcard_matches("^5\\.", "6.12.4"));
        assert!(!wildcard_matches("[", "6.12.4"));
    }

    #[test]
    fn version_java_defaults_follow_minecraft_release_changes() {
        assert_eq!(required_java_major(&Value::Null, "1.17.1"), 16);
        assert_eq!(required_java_major(&Value::Null, "1.18.2"), 17);
        assert_eq!(required_java_major(&Value::Null, "1.20.4"), 17);
        assert_eq!(required_java_major(&Value::Null, "1.20.5"), 21);
        assert_eq!(required_java_major(&Value::Null, "1.21.1"), 21);
    }
}
