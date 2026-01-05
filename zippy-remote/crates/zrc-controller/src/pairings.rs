//! Pairings store for persistent storage of device pairings
//!
//! This module provides SQLite-based persistent storage for device pairings.
//! Requirements: 7.1, 7.2

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

/// Pairings store errors
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Pairing not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Stored pairing information
#[derive(Debug, Clone)]
pub struct StoredPairing {
    /// Device ID (hex-encoded)
    pub device_id: String,
    /// Device name (optional)
    pub device_name: Option<String>,
    /// Device signing public key
    pub device_sign_pub: [u8; 32],
    /// Device key exchange public key
    pub device_kex_pub: [u8; 32],
    /// Granted permissions
    pub permissions: Vec<String>,
    /// When the pairing was established
    pub paired_at: SystemTime,
    /// Last session time (if any)
    pub last_session: Option<SystemTime>,
    /// Total session count
    pub session_count: u32,
}

/// Persistent storage for pairings using SQLite
pub struct PairingsStore {
    conn: Connection,
}

impl PairingsStore {
    /// Open or create pairings database
    /// Requirements: 7.1
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        
        // Create tables if they don't exist
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS pairings (
                device_id TEXT PRIMARY KEY,
                device_name TEXT,
                device_sign_pub BLOB NOT NULL,
                device_kex_pub BLOB NOT NULL,
                permissions TEXT NOT NULL,
                paired_at INTEGER NOT NULL,
                last_session INTEGER,
                session_count INTEGER NOT NULL DEFAULT 0
            );
            
