use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde_json::Value;

use crate::error::{LauncherError, Result};
use crate::models::{MinecraftInstallation, MinecraftVersion};

pub fn discover_installations() -> Vec<MinecraftInstallation> {
    let mut candidates = Vec::new();
    if let Some(custom) = std::env::var_os("MINECRAFT_DIR") {
        candidates.push(PathBuf::from(custom));
    }
    if let Some(base) = BaseDirs::new() {
        candidates.push(base.home_dir().join(".minecraft"));
        #[cfg(target_os = "linux")]
        candidates.push(
            base.home_dir()
                .join(".var/app/com.mojang.Minecraft/.minecraft"),
        );
        #[cfg(target_os = "macos")]
        candidates.push(
            base.home_dir()
                .join("Library/Application Support/minecraft"),
        );
        #[cfg(target_os = "windows")]
        if let Some(app_data) = std::env::var_os("APPDATA") {
            candidates.push(PathBuf::from(app_data).join(".minecraft"));
        }
    }

    let mut found = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for candidate in candidates {
        let Ok(root) = std::fs::canonicalize(candidate) else {
            continue;
        };
        if !seen.insert(root.clone()) {
            continue;
        }
        if let Ok(installation) = inspect_installation(&root) {
            found.push(installation);
        }
    }
    found
}

pub fn inspect_installation(path: &Path) -> Result<MinecraftInstallation> {
    let root = std::fs::canonicalize(path).map_err(|error| {
        LauncherError::Unsupported(format!(
            "Minecraft directory {} is unavailable: {error}",
            path.display()
        ))
    })?;
    if !root.is_dir() {
        return Err(LauncherError::InvalidInput(
            "Minecraft location must be a directory".to_owned(),
        ));
    }
    let versions_dir = root.join("versions");
    if !versions_dir.is_dir() {
        return Err(LauncherError::Unsupported(format!(
            "{} does not contain a versions directory",
            root.display()
        )));
    }

    let mut versions = Vec::new();
    for entry in std::fs::read_dir(versions_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if !is_safe_id(&id) {
            continue;
        }
        let json_path = entry.path().join(format!("{id}.json"));
        if !json_path.is_file() {
            continue;
        }
        let Ok(metadata) = std::fs::read(&json_path).and_then(|bytes| {
            serde_json::from_slice::<Value>(&bytes)
                .map_err(std::io::Error::other)
        }) else {
            continue;
        };
        if metadata.get("id").and_then(Value::as_str) != Some(id.as_str()) {
            continue;
        }
        versions.push(MinecraftVersion {
            id,
            kind: metadata
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("custom")
                .to_owned(),
            version_json: json_path.to_string_lossy().into_owned(),
        });
    }
    versions.sort_by(|left, right| {
        version_kind_rank(&left.kind)
            .cmp(&version_kind_rank(&right.kind))
            .then_with(|| compare_version_ids(&right.id, &left.id))
    });
    if versions.is_empty() {
        return Err(LauncherError::Unsupported(format!(
            "{} has no readable Minecraft version metadata",
            root.display()
        )));
    }
    Ok(MinecraftInstallation {
        root: root.to_string_lossy().into_owned(),
        versions,
    })
}

pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && id != "."
        && id != ".."
}

fn version_kind_rank(kind: &str) -> u8 {
    match kind {
        "release" => 0,
        "snapshot" => 1,
        _ => 2,
    }
}

fn compare_version_ids(left: &str, right: &str) -> std::cmp::Ordering {
    let left_parts = left.split('.').collect::<Vec<_>>();
    let right_parts = right.split('.').collect::<Vec<_>>();
    for (left_part, right_part) in left_parts.iter().zip(&right_parts) {
        let ordering = match (left_part.parse::<u64>(), right_part.parse::<u64>()) {
            (Ok(left_number), Ok(right_number)) => left_number.cmp(&right_number),
            _ => left_part.cmp(right_part),
        };
        if !ordering.is_eq() {
            return ordering;
        }
    }
    left_parts.len().cmp(&right_parts.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn installation_scan_only_accepts_matching_version_manifests() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("opnlauncher-scan-{unique}"));
        let version = root.join("versions/1.20.4");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(
            version.join("1.20.4.json"),
            r#"{"id":"1.20.4","type":"release"}"#,
        )
        .unwrap();
        let mismatch = root.join("versions/wrong");
        std::fs::create_dir_all(&mismatch).unwrap();
        std::fs::write(mismatch.join("wrong.json"), r#"{"id":"elsewhere"}"#).unwrap();

        let installation = inspect_installation(&root).unwrap();
        assert_eq!(installation.versions.len(), 1);
        assert_eq!(installation.versions[0].id, "1.20.4");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_path_like_version_ids() {
        assert!(!is_safe_id("../version"));
        assert!(!is_safe_id(""));
        assert!(is_safe_id("fabric-loader-1.21.4"));
    }

    #[test]
    fn sorts_release_versions_numerically_before_snapshots() {
        assert_eq!(compare_version_ids("1.21.1", "1.9.4"), std::cmp::Ordering::Greater);
        assert!(version_kind_rank("release") < version_kind_rank("snapshot"));
    }
}
