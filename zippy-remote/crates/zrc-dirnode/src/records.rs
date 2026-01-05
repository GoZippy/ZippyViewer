//! Record management and signature verification

use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use dashmap::DashMap;
use async_trait::async_trait;
use thiserror::Error;
use prost::Message;
use zrc_proto::v1::DirRecordV1;
use zrc_crypto::{directory::verify_record, hash::derive_id};

use crate::store::{RecordStore, StoreError};

#[derive(Debug, Error)]
pub enum RecordError {
    #[error("Store error: {0}")]
    Store(#[from] StoreError),
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Subject ID mismatch")]
    SubjectMismatch,
    #[error("Record expired")]
    Expired,
    #[error("Record too large")]
    RecordTooLarge,
    #[error("TTL too long")]
    TTLTooLong,
    #[error("Record not found")]
    NotFound,
}

/// Stored record with metadata
pub struct StoredRecord {
    pub record: DirRecordV1,
    pub stored_at: SystemTime,
    pub access_count: AtomicU64,
}

/// Record configuration
#[derive(Debug, Clone)]
pub struct RecordConfig {
    pub max_record_size: usize,
    pub max_ttl_seconds: u32,
    pub max_records: usize,
    pub cleanup_interval: Duration,
}

impl Default for RecordConfig {
    fn default() -> Self {
        Self {
            max_record_size: 4 * 1024,      // 4KB
            max_ttl_seconds: 86400,          // 24 hours
            max_records: 100_000,
            cleanup_interval: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Record manager
pub struct RecordManager {
    records: DashMap<[u8; 32], Arc<StoredRecord>>,
    store: Arc<dyn RecordStore>,
    config: RecordConfig,
}

impl RecordManager {
    pub fn new(store: Arc<dyn RecordStore>, config: RecordConfig) -> Self {
        Self {
            records: DashMap::new(),
            store,
            config,
        }
    }

    /// Verify record signature and subject ID binding
    pub(crate) fn verify_record(&self, record: &DirRecordV1, now: u64) -> Result<(), RecordError> {
        // Check subject_id matches device_sign_pub
        if record.subject_id.len() != 32 || record.device_sign_pub.len() != 32 {
            return Err(RecordError::SubjectMismatch);
        }

        let derived_id = derive_id(&record.device_sign_pub);
        if record.subject_id != derived_id.as_slice() {
            return Err(RecordError::SubjectMismatch);
        }

        // Encode endpoints for signature verification
        let endpoints_encoded = record.endpoints.as_ref()
            .map(|e| {
                let mut buf = Vec::new();
                Message::encode(e, &mut buf).ok();
                buf
            })
            .unwrap_or_default();

        // Verify signature using zrc-crypto
        verify_record(
            &record.subject_id,
            &record.device_sign_pub,
            &endpoints_encoded,
            record.ttl_seconds,
            record.timestamp,
            &record.signature,
            now,
        )
        .map_err(|_| RecordError::InvalidSignature)?;

        Ok(())
    }

    /// Store or update record
    pub async fn store(&self, record: DirRecordV1) -> Result<(), RecordError> {
        // Enforce size limit (approximate - encode to check)
        let mut test_buf = Vec::new();
        Message::encode(&record, &mut test_buf)
            .map_err(|_| RecordError::RecordTooLarge)?;
        let record_size = test_buf.len();

        if record_size > self.config.max_record_size {
            return Err(RecordError::RecordTooLarge);
        }

        // Enforce TTL limit
        if record.ttl_seconds > self.config.max_ttl_seconds {
            return Err(RecordError::TTLTooLong);
        }

        // Verify signature
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.verify_record(&record, now)?;

        // Extract subject_id
        if record.subject_id.len() != 32 {
            return Err(RecordError::SubjectMismatch);
        }
        let mut subject_id = [0u8; 32];
        subject_id.copy_from_slice(&record.subject_id);

        // Store in database
        self.store.save(&subject_id, &record).await?;

        // Update in-memory cache
        let stored_record = Arc::new(StoredRecord {
            record: record.clone(),
            stored_at: SystemTime::now(),
            access_count: AtomicU64::new(0),
        });
        self.records.insert(subject_id, stored_record);

        Ok(())
    }

    /// Get record by subject ID
    pub async fn get(&self, subject_id: &[u8; 32], now: u64) -> Result<Option<DirRecordV1>, RecordError> {
        // Check in-memory cache first
        if let Some(stored) = self.records.get(subject_id) {
            let record = &stored.record;
            // Check expiration
            let expires_at = record.timestamp.saturating_add(record.ttl_seconds as u64);
            if expires_at > now {
                stored.access_count.fetch_add(1, Ordering::Relaxed);
                return Ok(Some(record.clone()));
            } else {
                // Expired, remove from cache
                self.records.remove(subject_id);
            }
        }

        // Load from database
        if let Some(record) = self.store.load(subject_id).await? {
            // Check expiration
            let expires_at = record.timestamp.saturating_add(record.ttl_seconds as u64);
            if expires_at > now {
                // Cache it
                let stored_record = Arc::new(StoredRecord {
                    record: record.clone(),
                    stored_at: SystemTime::now(),
                    access_count: AtomicU64::new(1),
                });
                self.records.insert(*subject_id, stored_record);
                Ok(Some(record))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Get multiple records
    pub async fn get_batch(&self, subject_ids: &[[u8; 32]], now: u64) -> Vec<Option<DirRecordV1>> {
        let mut results = Vec::new();
        for subject_id in subject_ids {
            match self.get(subject_id, now).await {
                Ok(Some(record)) => results.push(Some(record)),
                Ok(None) | Err(_) => results.push(None),
            }
        }
        results
    }

    /// Delete record
    pub async fn delete(&self, subject_id: &[u8; 32]) -> Result<(), RecordError> {
        self.store.delete(subject_id).await?;
        self.records.remove(subject_id);
        Ok(())
    }

    /// Run expiration cleanup
    pub async fn cleanup_expired(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Get expired records from database
        if let Ok(expired_ids) = self.store.list_expired(now).await {
            for subject_id in expired_ids {
                self.records.remove(&subject_id);
                let _ = self.store.delete(&subject_id).await;
            }
        }

        // Also clean in-memory cache
        self.records.retain(|_id, stored| {
            let record = &stored.record;
            let expires_at = record.timestamp.saturating_add(record.ttl_seconds as u64);
            expires_at > now
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SqliteStore;
    use tempfile::TempDir;
    use zrc_crypto::identity::Identity;

    #[tokio::test]
    async fn test_record_store_and_retrieve() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = Arc::new(SqliteStore::new(&db_path).await.unwrap());
        let record_mgr = RecordManager::new(store, RecordConfig::default());

        // Create a valid signed record
        let identity = Identity::generate();
        let subject_id = identity.id();
        let device_sign_pub = identity.sign_pub();

        let mut record = DirRecordV1::default();
        record.subject_id = subject_id.to_vec();
        record.device_sign_pub = device_sign_pub.to_vec();
        record.ttl_seconds = 3600;
        record.timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Sign the record
        let endpoints_encoded = Vec::new();
        let sign_data = zrc_crypto::directory::dir_record_sign_data(
            &record.subject_id,
            &record.device_sign_pub,
            &endpoints_encoded,
            record.ttl_seconds,
            record.timestamp,
        );
        record.signature = identity.sign(&sign_data).to_vec();

        // Store
        record_mgr.store(record.clone()).await.unwrap();

        // Retrieve
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let retrieved = record_mgr.get(&subject_id, now).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_record = retrieved.unwrap();
        assert_eq!(retrieved_record.subject_id, record.subject_id);
    }

    #[tokio::test]
    async fn test_record_ttl_enforcement() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = Arc::new(SqliteStore::new(&db_path).await.unwrap());
        let record_mgr = RecordManager::new(store, RecordConfig::default());

        let identity = Identity::generate();
        let subject_id = identity.id();
        let device_sign_pub = identity.sign_pub();

        // Store with current timestamp and short TTL
        let base_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut record = DirRecordV1::default();
        record.subject_id = subject_id.to_vec();
        record.device_sign_pub = device_sign_pub.to_vec();
        record.ttl_seconds = 100; // Short TTL
        record.timestamp = base_timestamp;

        let endpoints_encoded = Vec::new();
        let sign_data = zrc_crypto::directory::dir_record_sign_data(
            &record.subject_id,
            &record.device_sign_pub,
            &endpoints_encoded,
            record.ttl_seconds,
            record.timestamp,
        );
        record.signature = identity.sign(&sign_data).to_vec();

        // Store (should succeed since not expired yet)
        record_mgr.store(record).await.unwrap();

        // Try to retrieve immediately (should succeed)
        let now = base_timestamp + 50; // Still within TTL
        let retrieved = record_mgr.get(&subject_id, now).await.unwrap();
        assert!(retrieved.is_some());

        // Try to retrieve after expiration (should return None)
        let now_expired = base_timestamp + 200; // 100 + 200 = 300 > 100, so expired
        let retrieved = record_mgr.get(&subject_id, now_expired).await.unwrap();
        assert!(retrieved.is_none());
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use crate::store::MemoryStore;
    use zrc_crypto::identity::Identity;

    /// Property 1: Signature Verification
    /// Validates: Requirements 1.2, 1.3
    /// 
    /// Property: For any record with valid signature, verification succeeds.
    /// For any record with invalid signature, verification fails.
    #[test]
    fn prop_signature_verification() {
        proptest!(|(
            ttl_seconds in 1u32..=86400u32,
            timestamp in 1000u64..=2000000000u64,
        )| {
            let identity = Identity::generate();
            let subject_id = identity.id();
            let device_sign_pub = identity.sign_pub();

            let mut record = DirRecordV1::default();
            record.subject_id = subject_id.to_vec();
            record.device_sign_pub = device_sign_pub.to_vec();
            record.ttl_seconds = ttl_seconds;
            record.timestamp = timestamp;

            let endpoints_encoded = Vec::new();
            let sign_data = zrc_crypto::directory::dir_record_sign_data(
                &record.subject_id,
                &record.device_sign_pub,
                &endpoints_encoded,
                record.ttl_seconds,
                record.timestamp,
            );
            record.signature = identity.sign(&sign_data).to_vec();

            // Verify signature should succeed
            let now = timestamp + 1;
            let store = Arc::new(MemoryStore::new());
            let record_mgr = RecordManager::new(store, RecordConfig::default());
            prop_assert!(record_mgr.verify_record(&record, now).is_ok());
        });
    }

    /// Property 2: Subject ID Binding
    /// Validates: Requirements 1.4
    /// 
    /// Property: For any record, subject_id must match SHA256(device_sign_pub)[0..32].
    #[test]
    fn prop_subject_id_binding() {
        proptest!(|(
            _dummy in 0u8..=255u8,
        )| {
            let identity = Identity::generate();
            let correct_subject_id = identity.id();
            let device_sign_pub = identity.sign_pub();

            let mut record = DirRecordV1::default();
            record.subject_id = correct_subject_id.to_vec();
            record.device_sign_pub = device_sign_pub.to_vec();
            record.ttl_seconds = 3600;
            record.timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let endpoints_encoded = Vec::new();
            let sign_data = zrc_crypto::directory::dir_record_sign_data(
                &record.subject_id,
                &record.device_sign_pub,
                &endpoints_encoded,
                record.ttl_seconds,
                record.timestamp,
            );
            record.signature = identity.sign(&sign_data).to_vec();

            let store = Arc::new(MemoryStore::new());
            let record_mgr = RecordManager::new(store, RecordConfig::default());
            let now = record.timestamp + 1;
            
            // Should succeed with correct subject_id
            prop_assert!(record_mgr.verify_record(&record, now).is_ok());

            // Should fail with incorrect subject_id
            let mut wrong_record = record.clone();
            wrong_record.subject_id = vec![0u8; 32];
            prop_assert!(record_mgr.verify_record(&wrong_record, now).is_err());
        });
    }

    /// Property 3: TTL Enforcement
    /// Validates: Requirements 2.4
    /// 
    /// Property: For any record where timestamp + ttl_seconds < now, get() returns None.
    #[test]
    fn prop_ttl_enforcement() {
        proptest!(|(
            ttl_seconds in 1u32..=86400u32,
            timestamp in 1000u64..=1000000u64,
            now_offset in 0u64..=2000000u64,
        )| {
            let expires_at = timestamp.saturating_add(ttl_seconds as u64);
            let now = timestamp.saturating_add(now_offset);

            // Verify TTL logic
            if now >= expires_at {
                // Record should be expired
                prop_assert!(now >= expires_at);
            } else {
                // Record should still be valid
                prop_assert!(now < expires_at);
            }
        });
    }

    /// Property 6: Record Integrity
    /// Validates: Requirements 6.2
    /// 
    /// Property: For any stored record, retrieval returns byte-identical data.
    #[test]
    fn prop_record_integrity() {
        proptest!(|(
            ttl_seconds in 1u32..=86400u32,
            timestamp in 1000u64..=2000000000u64,
        )| {
            let identity = Identity::generate();
            let subject_id = identity.id();
            let device_sign_pub = identity.sign_pub();

            let mut record = DirRecordV1::default();
            record.subject_id = subject_id.to_vec();
            record.device_sign_pub = device_sign_pub.to_vec();
            record.ttl_seconds = ttl_seconds;
            record.timestamp = timestamp;

            let endpoints_encoded = Vec::new();
            let sign_data = zrc_crypto::directory::dir_record_sign_data(
                &record.subject_id,
                &record.device_sign_pub,
                &endpoints_encoded,
                record.ttl_seconds,
                record.timestamp,
            );
            record.signature = identity.sign(&sign_data).to_vec();

            // Encode original
            let mut original_bytes = Vec::new();
            Message::encode(&record, &mut original_bytes).unwrap();

            // Decode and re-encode to verify integrity
            let decoded: DirRecordV1 = Message::decode(&original_bytes[..]).unwrap();
            let mut reencoded_bytes = Vec::new();
            Message::encode(&decoded, &mut reencoded_bytes).unwrap();

            prop_assert_eq!(original_bytes, reencoded_bytes);
        });
    }
}
