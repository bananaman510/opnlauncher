use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use md5::{Digest, Md5};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::{LauncherError, Result};
use crate::models::{
    Instance, InstanceEngine, MicrosoftAccount, OfflineProfile, PlaytimeSession, PlaytimeSummary,
};

pub struct Database {
    connection: Mutex<Connection>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInstallSource {
    pub file_name: String,
    pub provider: String,
    pub project_id: String,
    pub version_id: Option<String>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::from_connection(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS instances (
                 id TEXT PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL,
                 engine TEXT NOT NULL DEFAULT 'minecraft',
                 game_dir TEXT NOT NULL,
                 version_id TEXT,
                 java_path TEXT,
                 horizon_fabric_enabled INTEGER NOT NULL DEFAULT 0,
                 horizon_fabric_version_id TEXT,
                 horizon_fabric_java_path TEXT,
                 memory_mb INTEGER NOT NULL DEFAULT 2048 CHECK(memory_mb BETWEEN 512 AND 65536),
                 created_at INTEGER NOT NULL,
                 last_played_at INTEGER
             );
             CREATE TABLE IF NOT EXISTS offline_profiles (
                 id TEXT PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                 uuid TEXT NOT NULL UNIQUE,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS microsoft_accounts (
                 id TEXT PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL,
                 uuid TEXT NOT NULL UNIQUE,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS settings (
                 key TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS play_sessions (
                 id TEXT PRIMARY KEY NOT NULL,
                 instance_id TEXT NOT NULL,
                 started_at INTEGER NOT NULL,
                 last_updated_at INTEGER NOT NULL,
                 ended_at INTEGER,
                 duration_seconds INTEGER NOT NULL DEFAULT 0 CHECK(duration_seconds >= 0)
             );
             CREATE INDEX IF NOT EXISTS play_sessions_started_at_idx ON play_sessions(started_at);
             CREATE TABLE IF NOT EXISTS installed_mod_sources (
                 instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
                 file_name TEXT NOT NULL,
                 provider TEXT NOT NULL,
                 project_id TEXT NOT NULL,
                 version_id TEXT,
                 PRIMARY KEY (instance_id, file_name)
             );",
        )?;
        let columns = {
            let mut statement = connection.prepare("PRAGMA table_info(instances)")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        if !columns.iter().any(|column| column == "engine") {
            connection.execute(
                "ALTER TABLE instances ADD COLUMN engine TEXT NOT NULL DEFAULT 'minecraft'",
                [],
            )?;
        }
        if !columns.iter().any(|column| column == "horizon_fabric_enabled") {
            connection.execute(
                "ALTER TABLE instances ADD COLUMN horizon_fabric_enabled INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        if !columns.iter().any(|column| column == "horizon_fabric_version_id") {
            connection.execute(
                "ALTER TABLE instances ADD COLUMN horizon_fabric_version_id TEXT",
                [],
            )?;
        }
        if !columns.iter().any(|column| column == "horizon_fabric_java_path") {
            connection.execute(
                "ALTER TABLE instances ADD COLUMN horizon_fabric_java_path TEXT",
                [],
            )?;
        }
        connection.execute(
            "UPDATE play_sessions SET ended_at = last_updated_at WHERE ended_at IS NULL",
            [],
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn list_instances(&self) -> Result<Vec<Instance>> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, game_dir, version_id, java_path, memory_mb, created_at, last_played_at, engine,
                    horizon_fabric_enabled, horizon_fabric_version_id, horizon_fabric_java_path
             FROM instances ORDER BY created_at DESC, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Instance {
                id: row.get(0)?,
                name: row.get(1)?,
                engine: InstanceEngine::parse(&row.get::<_, String>(8)?)?,
                game_dir: row.get(2)?,
                version_id: row.get(3)?,
                java_path: row.get(4)?,
                memory_mb: row.get::<_, u32>(5)?,
                created_at: row.get(6)?,
                last_played_at: row.get(7)?,
                horizon_fabric_enabled: row.get::<_, i64>(9)? != 0,
                horizon_fabric_version_id: row.get(10)?,
                horizon_fabric_java_path: row.get(11)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_instance(&self, id: &str) -> Result<Instance> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection
            .query_row(
                "SELECT id, name, game_dir, version_id, java_path, memory_mb, created_at, last_played_at, engine,
                        horizon_fabric_enabled, horizon_fabric_version_id, horizon_fabric_java_path
                 FROM instances WHERE id = ?1",
                [id],
                |row| {
                    Ok(Instance {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        engine: InstanceEngine::parse(&row.get::<_, String>(8)?)?,
                        game_dir: row.get(2)?,
                        version_id: row.get(3)?,
                        java_path: row.get(4)?,
                        memory_mb: row.get::<_, u32>(5)?,
                        created_at: row.get(6)?,
                        last_played_at: row.get(7)?,
                        horizon_fabric_enabled: row.get::<_, i64>(9)? != 0,
                        horizon_fabric_version_id: row.get(10)?,
                        horizon_fabric_java_path: row.get(11)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| LauncherError::NotFound(format!("instance {id}")))
    }

    pub fn create_instance(
        &self,
        name: &str,
        game_dir: &str,
        version_id: Option<&str>,
    ) -> Result<Instance> {
        let id = Uuid::new_v4().to_string();
        self.create_instance_with_id(&id, name, game_dir, version_id)
    }

    pub fn create_instance_with_id(
        &self,
        id: &str,
        name: &str,
        game_dir: &str,
        version_id: Option<&str>,
    ) -> Result<Instance> {
        self.create_instance_with_engine(id, name, game_dir, version_id, InstanceEngine::Minecraft)
    }

    pub fn create_horizon_instance(&self, name: &str, game_dir: &str) -> Result<Instance> {
        let id = Uuid::new_v4().to_string();
        self.create_horizon_instance_with_id(&id, name, game_dir)
    }

    pub fn create_horizon_instance_with_id(
        &self,
        id: &str,
        name: &str,
        game_dir: &str,
    ) -> Result<Instance> {
        self.create_instance_with_engine(id, name, game_dir, Some("26.3"), InstanceEngine::Horizon)
    }

    fn create_instance_with_engine(
        &self,
        id: &str,
        name: &str,
        game_dir: &str,
        version_id: Option<&str>,
        engine: InstanceEngine,
    ) -> Result<Instance> {
        Uuid::parse_str(id).map_err(|_| {
            LauncherError::InvalidInput("instance identifier is invalid".to_owned())
        })?;
        let name = validate_instance_name(name)?;
        if game_dir.trim().is_empty() {
            return Err(LauncherError::InvalidInput(
                "a game directory is required".to_owned(),
            ));
        }
        let instance = Instance {
            id: id.to_owned(),
            name,
            engine,
            game_dir: game_dir.to_owned(),
            version_id: version_id.map(str::to_owned),
            java_path: None,
            horizon_fabric_enabled: false,
            horizon_fabric_version_id: None,
            horizon_fabric_java_path: None,
            memory_mb: 2048,
            created_at: unix_time(),
            last_played_at: None,
        };
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection.execute(
            "INSERT INTO instances (id, name, engine, game_dir, version_id, java_path, horizon_fabric_enabled,
                                    horizon_fabric_version_id, horizon_fabric_java_path, memory_mb, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                instance.id,
                instance.name,
                instance.engine.as_str(),
                instance.game_dir,
                instance.version_id,
                instance.java_path,
                instance.horizon_fabric_enabled,
                instance.horizon_fabric_version_id,
                instance.horizon_fabric_java_path,
                instance.memory_mb,
                instance.created_at,
            ],
        )?;
        Ok(instance)
    }

    pub fn rename_instance(&self, id: &str, name: &str) -> Result<()> {
        let name = validate_instance_name(name)?;
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let changed = connection.execute(
            "UPDATE instances SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        if changed == 0 {
            return Err(LauncherError::NotFound(format!("instance {id}")));
        }
        Ok(())
    }

    pub fn set_instance_java(&self, id: &str, java_path: Option<&str>) -> Result<()> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let changed = connection.execute(
            "UPDATE instances SET java_path = ?1 WHERE id = ?2",
            params![java_path, id],
        )?;
        if changed == 0 {
            return Err(LauncherError::NotFound(format!("instance {id}")));
        }
        Ok(())
    }

    pub fn set_horizon_fabric_runtime(
        &self,
        id: &str,
        version_id: Option<&str>,
        java_path: Option<&str>,
        enabled: bool,
    ) -> Result<()> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let changed = connection.execute(
            "UPDATE instances SET horizon_fabric_version_id = ?1, horizon_fabric_java_path = ?2,
                                  horizon_fabric_enabled = ?3
             WHERE id = ?4 AND engine = 'horizon'",
            params![version_id, java_path, enabled, id],
        )?;
        if changed == 0 {
            return Err(LauncherError::NotFound(format!("Horizon instance {id}")));
        }
        Ok(())
    }

    pub fn set_horizon_fabric_runtime_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let changed = connection.execute(
            "UPDATE instances SET horizon_fabric_enabled = ?1
             WHERE id = ?2 AND engine = 'horizon'
               AND (?1 = 0 OR (horizon_fabric_version_id IS NOT NULL AND horizon_fabric_java_path IS NOT NULL))",
            params![enabled, id],
        )?;
        if changed == 0 {
            return Err(LauncherError::InvalidInput(
                "install the isolated Fabric runtime before enabling it".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn set_instance_version(&self, id: &str, version_id: &str) -> Result<()> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let changed = connection.execute(
            "UPDATE instances SET version_id = ?1 WHERE id = ?2",
            params![version_id, id],
        )?;
        if changed == 0 {
            return Err(LauncherError::NotFound(format!("instance {id}")));
        }
        Ok(())
    }

    pub fn list_mod_sources(&self, instance_id: &str) -> Result<Vec<ModInstallSource>> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let mut statement = connection.prepare(
            "SELECT file_name, provider, project_id, version_id
             FROM installed_mod_sources WHERE instance_id = ?1",
        )?;
        let rows = statement.query_map([instance_id], |row| {
            Ok(ModInstallSource {
                file_name: row.get(0)?,
                provider: row.get(1)?,
                project_id: row.get(2)?,
                version_id: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_mod_source(&self, instance_id: &str, file_name: &str) -> Result<Option<ModInstallSource>> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection
            .query_row(
                "SELECT file_name, provider, project_id, version_id
                 FROM installed_mod_sources WHERE instance_id = ?1 AND file_name = ?2",
                params![instance_id, file_name],
                |row| {
                    Ok(ModInstallSource {
                        file_name: row.get(0)?,
                        provider: row.get(1)?,
                        project_id: row.get(2)?,
                        version_id: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn save_mod_sources(&self, instance_id: &str, sources: &[ModInstallSource]) -> Result<()> {
        let mut connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let transaction = connection.transaction()?;
        for source in sources {
            transaction.execute(
                "INSERT INTO installed_mod_sources (instance_id, file_name, provider, project_id, version_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(instance_id, file_name) DO UPDATE SET
                    provider = excluded.provider,
                    project_id = excluded.project_id,
                    version_id = excluded.version_id",
                params![instance_id, source.file_name, source.provider, source.project_id, source.version_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn replace_mod_sources(
        &self,
        instance_id: &str,
        old_file_name: &str,
        sources: &[ModInstallSource],
    ) -> Result<()> {
        let mut connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM installed_mod_sources WHERE instance_id = ?1 AND file_name = ?2",
            params![instance_id, old_file_name],
        )?;
        for source in sources {
            transaction.execute(
                "INSERT INTO installed_mod_sources (instance_id, file_name, provider, project_id, version_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(instance_id, file_name) DO UPDATE SET
                    provider = excluded.provider,
                    project_id = excluded.project_id,
                    version_id = excluded.version_id",
                params![instance_id, source.file_name, source.provider, source.project_id, source.version_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn rename_mod_source(&self, instance_id: &str, old_file_name: &str, new_file_name: &str) -> Result<()> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection.execute(
            "UPDATE installed_mod_sources SET file_name = ?1 WHERE instance_id = ?2 AND file_name = ?3",
            params![new_file_name, instance_id, old_file_name],
        )?;
        Ok(())
    }

    pub fn remove_mod_sources(&self, instance_id: &str, file_names: &[String]) -> Result<()> {
        let mut connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let transaction = connection.transaction()?;
        for file_name in file_names {
            transaction.execute(
                "DELETE FROM installed_mod_sources WHERE instance_id = ?1 AND file_name = ?2",
                params![instance_id, file_name],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn set_memory(&self, id: &str, memory_mb: u32) -> Result<()> {
        if !(512..=65536).contains(&memory_mb) {
            return Err(LauncherError::InvalidInput(
                "memory must be between 512 and 65536 MB".to_owned(),
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let changed = connection.execute(
            "UPDATE instances SET memory_mb = ?1 WHERE id = ?2",
            params![memory_mb, id],
        )?;
        if changed == 0 {
            return Err(LauncherError::NotFound(format!("instance {id}")));
        }
        Ok(())
    }

    pub fn delete_instance_record(&self, id: &str) -> Result<()> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let changed = connection.execute("DELETE FROM instances WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(LauncherError::NotFound(format!("instance {id}")));
        }
        Ok(())
    }

    pub fn touch_instance(&self, id: &str) -> Result<()> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let changed = connection.execute(
            "UPDATE instances SET last_played_at = ?1 WHERE id = ?2",
            params![unix_time(), id],
        )?;
        if changed == 0 {
            return Err(LauncherError::NotFound(format!("instance {id}")));
        }
        Ok(())
    }

    pub fn set_playtime_tracking_enabled(&self, enabled: bool) -> Result<()> {
        let mut connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO settings (key, value) VALUES ('playtime_tracking_enabled', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [if enabled { "true" } else { "false" }],
        )?;
        if !enabled {
            let now = unix_time();
            transaction.execute(
                "UPDATE play_sessions
                 SET last_updated_at = ?1, ended_at = ?1, duration_seconds = MAX(0, ?1 - started_at)
                 WHERE ended_at IS NULL",
                [now],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn start_play_session(&self, instance_id: &str) -> Result<Option<String>> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let enabled = connection
            .query_row(
                "SELECT value = 'true' FROM settings WHERE key = 'playtime_tracking_enabled'",
                [],
                |row| row.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false);
        if !enabled {
            return Ok(None);
        }
        let now = unix_time();
        let id = Uuid::new_v4().to_string();
        connection.execute(
            "INSERT INTO play_sessions (id, instance_id, started_at, last_updated_at, duration_seconds)
             VALUES (?1, ?2, ?3, ?3, 0)",
            params![id, instance_id, now],
        )?;
        Ok(Some(id))
    }

    pub fn update_play_session(&self, id: &str) -> Result<()> {
        let now = unix_time();
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection.execute(
            "UPDATE play_sessions
             SET last_updated_at = ?1, duration_seconds = MAX(0, ?1 - started_at)
             WHERE id = ?2 AND ended_at IS NULL",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn finish_play_session(&self, id: &str) -> Result<()> {
        let now = unix_time();
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection.execute(
            "UPDATE play_sessions
             SET last_updated_at = ?1, ended_at = ?1, duration_seconds = MAX(0, ?1 - started_at)
             WHERE id = ?2 AND ended_at IS NULL",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn playtime_summary(&self, start_at: i64, end_at: i64) -> Result<PlaytimeSummary> {
        if start_at >= end_at || end_at - start_at > 370 * 24 * 60 * 60 {
            return Err(LauncherError::InvalidInput(
                "playtime chart range must be between 1 second and 370 days".to_owned(),
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let lifetime_seconds = connection.query_row(
            "SELECT COALESCE(SUM(duration_seconds), 0) FROM play_sessions",
            [],
            |row| row.get::<_, i64>(0),
        )?.max(0) as u64;
        let mut statement = connection.prepare(
            "SELECT id, instance_id, started_at, last_updated_at, ended_at, duration_seconds
             FROM play_sessions
             WHERE started_at < ?2 AND COALESCE(ended_at, last_updated_at) >= ?1
             ORDER BY started_at",
        )?;
        let rows = statement.query_map(params![start_at, end_at], |row| {
            Ok(PlaytimeSession {
                id: row.get(0)?,
                instance_id: row.get(1)?,
                started_at: row.get(2)?,
                last_updated_at: row.get(3)?,
                ended_at: row.get(4)?,
                duration_seconds: row.get::<_, i64>(5)?.max(0) as u64,
            })
        })?;
        let sessions = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(PlaytimeSummary { lifetime_seconds, sessions })
    }

    pub fn create_offline_profile(&self, name: &str) -> Result<OfflineProfile> {
        let name = validate_offline_name(name)?;
        let profile = OfflineProfile {
            id: Uuid::new_v4().to_string(),
            uuid: offline_profile_uuid(&name),
            name,
            verified: false,
            created_at: unix_time(),
        };
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection.execute(
            "INSERT INTO offline_profiles (id, name, uuid, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![profile.id, profile.name, profile.uuid, profile.created_at],
        )?;
        Ok(profile)
    }

    pub fn list_offline_profiles(&self) -> Result<Vec<OfflineProfile>> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, uuid, created_at FROM offline_profiles ORDER BY created_at, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(OfflineProfile {
                id: row.get(0)?,
                name: row.get(1)?,
                uuid: row.get(2)?,
                verified: false,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_offline_profile(&self, id: &str) -> Result<OfflineProfile> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection
            .query_row(
                "SELECT id, name, uuid, created_at FROM offline_profiles WHERE id = ?1",
                [id],
                |row| {
                    Ok(OfflineProfile {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        uuid: row.get(2)?,
                        verified: false,
                        created_at: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| LauncherError::NotFound(format!("offline profile {id}")))
    }

    pub fn save_microsoft_account(&self, name: &str, uuid: &str) -> Result<MicrosoftAccount> {
        let name = validate_offline_name(name)?;
        let uuid = Uuid::parse_str(uuid)
            .map_err(|_| LauncherError::InvalidInput("Minecraft profile UUID is invalid".to_owned()))?
            .to_string();
        let account = MicrosoftAccount {
            id: format!("microsoft:{uuid}"),
            name,
            uuid,
            created_at: unix_time(),
        };
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection.execute(
            "INSERT INTO microsoft_accounts (id, name, uuid, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name",
            params![account.id, account.name, account.uuid, account.created_at],
        )?;
        Ok(account)
    }

    pub fn list_microsoft_accounts(&self) -> Result<Vec<MicrosoftAccount>> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, uuid, created_at FROM microsoft_accounts ORDER BY created_at, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(MicrosoftAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                uuid: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_microsoft_account(&self, id: &str) -> Result<MicrosoftAccount> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection
            .query_row(
                "SELECT id, name, uuid, created_at FROM microsoft_accounts WHERE id = ?1",
                [id],
                |row| {
                    Ok(MicrosoftAccount {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        uuid: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| LauncherError::NotFound(format!("Microsoft account {id}")))
    }

    pub fn remove_microsoft_account(&self, id: &str) -> Result<()> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let changed = connection.execute("DELETE FROM microsoft_accounts WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(LauncherError::NotFound(format!("Microsoft account {id}")));
        }
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0))
            .optional()
            .map_err(Into::into)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

fn offline_profile_uuid(name: &str) -> String {
    let identity = format!("OfflinePlayer:{name}");
    let mut digest = Md5::digest(identity.as_bytes());
    digest[6] = (digest[6] & 0x0f) | 0x30;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest);
    Uuid::from_bytes(bytes).to_string()
}

pub fn validate_instance_name(input: &str) -> Result<String> {
    let name = input.trim();
    if name.is_empty() || name.chars().count() > 48 || name.chars().any(char::is_control) {
        return Err(LauncherError::InvalidInput(
            "instance name must be 1 to 48 printable characters".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

pub fn validate_offline_name(input: &str) -> Result<String> {
    let name = input.trim();
    if !(1..=16).contains(&name.len())
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(LauncherError::InvalidInput(
            "offline test names must use 1 to 16 ASCII letters, numbers, or underscores".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

pub(crate) fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_and_renames_instances() {
        let db = Database::in_memory().unwrap();
        let created = db
            .create_instance("  Vanilla  ", "/tmp/game", Some("1.21.4"))
            .unwrap();
        assert_eq!(created.name, "Vanilla");
        db.rename_instance(&created.id, "Survival").unwrap();
        let stored = db.get_instance(&created.id).unwrap();
        assert_eq!(stored.name, "Survival");
        assert_eq!(stored.version_id.as_deref(), Some("1.21.4"));
    }

    #[test]
    fn switches_an_instance_to_an_installed_loader_version() {
        let db = Database::in_memory().unwrap();
        let instance = db.create_instance("Fabric test", "/tmp/game", Some("1.21.4")).unwrap();
        db.set_instance_version(&instance.id, "fabric-loader-0.16.10-1.21.4").unwrap();
        assert_eq!(db.get_instance(&instance.id).unwrap().version_id.as_deref(), Some("fabric-loader-0.16.10-1.21.4"));
    }

    #[test]
    fn creates_instance_with_a_reserved_valid_id() {
        let db = Database::in_memory().unwrap();
        let id = "1a8c91d6-2c84-49b6-a251-93e54684a45d";
        let created = db
            .create_instance_with_id(id, "Downloaded version", "/managed/instance", Some("1.21.4"))
            .unwrap();
        assert_eq!(created.id, id);
        assert_eq!(db.get_instance(id).unwrap().game_dir, "/managed/instance");
        assert!(db.create_instance_with_id("invalid-id", "Duplicate", "/other", None).is_err());
    }

    #[test]
    fn offline_profile_uuid_is_stable_and_unverified() {
        let first = Database::in_memory().unwrap();
        let profile = first.create_offline_profile("Test_Player").unwrap();
        let second = Database::in_memory().unwrap();
        let same_name = second.create_offline_profile("Test_Player").unwrap();
        assert_eq!(profile.uuid, "a33ac9aa-13a8-3009-a068-bb1380a2d0b0");
        assert_eq!(profile.uuid, same_name.uuid);
        assert!(!profile.verified);
    }

    #[test]
    fn microsoft_account_metadata_persists_without_credentials() {
        let db = Database::in_memory().unwrap();
        let account = db
            .save_microsoft_account("VerifiedPlayer", "123e4567-e89b-12d3-a456-426614174000")
            .unwrap();
        assert_eq!(account.id, "microsoft:123e4567-e89b-12d3-a456-426614174000");
        assert_eq!(db.list_microsoft_accounts().unwrap(), vec![account.clone()]);
        assert_eq!(db.get_microsoft_account(&account.id).unwrap(), account);
        db.remove_microsoft_account("microsoft:123e4567-e89b-12d3-a456-426614174000")
            .unwrap();
        assert!(db.list_microsoft_accounts().unwrap().is_empty());
    }

    #[test]
    fn playtime_is_opt_in_and_session_history_stays_local() {
        let db = Database::in_memory().unwrap();
        let instance = db.create_instance("Playtime test", "/managed/playtime", Some("1.21.4")).unwrap();

        assert_eq!(db.start_play_session(&instance.id).unwrap(), None);
        db.set_playtime_tracking_enabled(true).unwrap();
        let session_id = db.start_play_session(&instance.id).unwrap().unwrap();
        db.update_play_session(&session_id).unwrap();
        let before_disable = db.playtime_summary(unix_time() - 2, unix_time() + 2).unwrap();
        assert_eq!(before_disable.sessions.len(), 1);

        db.set_playtime_tracking_enabled(false).unwrap();
        let after_disable = db.playtime_summary(unix_time() - 2, unix_time() + 2).unwrap();
        assert_eq!(after_disable.sessions.len(), 1);
        assert!(after_disable.sessions[0].ended_at.is_some());
        assert_eq!(db.start_play_session(&instance.id).unwrap(), None);
    }

    #[test]
    fn playtime_chart_rejects_invalid_or_unbounded_ranges() {
        let db = Database::in_memory().unwrap();
        assert!(db.playtime_summary(10, 10).is_err());
        assert!(db.playtime_summary(0, 371 * 24 * 60 * 60).is_err());
    }

    #[test]
    fn interrupted_play_sessions_recover_at_the_last_checkpoint_and_survive_instance_removal() {
        let db = Database::in_memory().unwrap();
        let instance = db.create_instance("Recovered playtime", "/managed/recovered", Some("1.21.4")).unwrap();
        let started_at = unix_time() - 120;
        let checkpoint_at = started_at + 45;
        db.connection.lock().unwrap().execute(
            "INSERT INTO play_sessions (id, instance_id, started_at, last_updated_at, ended_at, duration_seconds)
             VALUES ('interrupted-session', ?1, ?2, ?3, NULL, 45)",
            params![instance.id, started_at, checkpoint_at],
        ).unwrap();
        let connection = db.connection.into_inner().unwrap();
        let db = Database::from_connection(connection).unwrap();

        let recovered = db.playtime_summary(started_at - 1, unix_time() + 1).unwrap();
        assert_eq!(recovered.sessions.len(), 1);
        assert_eq!(recovered.sessions[0].duration_seconds, 45);
        assert_eq!(recovered.sessions[0].ended_at, Some(checkpoint_at));

        db.delete_instance_record(&instance.id).unwrap();
        let retained = db.playtime_summary(started_at - 1, unix_time() + 1).unwrap();
        assert_eq!(retained.sessions.len(), 1);
        assert_eq!(retained.lifetime_seconds, 45);
    }

    #[test]
    fn creates_horizon_instance_as_a_separate_engine_at_26_3() {
        let db = Database::in_memory().unwrap();
        let instance = db.create_horizon_instance("Horizon test", "/managed/horizon").unwrap();
        assert_eq!(instance.engine, InstanceEngine::Horizon);
        assert_eq!(instance.version_id.as_deref(), Some("26.3"));
        assert_eq!(db.get_instance(&instance.id).unwrap(), instance);
    }

    #[test]
    fn stores_horizon_fabric_runtime_separately_from_the_rust_version() {
        let db = Database::in_memory().unwrap();
        let instance = db.create_horizon_instance("Horizon Fabric", "/managed/horizon").unwrap();
        db.set_horizon_fabric_runtime(
            &instance.id,
            Some("fabric-loader-0.20.1-26.3"),
            Some("/java-25/bin/java"),
            true,
        ).unwrap();
        let stored = db.get_instance(&instance.id).unwrap();
        assert_eq!(stored.version_id.as_deref(), Some("26.3"));
        assert!(stored.horizon_fabric_enabled);
        assert_eq!(stored.horizon_fabric_version_id.as_deref(), Some("fabric-loader-0.20.1-26.3"));
        assert_eq!(stored.horizon_fabric_java_path.as_deref(), Some("/java-25/bin/java"));
        db.set_horizon_fabric_runtime_enabled(&instance.id, false).unwrap();
        assert!(!db.get_instance(&instance.id).unwrap().horizon_fabric_enabled);
    }

    #[test]
    fn cannot_enable_horizon_fabric_runtime_before_installation() {
        let db = Database::in_memory().unwrap();
        let instance = db.create_horizon_instance("Horizon test", "/managed/horizon").unwrap();
        assert!(db.set_horizon_fabric_runtime_enabled(&instance.id, true).is_err());
        assert!(!db.get_instance(&instance.id).unwrap().horizon_fabric_enabled);
    }

    #[test]
    fn migrates_old_instance_tables_to_vanilla_engine() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(
            "CREATE TABLE instances (
                id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, game_dir TEXT NOT NULL,
                version_id TEXT, java_path TEXT, memory_mb INTEGER NOT NULL DEFAULT 2048,
                created_at INTEGER NOT NULL, last_played_at INTEGER
             );",
        ).unwrap();
        let db = Database::from_connection(connection).unwrap();
        let instance = db.create_instance("Existing", "/managed/vanilla", Some("26.3")).unwrap();
        assert_eq!(instance.engine, InstanceEngine::Minecraft);
        assert!(db.list_instances().unwrap().iter().all(|saved| saved.engine == InstanceEngine::Minecraft));
    }

    #[test]
    fn rejects_invalid_profile_names() {
        assert!(validate_offline_name("player name").is_err());
        assert!(validate_offline_name("x".repeat(17).as_str()).is_err());
    }

    #[test]
    fn keeps_mod_provider_records_in_sync_with_file_changes() {
        let db = Database::in_memory().unwrap();
        let instance = db.create_instance("Mod test", "/project/game", Some("fabric-test")).unwrap();
        let source = ModInstallSource {
            file_name: "example.jar".to_owned(),
            provider: "modrinth".to_owned(),
            project_id: "example-project".to_owned(),
            version_id: Some("example-version".to_owned()),
        };

        db.save_mod_sources(&instance.id, std::slice::from_ref(&source)).unwrap();
        assert_eq!(db.get_mod_source(&instance.id, "example.jar").unwrap(), Some(source.clone()));
        db.rename_mod_source(&instance.id, "example.jar", "example.jar.disabled").unwrap();
        assert_eq!(db.get_mod_source(&instance.id, "example.jar").unwrap(), None);
        assert_eq!(db.get_mod_source(&instance.id, "example.jar.disabled").unwrap().unwrap().version_id.as_deref(), Some("example-version"));
        let replacement = ModInstallSource {
            file_name: "example-new.jar".to_owned(),
            version_id: Some("new-version".to_owned()),
            ..source
        };
        db.replace_mod_sources(&instance.id, "example.jar.disabled", std::slice::from_ref(&replacement)).unwrap();
        assert_eq!(db.get_mod_source(&instance.id, "example.jar.disabled").unwrap(), None);
        assert_eq!(db.get_mod_source(&instance.id, "example-new.jar").unwrap(), Some(replacement));
        db.remove_mod_sources(&instance.id, &["example-new.jar".to_owned()]).unwrap();
        assert!(db.list_mod_sources(&instance.id).unwrap().is_empty());
    }
}
