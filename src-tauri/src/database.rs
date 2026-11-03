use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use md5::{Digest, Md5};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::{LauncherError, Result};
use crate::models::{Instance, InstanceEngine, OfflineProfile};

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
             CREATE TABLE IF NOT EXISTS settings (
                 key TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
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
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn list_instances(&self) -> Result<Vec<Instance>> {
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, game_dir, version_id, java_path, memory_mb, created_at, last_played_at, engine
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
                "SELECT id, name, game_dir, version_id, java_path, memory_mb, created_at, last_played_at, engine
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
            memory_mb: 2048,
            created_at: unix_time(),
            last_played_at: None,
        };
        let connection = self.connection.lock().map_err(|_| {
            LauncherError::Unsupported("database lock was poisoned".to_owned())
        })?;
        connection.execute(
            "INSERT INTO instances (id, name, engine, game_dir, version_id, java_path, memory_mb, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                instance.id,
                instance.name,
                instance.engine.as_str(),
                instance.game_dir,
                instance.version_id,
                instance.java_path,
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

fn unix_time() -> i64 {
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
    fn creates_horizon_instance_as_a_separate_engine_at_26_3() {
        let db = Database::in_memory().unwrap();
        let instance = db.create_horizon_instance("Horizon test", "/managed/horizon").unwrap();
        assert_eq!(instance.engine, InstanceEngine::Horizon);
        assert_eq!(instance.version_id.as_deref(), Some("26.3"));
        assert_eq!(db.get_instance(&instance.id).unwrap(), instance);
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
