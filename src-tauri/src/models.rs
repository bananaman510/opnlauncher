use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstanceEngine {
    Minecraft,
    Horizon,
}

impl InstanceEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minecraft => "minecraft",
            Self::Horizon => "horizon",
        }
    }

    pub(crate) fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "minecraft" => Ok(Self::Minecraft),
            "horizon" => Ok(Self::Horizon),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                8,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown instance engine {value}"),
                )),
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersion {
    pub id: String,
    pub kind: String,
    pub version_json: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftInstallation {
    pub root: String,
    pub versions: Vec<MinecraftVersion>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub engine: InstanceEngine,
    pub game_dir: String,
    pub version_id: Option<String>,
    pub java_path: Option<String>,
    /// Optional isolated Java/Fabric runtime used by a Horizon profile. This runs the
    /// Java Minecraft client for Fabric compatibility; it is not code executed by Rust Horizon.
    pub horizon_fabric_enabled: bool,
    pub horizon_fabric_version_id: Option<String>,
    pub horizon_fabric_java_path: Option<String>,
    pub memory_mb: u32,
    pub created_at: i64,
    pub last_played_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlaytimeSession {
    pub id: String,
    pub instance_id: String,
    pub started_at: i64,
    pub last_updated_at: i64,
    pub ended_at: Option<i64>,
    pub duration_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlaytimeSummary {
    pub lifetime_seconds: u64,
    pub sessions: Vec<PlaytimeSession>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineProfile {
    pub id: String,
    pub name: String,
    pub uuid: String,
    pub verified: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftAccount {
    pub id: String,
    pub name: String,
    pub uuid: String,
    pub created_at: i64,
}

/// Runtime identity for a Java launch. Access tokens stay in backend memory and
/// are never serialized to the frontend or written to launcher logs.
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchProfile {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    pub xuid: String,
    pub verified: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntime {
    pub path: String,
    pub version: String,
    pub major_version: u32,
    pub architecture: Option<String>,
    pub vendor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchStarted {
    pub pid: u32,
    pub log_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchLogEvent {
    pub instance_id: String,
    pub stream: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchExitEvent {
    pub instance_id: String,
    pub success: bool,
    pub code: Option<i32>,
    pub log_path: String,
}