            CREATE INDEX IF NOT EXISTS idx_pairings_paired_at ON pairings(paired_at);
            "#,
        )?;

        Ok(Self { conn })
    }

    /// Get default database path
    pub fn default_path() -> Option<std::path::PathBuf> {
        directories::ProjectDirs::from("io", "zippyremote", "zrc")
            .map(|dirs| dirs.data_dir().join("pairings.db"))
    }

    /// List all pairings
    /// Requirements: 7.1
    pub fn list(&self) -> Result<Vec<StoredPairing>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, device_name, device_sign_pub, device_kex_pub, 
                    permissions, paired_at, last_session, session_count 
             FROM pairings ORDER BY paired_at DESC",
        )?;

        let pairings = stmt
            .query_map([], |row| {
                let device_id: String = row.get(0)?;
                let device_name: Option<String> = row.get(1)?;
                let device_sign_pub: Vec<u8> = row.get(2)?;
                let device_kex_pub: Vec<u8> = row.get(3)?;
                let permissions_str: String = row.get(4)?;
                let paired_at_unix: i64 = row.get(5)?;
                let last_session_unix: Option<i64> = row.get(6)?;
                let session_count: u32 = row.get(7)?;

                Ok(StoredPairing {
                    device_id,
                    device_name,
                    device_sign_pub: device_sign_pub.try_into().unwrap_or([0u8; 32]),
                    device_kex_pub: device_kex_pub.try_into().unwrap_or([0u8; 32]),
                    permissions: permissions_str.split(',').map(|s| s.to_string()).collect(),
                    paired_at: UNIX_EPOCH + Duration::from_secs(paired_at_unix as u64),
                    last_session: last_session_unix
                        .map(|ts| UNIX_EPOCH + Duration::from_secs(ts as u64)),
                    session_count,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(pairings)
    }

    /// Get pairing by device ID
    /// Requirements: 7.3
    pub fn get(&self, device_id: &str) -> Result<Option<StoredPairing>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, device_name, device_sign_pub, device_kex_pub, 
                    permissions, paired_at, last_session, session_count 
             FROM pairings WHERE device_id = ?",
        )?;

        let pairing = stmt
            .query_row([device_id], |row| {
                let device_id: String = row.get(0)?;
                let device_name: Option<String> = row.get(1)?;
                let device_sign_pub: Vec<u8> = row.get(2)?;
                let device_kex_pub: Vec<u8> = row.get(3)?;
                let permissions_str: String = row.get(4)?;
                let paired_at_unix: i64 = row.get(5)?;
                let last_session_unix: Option<i64> = row.get(6)?;
                let session_count: u32 = row.get(7)?;

                Ok(StoredPairing {
                    device_id,
                    device_name,
                    device_sign_pub: device_sign_pub.try_into().unwrap_or([0u8; 32]),
                    device_kex_pub: device_kex_pub.try_into().unwrap_or([0u8; 32]),
                    permissions: permissions_str.split(',').map(|s| s.to_string()).collect(),
                    paired_at: UNIX_EPOCH + Duration::from_secs(paired_at_unix as u64),
                    last_session: last_session_unix
                        .map(|ts| UNIX_EPOCH + Duration::from_secs(ts as u64)),
                    session_count,
                })
            })
            .optional()?;

        Ok(pairing)
    }

    /// Store new pairing
    /// Requirements: 7.2
    pub fn store(&self, pairing: StoredPairing) -> Result<(), StoreError> {
        let paired_at_unix = pairing
            .paired_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let last_session_unix = pairing.last_session.map(|ts| {
            ts.duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        let permissions_str = pairing.permissions.join(",");

        self.conn.execute(
            "INSERT OR REPLACE INTO pairings 
             (device_id, device_name, device_sign_pub, device_kex_pub, 
              permissions, paired_at, last_session, session_count)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                pairing.device_id,
                pairing.device_name,
                pairing.device_sign_pub.to_vec(),
                pairing.device_kex_pub.to_vec(),
                permissions_str,
                paired_at_unix,
                last_session_unix,
                pairing.session_count,
            ],
        )?;

        Ok(())
    }

    /// Update existing pairing
    pub fn update(&self, pairing: &StoredPairing) -> Result<(), StoreError> {
        let paired_at_unix = pairing
            .paired_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let last_session_unix = pairing.last_session.map(|ts| {
            ts.duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        let permissions_str = pairing.permissions.join(",");

        let rows = self.conn.execute(
            "UPDATE pairings SET 
             device_name = ?, device_sign_pub = ?, device_kex_pub = ?,
             permissions = ?, paired_at = ?, last_session = ?, session_count = ?
             WHERE device_id = ?",
            params![
                pairing.device_name,
                pairing.device_sign_pub.to_vec(),
                pairing.device_kex_pub.to_vec(),
                permissions_str,
                paired_at_unix,
                last_session_unix,
                pairing.session_count,
                pairing.device_id,
            ],
        )?;

        if rows == 0 {
            return Err(StoreError::NotFound(pairing.device_id.clone()));
        }

        Ok(())
    }

    /// Delete pairing
    /// Requirements: 7.4
    pub fn delete(&self, device_id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM pairings WHERE device_id = ?", [device_id])?;
        Ok(())
    }

    /// Update last session timestamp
    pub fn update_last_session(&self, device_id: &str) -> Result<(), StoreError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let rows = self.conn.execute(
            "UPDATE pairings SET last_session = ?, session_count = session_count + 1 
             WHERE device_id = ?",
            params![now, device_id],
        )?;

        if rows == 0 {
            return Err(StoreError::NotFound(device_id.to_string()));
        }

        Ok(())
    }

    /// Export pairings to file
    /// Requirements: 7.5
    pub fn export(&self, path: &Path) -> Result<(), StoreError> {
        let pairings = self.list()?;
        
        #[derive(serde::Serialize)]
        struct ExportedPairing {
            device_id: String,
            device_name: Option<String>,
            device_sign_pub: String,
            device_kex_pub: String,
            permissions: Vec<String>,
            paired_at: String,
            last_session: Option<String>,
            session_count: u32,
        }

        let exported: Vec<ExportedPairing> = pairings
            .into_iter()
            .map(|p| {
                let paired_at: chrono::DateTime<chrono::Utc> = p.paired_at.into();
                let last_session = p.last_session.map(|ts| {
                    let dt: chrono::DateTime<chrono::Utc> = ts.into();
                    dt.to_rfc3339()
                });

                ExportedPairing {
                    device_id: p.device_id,
                    device_name: p.device_name,
                    device_sign_pub: hex::encode(p.device_sign_pub),
                    device_kex_pub: hex::encode(p.device_kex_pub),
                    permissions: p.permissions,
                    paired_at: paired_at.to_rfc3339(),
                    last_session,
                    session_count: p.session_count,
                }
            })
            .collect();

        let json = serde_json::to_string_pretty(&exported)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Import pairings from file
    /// Requirements: 7.6
    pub fn import(&self, path: &Path) -> Result<u32, StoreError> {
        let contents = std::fs::read_to_string(path)?;
        
        #[derive(serde::Deserialize)]
        struct ImportedPairing {
            device_id: String,
            device_name: Option<String>,
            device_sign_pub: String,
            device_kex_pub: String,
            permissions: Vec<String>,
            paired_at: String,
            last_session: Option<String>,
            session_count: u32,
        }

        let imported: Vec<ImportedPairing> = serde_json::from_str(&contents)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        let mut count = 0;
        for p in imported {
            let device_sign_pub: [u8; 32] = hex::decode(&p.device_sign_pub)
                .map_err(|e| StoreError::Serialization(e.to_string()))?
                .try_into()
                .map_err(|_| StoreError::Serialization("Invalid key length".to_string()))?;

            let device_kex_pub: [u8; 32] = hex::decode(&p.device_kex_pub)
                .map_err(|e| StoreError::Serialization(e.to_string()))?
                .try_into()
                .map_err(|_| StoreError::Serialization("Invalid key length".to_string()))?;

            let paired_at = chrono::DateTime::parse_from_rfc3339(&p.paired_at)
                .map_err(|e| StoreError::Serialization(e.to_string()))?
                .with_timezone(&chrono::Utc);

            let last_session = p.last_session.map(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&chrono::Utc).into())
                    .ok()
            }).flatten();

            let pairing = StoredPairing {
                device_id: p.device_id,
                device_name: p.device_name,
                device_sign_pub,
                device_kex_pub,
                permissions: p.permissions,
                paired_at: paired_at.into(),
                last_session,
                session_count: p.session_count,
            };

            self.store(pairing)?;
            count += 1;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_pairing(device_id: &str) -> StoredPairing {
        StoredPairing {
            device_id: device_id.to_string(),
            device_name: Some("Test Device".to_string()),
            device_sign_pub: [1u8; 32],
            device_kex_pub: [2u8; 32],
            permissions: vec!["view".to_string(), "control".to_string()],
            paired_at: SystemTime::now(),
            last_session: None,
            session_count: 0,
        }
    }

    #[test]
    fn test_store_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("pairings.db");
        let store = PairingsStore::open(&db_path).unwrap();

        let pairing = create_test_pairing("device123");
        store.store(pairing.clone()).unwrap();

        let retrieved = store.get("device123").unwrap().unwrap();
        assert_eq!(retrieved.device_id, "device123");
        assert_eq!(retrieved.device_name, Some("Test Device".to_string()));
        assert_eq!(retrieved.permissions, vec!["view", "control"]);
    }

    #[test]
    fn test_list() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("pairings.db");
        let store = PairingsStore::open(&db_path).unwrap();

        store.store(create_test_pairing("device1")).unwrap();
        store.store(create_test_pairing("device2")).unwrap();
        store.store(create_test_pairing("device3")).unwrap();

        let pairings = store.list().unwrap();
        assert_eq!(pairings.len(), 3);
    }

    #[test]
    fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("pairings.db");
        let store = PairingsStore::open(&db_path).unwrap();

        store.store(create_test_pairing("device123")).unwrap();
        assert!(store.get("device123").unwrap().is_some());

        store.delete("device123").unwrap();
        assert!(store.get("device123").unwrap().is_none());
    }

    #[test]
    fn test_update_last_session() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("pairings.db");
        let store = PairingsStore::open(&db_path).unwrap();

        store.store(create_test_pairing("device123")).unwrap();
        
        let before = store.get("device123").unwrap().unwrap();
        assert!(before.last_session.is_none());
        assert_eq!(before.session_count, 0);

        store.update_last_session("device123").unwrap();

        let after = store.get("device123").unwrap().unwrap();
        assert!(after.last_session.is_some());
        assert_eq!(after.session_count, 1);
    }

    #[test]
    fn test_export_import() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("pairings.db");
        let export_path = temp_dir.path().join("export.json");
        
        // Create and populate first store
        let store1 = PairingsStore::open(&db_path).unwrap();
        store1.store(create_test_pairing("device1")).unwrap();
        store1.store(create_test_pairing("device2")).unwrap();
        store1.export(&export_path).unwrap();
        drop(store1);

        // Create second store and import
        let db_path2 = temp_dir.path().join("pairings2.db");
        let store2 = PairingsStore::open(&db_path2).unwrap();
        let count = store2.import(&export_path).unwrap();
        
        assert_eq!(count, 2);
        assert!(store2.get("device1").unwrap().is_some());
        assert!(store2.get("device2").unwrap().is_some());
    }
}
