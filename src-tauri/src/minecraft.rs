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
        let home = base.home_dir();
        candidates.push(base.home_dir().join(".minecraft"));
        #[cfg(target_os = "linux")]
        {
            candidates.push(home.join(".var/app/com.mojang.Minecraft/.minecraft"));
            candidates.push(home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher"));
            candidates.push(home.join(".var/app/org.polymc.PolyMC/data/PolyMC"));
            candidates.push(home.join(".var/app/org.multimc.MultiMC/data/MultiMC"));
        }
        #[cfg(target_os = "macos")]
        {
            candidates.push(home.join("Library/Application Support/minecraft"));
            candidates.push(home.join("Library/Application Support/PrismLauncher"));
            candidates.push(home.join("Library/Application Support/PolyMC"));
            candidates.push(home.join("Library/Application Support/MultiMC"));
            candidates.push(home.join("Library/Application Support/ATLauncher"));
        }
        #[cfg(target_os = "windows")]
        if let Some(app_data) = std::env::var_os("APPDATA") {
            candidates.push(PathBuf::from(app_data).join(".minecraft"));
            candidates.push(PathBuf::from(app_data).join("PrismLauncher"));
            candidates.push(PathBuf::from(app_data).join("PolyMC"));
            candidates.push(PathBuf::from(app_data).join("MultiMC"));
            candidates.push(PathBuf::from(app_data).join("ATLauncher"));
        }

        // Multi-instance launchers keep the game files in a known subfolder
        // or at the instance folder itself.
        for launcher_root in known_instance_roots(&base) {
            candidates.extend(instance_game_directories(&launcher_root));
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
    found.sort_by(|left, right| left.root.cmp(&right.root));
    found
}

fn known_instance_roots(base: &BaseDirs) -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    let home = base.home_dir();
    let mut roots = Vec::new();
    #[cfg(target_os = "linux")]
    roots.extend([
        base.data_local_dir().join("PrismLauncher/instances"),
        base.data_local_dir().join("PolyMC/instances"),
        base.data_local_dir().join("MultiMC/instances"),
        base.data_local_dir().join("ATLauncher/instances"),
        base.data_local_dir().join("gdlauncher_carbon/data/instances"),
        base.data_local_dir().join("gdlauncher_next/instances"),
        home.join(".atlauncher/instances"),
        home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances"),
        home.join(".var/app/org.polymc.PolyMC/data/PolyMC/instances"),
        home.join(".var/app/org.multimc.MultiMC/data/MultiMC/instances"),
        home.join(".var/app/com.atlauncher.ATLauncher/data/ATLauncher/instances"),
    ]);
    #[cfg(target_os = "macos")]
    roots.extend([
        base.data_local_dir().join("PrismLauncher/instances"),
        base.data_local_dir().join("PolyMC/instances"),
        base.data_local_dir().join("MultiMC/instances"),
        base.data_local_dir().join("ATLauncher/instances"),
        base.data_local_dir().join("gdlauncher_carbon/data/instances"),
    ]);
    #[cfg(target_os = "windows")]
    if let Some(app_data) = std::env::var_os("APPDATA") {
        let app_data = PathBuf::from(app_data);
        roots.extend([
            app_data.join("PrismLauncher/instances"),
            app_data.join("PolyMC/instances"),
            app_data.join("MultiMC/instances"),
            app_data.join("ATLauncher/instances"),
            app_data.join("gdlauncher_carbon/data/instances"),
            app_data.join("gdlauncher_next/instances"),
        ]);
    }
    roots.sort();
    roots.dedup();
    roots
}

fn instance_game_directories(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        if !file_type.is_dir() {
            continue;
        }
        let instance = entry.path();
        candidates.push(instance.join(".minecraft"));
        candidates.push(instance.join("minecraft"));
        candidates.push(instance.join("instance"));
        candidates.push(instance);
    }
    candidates
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
    let device_name = id
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let reserved_device_name = matches!(
        device_name.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "COM1" | "COM2" | "COM3" | "COM4" | "COM5"
            | "COM6" | "COM7" | "COM8" | "COM9" | "LPT1" | "LPT2" | "LPT3" | "LPT4"
            | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
    );

    !id.is_empty()
        && id.len() <= 128
        && id != "."
        && id != ".."
        && !id.ends_with(' ')
        && !id.ends_with('.')
        && !reserved_device_name
        && !id.chars().any(|character| {
            character.is_control()
                || matches!(character, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
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
        assert!(!is_safe_id("CON"));
        assert!(!is_safe_id("COM1.txt"));
        assert!(!is_safe_id("drive:relative"));
        assert!(!is_safe_id("trailing."));
        assert!(is_safe_id("fabric-loader-1.21.4"));
        assert!(is_safe_id("3D Shareware v1.34"));
        assert!(is_safe_id("20w14∞"));
    }

    #[test]
    fn sorts_release_versions_numerically_before_snapshots() {
        assert_eq!(compare_version_ids("1.21.1", "1.9.4"), std::cmp::Ordering::Greater);
        assert!(version_kind_rank("release") < version_kind_rank("snapshot"));
    }

    #[test]
    fn finds_game_folders_inside_known_launcher_instances() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target")
            .join(format!("launcher-scan-test-{}", uuid::Uuid::new_v4()));
        let instance = root.join("Survival Pack");
        std::fs::create_dir_all(instance.join(".minecraft/versions/26.3")).unwrap();
        std::fs::create_dir_all(root.join("Not an instance")).unwrap();

        let candidates = instance_game_directories(&root);
        assert!(candidates.contains(&instance.join(".minecraft")));
        assert!(candidates.contains(&instance.join("minecraft")));
        assert!(candidates.contains(&instance));
        std::fs::remove_dir_all(root).unwrap();
    }
}
