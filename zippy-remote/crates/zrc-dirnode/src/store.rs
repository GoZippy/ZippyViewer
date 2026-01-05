//! SQLite-based record storage

use std::path::Path;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use rusqlite::{Connection, params, OptionalExtension};
use thiserror::Error;
use prost::Message;
use zrc_proto::v1::DirRecordV1;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Record store trait
#[async_trait]
pub trait RecordStore: Send + Sync {
    async fn save(&self, subject_id: &[u8; 32], record: &DirRecordV1) -> Result<(), StoreError>;
    async fn load(&self, subject_id: &[u8; 32]) -> Result<Option<DirRecordV1>, StoreError>;
    async fn delete(&self, subject_id: &[u8; 32]) -> Result<(), StoreError>;
    async fn list_expired(&self, now: u64) -> Result<Vec<[u8; 32]>, StoreError>;
}

/// In-memory store for testing
#[cfg(test)]
pub struct MemoryStore {
    records: Arc<Mutex<std::collections::HashMap<[u8; 32], DirRecordV1>>>,
}

#[cfg(test)]
impl MemoryStore {
    pub fn new() -> Self {
        Self {
            records: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl RecordStore for MemoryStore {
    async fn save(&self, subject_id: &[u8; 32], record: &DirRecordV1) -> Result<(), StoreError> {
        self.records.lock().unwrap().insert(*subject_id, record.clone());
        Ok(())
    }

    async fn load(&self, subject_id: &[u8; 32]) -> Result<Option<DirRecordV1>, StoreError> {
        Ok(self.records.lock().unwrap().get(subject_id).cloned())
    }

    async fn delete(&self, subject_id: &[u8; 32]) -> Result<(), StoreError> {
        self.records.lock().unwrap().remove(subject_id);
        Ok(())
    }

    async fn list_expired(&self, _now: u64) -> Result<Vec<[u8; 32]>, StoreError> {
        // For property tests, we don't need to implement this
        Ok(Vec::new())
    }
}

/// SQLite-based record store
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    /// Create new SQLite store
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema().await?;
        Ok(store)
    }

    /// Initialize database schema
    async fn init_schema(&self) -> Result<(), StoreError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let conn = conn.lock().unwrap();
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS records (
                    subject_id BLOB PRIMARY KEY,
                    record_data BLOB NOT NULL,
                    signature BLOB NOT NULL,
                    timestamp INTEGER NOT NULL,
                    ttl_seconds INTEGER NOT NULL,
                    stored_at INTEGER NOT NULL
                )
                "#,
                [],
            )?;

