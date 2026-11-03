use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use regex::Regex;
use serde_json::{Map, Value};
use zip::ZipArchive;

use crate::error::{LauncherError, Result};
use crate::java;
use crate::minecraft::is_safe_id;
use crate::models::{Instance, OfflineProfile};

#[derive(Debug)]
pub struct LaunchPlan {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
}

pub struct LaunchRequest<'a> {
    pub instance: &'a Instance,
    pub profile: &'a OfflineProfile,
    pub java_path: &'a Path,
    pub native_cache: &'a Path,
    pub launcher_version: &'a str,
}

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

    let version_jar_id = metadata
        .get("jar")
        .and_then(Value::as_str)
        .filter(|id| is_safe_id(id))
        .unwrap_or(version_id);
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
        .join(version_id);
    recreate_cache_directory(&native_dir)?;
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
    for native in native_jars {
        extract_native_archive(&native, &native_dir)?;
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
    substitutions.insert("auth_access_token".to_owned(), Value::String(String::new()));
    substitutions.insert("auth_session".to_owned(), Value::String(String::new()));
    substitutions.insert("user_properties".to_owned(), Value::String("{}".to_owned()));
    substitutions.insert("user_type".to_owned(), Value::String("legacy".to_owned()));
    substitutions.insert("version_type".to_owned(), Value::String(
        metadata.get("type").and_then(Value::as_str).unwrap_or("custom").to_owned(),
    ));
    substitutions.insert("natives_directory".to_owned(), Value::String(native_dir.to_string_lossy().into_owned()));
    substitutions.insert("launcher_name".to_owned(), Value::String("OpnLauncher".to_owned()));
    substitutions.insert("launcher_version".to_owned(), Value::String(request.launcher_version.to_owned()));
    substitutions.insert("classpath".to_owned(), Value::String(class_path));
    substitutions.insert("classpath_separator".to_owned(), Value::String(if cfg!(windows) { ";" } else { ":" }.to_owned()));
    substitutions.insert("library_directory".to_owned(), Value::String(game_dir.join("libraries").to_string_lossy().into_owned()));
    substitutions.insert("auth_xuid".to_owned(), Value::String(String::new()));
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

    Ok(LaunchPlan {
        executable: request.java_path.to_path_buf(),
        arguments,
        working_directory: game_dir,
    })
}

fn resolve_version_metadata(game_dir: &Path, selected_id: &str) -> Result<Value> {
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
        let data = fs::read(&path).map_err(|error| {
            LauncherError::Unsupported(format!("cannot read version metadata {}: {error}", path.display()))
        })?;
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
    let mut resolved = chain.pop().expect("version chain contains selected metadata");
    while let Some(child) = chain.pop() {
        resolved = merge_metadata(resolved, child);
    }
    Ok(resolved)
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
    let artifact = library.get("downloads").and_then(|d| d.get("artifact"));
    if let Some(path) = artifact.and_then(|item| item.get("path")).and_then(Value::as_str) {
        let relative = safe_relative_path(path)?;
        let absolute = game_dir.join("libraries").join(relative);
        if !absolute.is_file() {
            return Err(LauncherError::NotFound(format!("library {}", absolute.display())));
        }
        return Ok(Some(absolute));
    }
    let Some(coordinate) = library.get("name").and_then(Value::as_str) else {
        return Ok(None);
    };
    let relative = maven_path(coordinate)?;
    let absolute = game_dir.join("libraries").join(relative);
    if !absolute.is_file() {
        return Err(LauncherError::NotFound(format!("library {}", absolute.display())));
    }
    Ok(Some(absolute))
}

fn native_artifact_path(library: &Value, game_dir: &Path) -> Result<Option<PathBuf>> {
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
    let absolute = game_dir.join("libraries").join(path);
    if !absolute.is_file() {
        return Err(LauncherError::NotFound(format!("native library {}", absolute.display())));
    }
    Ok(Some(absolute))
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

fn extract_native_archive(archive_path: &Path, destination: &Path) -> Result<()> {
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        LauncherError::Unsupported(format!("invalid native archive {}: {error}", archive_path.display()))
    })?;
    for index in 0..archive.len() {
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
        let output = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(output)?;
        } else {
            let parent = output.parent().ok_or_else(|| {
                LauncherError::Unsupported("invalid native archive output path".to_owned())
            })?;
            fs::create_dir_all(parent)?;
            let mut file = File::create(output)?;
            std::io::copy(&mut entry, &mut file)?;
            file.flush()?;
        }
    }
    Ok(())
}

fn recreate_cache_directory(path: &Path) -> Result<()> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(LauncherError::Unsupported("native cache path is not a safe directory".to_owned()));
        }
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
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
