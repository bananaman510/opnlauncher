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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HorizonToolDiscovery {
    pub clients: Vec<String>,
    pub preparers: Vec<String>,
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
    pub memory_mb: u32,
    pub created_at: i64,
    pub last_played_at: Option<i64>,
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