            conn.execute(
                r#"
                CREATE INDEX IF NOT EXISTS idx_records_expiry 
                ON records (timestamp + ttl_seconds)
                "#,
                [],
            )?;

            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS discovery_tokens (
                    token_id BLOB PRIMARY KEY,
                    subject_id BLOB NOT NULL,
                    expires_at INTEGER NOT NULL,
                    scope TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                )
                "#,
                [],
            )?;

            conn.execute(
                r#"
                CREATE INDEX IF NOT EXISTS idx_discovery_subject 
                ON discovery_tokens (subject_id)
                "#,
                [],
            )?;

            conn.execute(
                r#"
                CREATE INDEX IF NOT EXISTS idx_discovery_expiry 
                ON discovery_tokens (expires_at)
                "#,
                [],
            )?;

            Ok(())
        }).await
        .map_err(|_| StoreError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            None
        )))?
        .map_err(StoreError::Database)?;
        Ok(())
    }

    /// Backup database using VACUUM INTO
    pub async fn backup(&self, dest: impl AsRef<Path>) -> Result<(), StoreError> {
        let conn = self.conn.clone();
        let dest = dest.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let conn = conn.lock().unwrap();
            let dest_str = dest.to_string_lossy().replace("'", "''");
            conn.execute(&format!("VACUUM INTO '{}'", dest_str), [])?;
            Ok(())
        }).await
        .map_err(|_| StoreError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            None
        )))?
        .map_err(StoreError::Database)?;
        Ok(())
    }

    /// Export all records as JSON
    pub async fn export_json(&self) -> Result<String, StoreError> {
        let conn = self.conn.clone();
        let rows: Vec<(Vec<u8>, Vec<u8>)> = tokio::task::spawn_blocking(move || -> Result<Vec<(Vec<u8>, Vec<u8>)>, rusqlite::Error> {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT subject_id, record_data FROM records")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                ))
            })?;

            let mut results = Vec::new();
            for row in rows {
                results.push(row?);
            }
            Ok(results)
        }).await
        .map_err(|_| StoreError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            None
        )))?
        .map_err(StoreError::Database)?;

        let mut records = Vec::new();
        for (subject_id, record_data) in rows {
            let record: DirRecordV1 = Message::decode(&record_data[..])
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            records.push(serde_json::json!({
                "subject_id": hex::encode(subject_id),
                "record": {
                    "subject_id": hex::encode(record.subject_id),
                    "device_sign_pub": hex::encode(record.device_sign_pub),
                    "ttl_seconds": record.ttl_seconds,
                    "timestamp": record.timestamp,
                    "signature": hex::encode(record.signature),
                }
            }));
        }

        serde_json::to_string_pretty(&records)
            .map_err(|e| StoreError::Serialization(e.to_string()))
    }

    /// Import records from JSON
    pub async fn import_json(&self, json: &str) -> Result<usize, StoreError> {
        let records: Vec<serde_json::Value> = serde_json::from_str(json)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let mut count = 0;

        for record_json in records {
            let subject_id_hex = record_json["subject_id"]
                .as_str()
                .ok_or_else(|| StoreError::Serialization("missing subject_id".to_string()))?;
            let subject_id = hex::decode(subject_id_hex)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;

            if subject_id.len() != 32 {
                continue;
            }

            let mut subject_id_arr = [0u8; 32];
            subject_id_arr.copy_from_slice(&subject_id);

            // Reconstruct DirRecordV1 from JSON
            let record_data = record_json["record"].as_object()
                .ok_or_else(|| StoreError::Serialization("invalid record format".to_string()))?;

            let mut record = DirRecordV1::default();
            record.subject_id = hex::decode(record_data["subject_id"]
                .as_str()
                .ok_or_else(|| StoreError::Serialization("missing subject_id".to_string()))?)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            record.device_sign_pub = hex::decode(record_data["device_sign_pub"]
                .as_str()
                .ok_or_else(|| StoreError::Serialization("missing device_sign_pub".to_string()))?)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            record.ttl_seconds = record_data["ttl_seconds"]
                .as_u64()
                .ok_or_else(|| StoreError::Serialization("missing ttl_seconds".to_string()))?
                as u32;
            record.timestamp = record_data["timestamp"]
                .as_u64()
                .ok_or_else(|| StoreError::Serialization("missing timestamp".to_string()))?;
            record.signature = hex::decode(record_data["signature"]
                .as_str()
                .ok_or_else(|| StoreError::Serialization("missing signature".to_string()))?)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;

            self.save(&subject_id_arr, &record).await?;
            count += 1;
        }

        Ok(count)
    }
}

