use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{LauncherError, Result};
use crate::models::JavaRuntime;

pub fn discover_runtimes() -> Vec<JavaRuntime> {
    let mut candidates = Vec::new();
    if let Some(home) = env::var_os("JAVA_HOME") {
        candidates.push(PathBuf::from(home).join("bin").join(java_binary_name()));
    }

    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            candidates.push(directory.join(java_binary_name()));
        }
    }

    candidates.extend([
        PathBuf::from("/usr/bin/java"),
        PathBuf::from("/usr/local/bin/java"),
    ]);

    #[cfg(target_os = "linux")]
    for root in ["/usr/lib/jvm", "/opt"] {
        if let Ok(entries) = std::fs::read_dir(root) {
            candidates.extend(entries.flatten().map(|entry| {
                entry.path().join("bin").join(java_binary_name())
            }));
        }
    }

    let mut seen = BTreeSet::new();
    candidates
        .into_iter()
        .filter_map(|candidate| {
            let canonical = std::fs::canonicalize(&candidate).ok()?;
            if !seen.insert(canonical.clone()) || !canonical.is_file() {
                return None;
            }
            inspect_runtime(&canonical).ok()
        })
        .collect()
}

pub fn inspect_runtime(path: &Path) -> Result<JavaRuntime> {
    let output = Command::new(path)
        .args(["-XshowSettings:properties", "-version"])
        .output()
        .map_err(|error| {
            LauncherError::Unsupported(format!("could not run Java at {}: {error}", path.display()))
        })?;
    let report = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        return Err(LauncherError::Unsupported(format!(
            "Java at {} exited with {}",
            path.display(),
            output.status
        )));
    }
    let version = parse_version(&report).ok_or_else(|| {
        LauncherError::Unsupported(format!("could not read Java version at {}", path.display()))
    })?;
    let major_version = parse_major_version(&version).ok_or_else(|| {
        LauncherError::Unsupported(format!("could not parse Java version {version}"))
    })?;
    Ok(JavaRuntime {
        path: path.to_string_lossy().into_owned(),
        version,
        major_version,
        architecture: property(&report, "os.arch"),
        vendor: property(&report, "java.vendor"),
    })
}

fn java_binary_name() -> &'static str {
    if cfg!(windows) { "java.exe" } else { "java" }
}

fn property(report: &str, name: &str) -> Option<String> {
    report.lines().find_map(|line| {
        let (key, value) = line.trim().split_once('=')?;
        (key.trim() == name).then(|| value.trim().to_owned())
    })
}

fn parse_version(report: &str) -> Option<String> {
    let line = report.lines().find(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("openjdk version")
            || trimmed.starts_with("java version")
            || trimmed.starts_with("openjdk ")
            || trimmed.starts_with("java ")
    })?;
    if let Some((_, rest)) = line.split_once('"') {
        return rest.split('"').next().map(str::to_owned);
    }
    line.split_whitespace().nth(1).map(str::to_owned)
}

fn parse_major_version(version: &str) -> Option<u32> {
    let first = version.split(['.', '-', '+', '_']).next()?;
    let numeric = first.parse::<u32>().ok()?;
    if numeric == 1 {
        version.split('.').nth(1)?.parse().ok()
    } else {
        Some(numeric)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_and_legacy_java_versions() {
        assert_eq!(parse_major_version("27-ea"), Some(27));
        assert_eq!(parse_major_version("21.0.3+9"), Some(21));
        assert_eq!(parse_major_version("1.8.0_412"), Some(8));
        assert_eq!(parse_major_version("unknown"), None);
    }

    #[test]
    fn reads_openjdk_version_output() {
        let report = "openjdk version \"21.0.3\" 2024-04-16\n";
        assert_eq!(parse_version(report).as_deref(), Some("21.0.3"));
    }
}