#[async_trait]
impl RecordStore for SqliteStore {
    async fn save(&self, subject_id: &[u8; 32], record: &DirRecordV1) -> Result<(), StoreError> {
        let mut record_data = Vec::new();
        Message::encode(record, &mut record_data)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let stored_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let conn = self.conn.clone();
        let subject_id = *subject_id;
        let record_data = record_data.clone();
        let signature = record.signature.clone();
        let timestamp = record.timestamp;
        let ttl_seconds = record.ttl_seconds;

        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let conn = conn.lock().unwrap();
            conn.execute(
                r#"
                INSERT OR REPLACE INTO records 
                (subject_id, record_data, signature, timestamp, ttl_seconds, stored_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    subject_id.as_slice(),
                    record_data.as_slice(),
                    signature.as_slice(),
                    timestamp as i64,
                    ttl_seconds as i64,
                    stored_at as i64,
                ],
            )?;
            Ok(())
        }).await
        .map_err(|_| StoreError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            None
        )))?
        .map_err(StoreError::Database)?;

        Ok(())
    }

    async fn load(&self, subject_id: &[u8; 32]) -> Result<Option<DirRecordV1>, StoreError> {
        let conn = self.conn.clone();
        let subject_id = *subject_id;

        let record_data = tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>, rusqlite::Error> {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT record_data FROM records WHERE subject_id = ?1")?;
            Ok(stmt.query_row(params![subject_id.as_slice()], |row| {
                row.get::<_, Vec<u8>>(0)
            })
            .optional()?)
        }).await
        .map_err(|_| StoreError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            None
        )))?
        .map_err(StoreError::Database)?;

        if let Some(record_data) = record_data {
            let record: DirRecordV1 = Message::decode(&record_data[..])
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, subject_id: &[u8; 32]) -> Result<(), StoreError> {
        let conn = self.conn.clone();
        let subject_id = *subject_id;
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let conn = conn.lock().unwrap();
            conn.execute("DELETE FROM records WHERE subject_id = ?1", params![subject_id.as_slice()])?;
            Ok(())
        }).await
        .map_err(|_| StoreError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            None
        )))?
        .map_err(StoreError::Database)?;
        Ok(())
    }

    async fn list_expired(&self, now: u64) -> Result<Vec<[u8; 32]>, StoreError> {
        let conn = self.conn.clone();
        let expired = tokio::task::spawn_blocking(move || -> Result<Vec<[u8; 32]>, rusqlite::Error> {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT subject_id FROM records WHERE (timestamp + ttl_seconds) <= ?1")?;
            let rows = stmt.query_map(params![now as i64], |row| {
                row.get::<_, Vec<u8>>(0)
            })?;

            let mut expired = Vec::new();
            for row in rows {
                let subject_id_bytes = row?;
                if subject_id_bytes.len() == 32 {
                    let mut subject_id = [0u8; 32];
                    subject_id.copy_from_slice(&subject_id_bytes);
                    expired.push(subject_id);
                }
            }
            Ok(expired)
        }).await
        .map_err(|_| StoreError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            None
        )))?
        .map_err(StoreError::Database)?;

        Ok(expired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use zrc_proto::v1::DirRecordV1;

    #[tokio::test]
    async fn test_store_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let store = SqliteStore::new(&db_path).await.unwrap();
        
        // Create a test record
        let mut record = DirRecordV1::default();
        record.subject_id = vec![1u8; 32];
        record.device_sign_pub = vec![2u8; 32];
        record.ttl_seconds = 3600;
        record.timestamp = 1000;
        record.signature = vec![3u8; 64];
        
        let subject_id = [1u8; 32];
        
        // Save
        store.save(&subject_id, &record).await.unwrap();
        
        // Load
        let loaded = store.load(&subject_id).await.unwrap();
        assert!(loaded.is_some());
        let loaded_record = loaded.unwrap();
        
        assert_eq!(loaded_record.subject_id, record.subject_id);
        assert_eq!(loaded_record.device_sign_pub, record.device_sign_pub);
        assert_eq!(loaded_record.ttl_seconds, record.ttl_seconds);
        assert_eq!(loaded_record.timestamp, record.timestamp);
        assert_eq!(loaded_record.signature, record.signature);
    }

    #[tokio::test]
    async fn test_store_delete() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let store = SqliteStore::new(&db_path).await.unwrap();
        
        let mut record = DirRecordV1::default();
        record.subject_id = vec![1u8; 32];
        record.device_sign_pub = vec![2u8; 32];
        record.ttl_seconds = 3600;
        record.timestamp = 1000;
        record.signature = vec![3u8; 64];
        
        let subject_id = [1u8; 32];
        
        store.save(&subject_id, &record).await.unwrap();
        assert!(store.load(&subject_id).await.unwrap().is_some());
        
        store.delete(&subject_id).await.unwrap();
        assert!(store.load(&subject_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_expired() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let store = SqliteStore::new(&db_path).await.unwrap();
        
        let now = 2000u64;
        
        // Create expired record
        let mut expired_record = DirRecordV1::default();
        expired_record.subject_id = vec![1u8; 32];
        expired_record.device_sign_pub = vec![2u8; 32];
        expired_record.ttl_seconds = 100; // Expires at 1000 + 100 = 1100 < 2000
        expired_record.timestamp = 1000;
        expired_record.signature = vec![3u8; 64];
        
        // Create non-expired record
        let mut active_record = DirRecordV1::default();
        active_record.subject_id = vec![4u8; 32];
        active_record.device_sign_pub = vec![5u8; 32];
        active_record.ttl_seconds = 2000; // Expires at 1000 + 2000 = 3000 > 2000
        active_record.timestamp = 1000;
        active_record.signature = vec![6u8; 64];
        
        let expired_id = [1u8; 32];
        let active_id = [4u8; 32];
        
        store.save(&expired_id, &expired_record).await.unwrap();
        store.save(&active_id, &active_record).await.unwrap();
        
        let expired = store.list_expired(now).await.unwrap();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], expired_id);
    }
}
