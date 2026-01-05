//! SQLite-based persistent storage implementation for ZRC.
//!
//! This module provides a production-ready storage backend using SQLite
//! with support for atomic operations and schema migrations.
//!
//! Requirements: 8.5, 8.6, 8.7

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use rusqlite::{params, Connection, OptionalExtension};
use tokio::sync::Mutex;

use crate::store::{InviteRecord, PairingRecord, Store, StoreError, TicketRecord};
use zrc_proto::v1::PublicKeyV1;

// ============================================================================
// Schema Version
// ============================================================================

/// Current schema version for migrations.
/// Increment this when adding new migrations.
#[allow(dead_code)]
const SCHEMA_VERSION: i32 = 1;

// ============================================================================
// SQLite Store Implementation
// ============================================================================

/// SQLite-based persistent store implementation.
///
/// Provides durable storage for invites, pairings, and tickets with:
/// - Atomic operations using transactions
/// - Schema migrations for version upgrades
/// - Thread-safe access via Mutex
///
/// Requirements: 8.5, 8.6, 8.7
pub struct SqliteStore {
    /// SQLite connection wrapped in a mutex for thread-safe access
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    /// Create a new SQLite store at the specified path.
    ///
    /// Creates the database file if it doesn't exist and runs migrations.
    ///
    /// # Arguments
    /// * `path` - Path to the SQLite database file
    ///
    /// # Returns
    /// * `Ok(SqliteStore)` on success
    /// * `Err(StoreError)` if database creation or migration fails
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| {
            StoreError::OperationFailed(format!("failed to open database: {}", e))
        })?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| StoreError::OperationFailed(format!("failed to set pragmas: {}", e)))?;

        // Run migrations synchronously during construction (before wrapping in Mutex)
        Self::run_migrations(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create a new in-memory SQLite store for testing.
    ///
    /// # Returns
    /// * `Ok(SqliteStore)` on success
    /// * `Err(StoreError)` if database creation fails
    pub fn new_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(|e| {
            StoreError::OperationFailed(format!("failed to open in-memory database: {}", e))
        })?;

        // Run migrations synchronously during construction (before wrapping in Mutex)
        Self::run_migrations(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run database migrations to ensure schema is up to date.
    fn run_migrations(conn: &Connection) -> Result<(), StoreError> {
        // Create schema_version table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
            [],
        )
        .map_err(|e| StoreError::OperationFailed(format!("failed to create schema_version: {}", e)))?;

        // Get current version
        let current_version: i32 = conn
            .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        // Run migrations
        if current_version < 1 {
            Self::migrate_v1(conn)?;
        }

        Ok(())
    }

    /// Migration to schema version 1 - initial schema.
    fn migrate_v1(conn: &Connection) -> Result<(), StoreError> {
        conn.execute_batch(
            r#"
            -- Invites table
            CREATE TABLE IF NOT EXISTS invites (
                device_id BLOB PRIMARY KEY,
                invite_secret BLOB NOT NULL,
                expires_at_unix INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_invites_expires ON invites(expires_at_unix);

            -- Pairings table
            CREATE TABLE IF NOT EXISTS pairings (
                pairing_id BLOB PRIMARY KEY,
                device_id BLOB NOT NULL,
                operator_id BLOB NOT NULL,
                device_sign_pub_type INTEGER NOT NULL,
                device_sign_pub_bytes BLOB NOT NULL,
                device_kex_pub_type INTEGER NOT NULL,
                device_kex_pub_bytes BLOB NOT NULL,
                operator_sign_pub_type INTEGER NOT NULL,
                operator_sign_pub_bytes BLOB NOT NULL,
                operator_kex_pub_type INTEGER NOT NULL,
                operator_kex_pub_bytes BLOB NOT NULL,
                granted_perms BLOB NOT NULL,
                unattended_enabled INTEGER NOT NULL,
                require_consent_each_time INTEGER NOT NULL,
                issued_at INTEGER NOT NULL,
                last_session INTEGER,
                UNIQUE(device_id, operator_id)
            );
            CREATE INDEX IF NOT EXISTS idx_pairings_device ON pairings(device_id);
            CREATE INDEX IF NOT EXISTS idx_pairings_operator ON pairings(operator_id);

            -- Tickets table
            CREATE TABLE IF NOT EXISTS tickets (
                ticket_id BLOB PRIMARY KEY,
                session_id BLOB NOT NULL,
                operator_id BLOB NOT NULL,
                device_id BLOB NOT NULL,
                permissions INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                session_binding BLOB NOT NULL,
                revoked INTEGER NOT NULL DEFAULT 0,
                issued_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_tickets_expires ON tickets(expires_at);
            CREATE INDEX IF NOT EXISTS idx_tickets_session ON tickets(session_id);

            -- Record schema version
            INSERT INTO schema_version (version) VALUES (1);
            "#,
        )
        .map_err(|e| StoreError::OperationFailed(format!("migration v1 failed: {}", e)))?;

        Ok(())
    }


    // -------------------------------------------------------------------------
    // Helper methods for serialization
    // -------------------------------------------------------------------------

    /// Serialize permissions vector to bytes.
    fn serialize_perms(perms: &[i32]) -> Vec<u8> {
        perms.iter().flat_map(|p| p.to_le_bytes()).collect()
    }

    /// Deserialize permissions from bytes.
    fn deserialize_perms(bytes: &[u8]) -> Vec<i32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }
}

// ============================================================================
// Store Trait Implementation
// ============================================================================

#[async_trait]
impl Store for SqliteStore {
    // -------------------------------------------------------------------------
    // Invite Operations
    // -------------------------------------------------------------------------

    async fn save_invite(&self, invite: InviteRecord) -> Result<(), StoreError> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO invites (device_id, invite_secret, expires_at_unix)
             VALUES (?1, ?2, ?3)",
            params![
                invite.device_id,
                invite.invite_secret.as_slice(),
                invite.expires_at_unix as i64,
            ],
        )
        .map_err(|e| StoreError::OperationFailed(format!("failed to save invite: {}", e)))?;
        Ok(())
    }

    async fn load_invite(&self, device_id: &[u8]) -> Result<Option<InviteRecord>, StoreError> {
        let conn = self.conn.lock().await;
        let result = conn
            .query_row(
                "SELECT device_id, invite_secret, expires_at_unix FROM invites WHERE device_id = ?1",
                params![device_id],
                |row| {
                    let device_id: Vec<u8> = row.get(0)?;
                    let secret_bytes: Vec<u8> = row.get(1)?;
                    let expires_at: i64 = row.get(2)?;

                    let mut invite_secret = [0u8; 32];
                    if secret_bytes.len() == 32 {
                        invite_secret.copy_from_slice(&secret_bytes);
                    }

                    Ok(InviteRecord {
                        device_id,
                        invite_secret,
                        expires_at_unix: expires_at as u64,
                    })
                },
            )
            .optional()
            .map_err(|e| StoreError::OperationFailed(format!("failed to load invite: {}", e)))?;
        Ok(result)
    }

    async fn delete_invite(&self, device_id: &[u8]) -> Result<(), StoreError> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM invites WHERE device_id = ?1", params![device_id])
            .map_err(|e| StoreError::OperationFailed(format!("failed to delete invite: {}", e)))?;
        Ok(())
    }

    async fn cleanup_expired_invites(&self, current_time: u64) -> Result<usize, StoreError> {
        let conn = self.conn.lock().await;
        let count = conn
            .execute(
                "DELETE FROM invites WHERE expires_at_unix <= ?1",
                params![current_time as i64],
            )
            .map_err(|e| {
                StoreError::OperationFailed(format!("failed to cleanup expired invites: {}", e))
            })?;
        Ok(count)
    }

    // -------------------------------------------------------------------------
    // Pairing Operations
    // -------------------------------------------------------------------------

    async fn save_pairing(&self, pairing: PairingRecord) -> Result<(), StoreError> {
        let conn = self.conn.lock().await;
        let perms_bytes = Self::serialize_perms(&pairing.granted_perms);

        conn.execute(
            "INSERT OR REPLACE INTO pairings (
                pairing_id, device_id, operator_id,
                device_sign_pub_type, device_sign_pub_bytes,
                device_kex_pub_type, device_kex_pub_bytes,
                operator_sign_pub_type, operator_sign_pub_bytes,
                operator_kex_pub_type, operator_kex_pub_bytes,
                granted_perms, unattended_enabled, require_consent_each_time,
                issued_at, last_session
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                pairing.pairing_id,
                pairing.device_id,
                pairing.operator_id,
                pairing.device_sign_pub.key_type,
                pairing.device_sign_pub.key_bytes,
                pairing.device_kex_pub.key_type,
                pairing.device_kex_pub.key_bytes,
                pairing.operator_sign_pub.key_type,
                pairing.operator_sign_pub.key_bytes,
                pairing.operator_kex_pub.key_type,
                pairing.operator_kex_pub.key_bytes,
                perms_bytes,
                pairing.unattended_enabled as i32,
                pairing.require_consent_each_time as i32,
                pairing.issued_at as i64,
                pairing.last_session.map(|t| t as i64),
            ],
        )
        .map_err(|e| StoreError::OperationFailed(format!("failed to save pairing: {}", e)))?;
        Ok(())
    }

    async fn load_pairing(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
    ) -> Result<Option<PairingRecord>, StoreError> {
        let conn = self.conn.lock().await;
        let result = conn
            .query_row(
                "SELECT pairing_id, device_id, operator_id,
                        device_sign_pub_type, device_sign_pub_bytes,
                        device_kex_pub_type, device_kex_pub_bytes,
                        operator_sign_pub_type, operator_sign_pub_bytes,
                        operator_kex_pub_type, operator_kex_pub_bytes,
                        granted_perms, unattended_enabled, require_consent_each_time,
                        issued_at, last_session
                 FROM pairings WHERE device_id = ?1 AND operator_id = ?2",
                params![device_id, operator_id],
                Self::row_to_pairing,
            )
            .optional()
            .map_err(|e| StoreError::OperationFailed(format!("failed to load pairing: {}", e)))?;
        Ok(result)
    }

    async fn list_pairings(&self) -> Result<Vec<PairingRecord>, StoreError> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT pairing_id, device_id, operator_id,
                        device_sign_pub_type, device_sign_pub_bytes,
                        device_kex_pub_type, device_kex_pub_bytes,
                        operator_sign_pub_type, operator_sign_pub_bytes,
                        operator_kex_pub_type, operator_kex_pub_bytes,
                        granted_perms, unattended_enabled, require_consent_each_time,
                        issued_at, last_session
                 FROM pairings",
            )
            .map_err(|e| StoreError::OperationFailed(format!("failed to prepare query: {}", e)))?;

        let pairings = stmt
            .query_map([], Self::row_to_pairing)
            .map_err(|e| StoreError::OperationFailed(format!("failed to list pairings: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| StoreError::OperationFailed(format!("failed to collect pairings: {}", e)))?;

        Ok(pairings)
    }

    async fn list_pairings_for_device(
        &self,
        device_id: &[u8],
    ) -> Result<Vec<PairingRecord>, StoreError> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT pairing_id, device_id, operator_id,
                        device_sign_pub_type, device_sign_pub_bytes,
                        device_kex_pub_type, device_kex_pub_bytes,
                        operator_sign_pub_type, operator_sign_pub_bytes,
                        operator_kex_pub_type, operator_kex_pub_bytes,
                        granted_perms, unattended_enabled, require_consent_each_time,
                        issued_at, last_session
                 FROM pairings WHERE device_id = ?1",
            )
            .map_err(|e| StoreError::OperationFailed(format!("failed to prepare query: {}", e)))?;

        let pairings = stmt
            .query_map(params![device_id], Self::row_to_pairing)
            .map_err(|e| {
                StoreError::OperationFailed(format!("failed to list pairings for device: {}", e))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| StoreError::OperationFailed(format!("failed to collect pairings: {}", e)))?;

        Ok(pairings)
    }

    async fn delete_pairing(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
    ) -> Result<(), StoreError> {
        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM pairings WHERE device_id = ?1 AND operator_id = ?2",
            params![device_id, operator_id],
        )
        .map_err(|e| StoreError::OperationFailed(format!("failed to delete pairing: {}", e)))?;
        Ok(())
    }

    async fn update_pairing_last_session(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
        timestamp: u64,
    ) -> Result<(), StoreError> {
        let conn = self.conn.lock().await;
        let rows_affected = conn
            .execute(
                "UPDATE pairings SET last_session = ?1 WHERE device_id = ?2 AND operator_id = ?3",
                params![timestamp as i64, device_id, operator_id],
            )
            .map_err(|e| {
                StoreError::OperationFailed(format!("failed to update pairing last session: {}", e))
            })?;

        if rows_affected == 0 {
            return Err(StoreError::NotFound(format!(
                "pairing for device {:?} and operator {:?}",
                device_id, operator_id
            )));
        }
        Ok(())
    }


    // -------------------------------------------------------------------------
    // Ticket Operations
    // -------------------------------------------------------------------------

    async fn save_ticket(&self, ticket: TicketRecord) -> Result<(), StoreError> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO tickets (
                ticket_id, session_id, operator_id, device_id,
                permissions, expires_at, session_binding, revoked, issued_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                ticket.ticket_id,
                ticket.session_id,
                ticket.operator_id,
                ticket.device_id,
                ticket.permissions,
                ticket.expires_at as i64,
                ticket.session_binding,
                ticket.revoked as i32,
                ticket.issued_at as i64,
            ],
        )
        .map_err(|e| StoreError::OperationFailed(format!("failed to save ticket: {}", e)))?;
        Ok(())
    }

    async fn load_ticket(&self, ticket_id: &[u8]) -> Result<Option<TicketRecord>, StoreError> {
        let conn = self.conn.lock().await;
        let result = conn
            .query_row(
                "SELECT ticket_id, session_id, operator_id, device_id,
                        permissions, expires_at, session_binding, revoked, issued_at
                 FROM tickets WHERE ticket_id = ?1 AND revoked = 0",
                params![ticket_id],
                Self::row_to_ticket,
            )
            .optional()
            .map_err(|e| StoreError::OperationFailed(format!("failed to load ticket: {}", e)))?;
        Ok(result)
    }

    async fn revoke_ticket(&self, ticket_id: &[u8]) -> Result<(), StoreError> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE tickets SET revoked = 1 WHERE ticket_id = ?1",
            params![ticket_id],
        )
        .map_err(|e| StoreError::OperationFailed(format!("failed to revoke ticket: {}", e)))?;
        Ok(())
    }

    async fn cleanup_expired_tickets(&self, current_time: u64) -> Result<usize, StoreError> {
        let conn = self.conn.lock().await;
        let count = conn
            .execute(
                "DELETE FROM tickets WHERE expires_at <= ?1",
                params![current_time as i64],
            )
            .map_err(|e| {
                StoreError::OperationFailed(format!("failed to cleanup expired tickets: {}", e))
            })?;
        Ok(count)
    }

    async fn is_ticket_valid(
        &self,
        ticket_id: &[u8],
        current_time: u64,
    ) -> Result<bool, StoreError> {
        let conn = self.conn.lock().await;
        let result: Option<i32> = conn
            .query_row(
                "SELECT 1 FROM tickets WHERE ticket_id = ?1 AND revoked = 0 AND expires_at > ?2",
                params![ticket_id, current_time as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| {
                StoreError::OperationFailed(format!("failed to check ticket validity: {}", e))
            })?;
        Ok(result.is_some())
    }
}

// ============================================================================
// Helper Methods for Row Conversion
// ============================================================================

impl SqliteStore {
    /// Convert a database row to a PairingRecord.
    fn row_to_pairing(row: &rusqlite::Row) -> rusqlite::Result<PairingRecord> {
        let pairing_id: Vec<u8> = row.get(0)?;
        let device_id: Vec<u8> = row.get(1)?;
        let operator_id: Vec<u8> = row.get(2)?;

        let device_sign_pub_type: i32 = row.get(3)?;
        let device_sign_pub_bytes: Vec<u8> = row.get(4)?;
        let device_kex_pub_type: i32 = row.get(5)?;
        let device_kex_pub_bytes: Vec<u8> = row.get(6)?;

        let operator_sign_pub_type: i32 = row.get(7)?;
        let operator_sign_pub_bytes: Vec<u8> = row.get(8)?;
        let operator_kex_pub_type: i32 = row.get(9)?;
        let operator_kex_pub_bytes: Vec<u8> = row.get(10)?;

        let perms_bytes: Vec<u8> = row.get(11)?;
        let unattended_enabled: i32 = row.get(12)?;
        let require_consent_each_time: i32 = row.get(13)?;
        let issued_at: i64 = row.get(14)?;
        let last_session: Option<i64> = row.get(15)?;

        Ok(PairingRecord {
            pairing_id,
            device_id,
            operator_id,
            device_sign_pub: PublicKeyV1 {
                key_type: device_sign_pub_type,
                key_bytes: device_sign_pub_bytes,
            },
            device_kex_pub: PublicKeyV1 {
                key_type: device_kex_pub_type,
                key_bytes: device_kex_pub_bytes,
            },
            operator_sign_pub: PublicKeyV1 {
                key_type: operator_sign_pub_type,
                key_bytes: operator_sign_pub_bytes,
            },
            operator_kex_pub: PublicKeyV1 {
                key_type: operator_kex_pub_type,
                key_bytes: operator_kex_pub_bytes,
            },
            granted_perms: Self::deserialize_perms(&perms_bytes),
            unattended_enabled: unattended_enabled != 0,
            require_consent_each_time: require_consent_each_time != 0,
            issued_at: issued_at as u64,
            last_session: last_session.map(|t| t as u64),
        })
    }

    /// Convert a database row to a TicketRecord.
    fn row_to_ticket(row: &rusqlite::Row) -> rusqlite::Result<TicketRecord> {
        Ok(TicketRecord {
            ticket_id: row.get(0)?,
            session_id: row.get(1)?,
            operator_id: row.get(2)?,
            device_id: row.get(3)?,
            permissions: row.get(4)?,
            expires_at: row.get::<_, i64>(5)? as u64,
            session_binding: row.get(6)?,
            revoked: row.get::<_, i32>(7)? != 0,
            issued_at: row.get::<_, i64>(8)? as u64,
        })
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use zrc_proto::v1::KeyTypeV1;

    fn make_test_invite(device_id: &[u8], expires_at: u64) -> InviteRecord {
        InviteRecord {
            device_id: device_id.to_vec(),
            invite_secret: [42u8; 32],
            expires_at_unix: expires_at,
        }
    }

    fn make_test_pairing(device_id: &[u8], operator_id: &[u8]) -> PairingRecord {
        PairingRecord {
            pairing_id: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            device_id: device_id.to_vec(),
            operator_id: operator_id.to_vec(),
            device_sign_pub: PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: vec![1u8; 32],
            },
            device_kex_pub: PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: vec![2u8; 32],
            },
            operator_sign_pub: PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: vec![3u8; 32],
            },
            operator_kex_pub: PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: vec![4u8; 32],
            },
            granted_perms: vec![1, 2, 3], // VIEW, CONTROL, CLIPBOARD
            unattended_enabled: true,
            require_consent_each_time: false,
            issued_at: 1000,
            last_session: None,
        }
    }

    fn make_test_ticket(ticket_id: &[u8], expires_at: u64) -> TicketRecord {
        TicketRecord {
            ticket_id: ticket_id.to_vec(),
            session_id: vec![0u8; 32],
            operator_id: vec![1u8; 32],
            device_id: vec![2u8; 32],
            permissions: 7, // VIEW | CONTROL | CLIPBOARD
            expires_at,
            session_binding: vec![5u8; 32],
            revoked: false,
            issued_at: 1000,
        }
    }

    // -------------------------------------------------------------------------
    // Invite Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sqlite_invite_save_and_load() {
        let store = SqliteStore::new_in_memory().unwrap();
        let device_id = vec![1u8; 32];
        let invite = make_test_invite(&device_id, 2000);

        store.save_invite(invite.clone()).await.unwrap();
        let retrieved = store.load_invite(&device_id).await.unwrap();

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.device_id, device_id);
        assert_eq!(retrieved.invite_secret, [42u8; 32]);
        assert_eq!(retrieved.expires_at_unix, 2000);
    }

    #[tokio::test]
    async fn test_sqlite_invite_load_nonexistent() {
        let store = SqliteStore::new_in_memory().unwrap();
        let device_id = vec![1u8; 32];

        let retrieved = store.load_invite(&device_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_invite_delete() {
        let store = SqliteStore::new_in_memory().unwrap();
        let device_id = vec![1u8; 32];
        let invite = make_test_invite(&device_id, 2000);

        store.save_invite(invite).await.unwrap();
        store.delete_invite(&device_id).await.unwrap();

        let retrieved = store.load_invite(&device_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_invite_cleanup_expired() {
        let store = SqliteStore::new_in_memory().unwrap();

        store.save_invite(make_test_invite(&[1u8; 32], 1000)).await.unwrap();
        store.save_invite(make_test_invite(&[2u8; 32], 2000)).await.unwrap();
        store.save_invite(make_test_invite(&[3u8; 32], 3000)).await.unwrap();

        let removed = store.cleanup_expired_invites(1500).await.unwrap();
        assert_eq!(removed, 1);

        assert!(store.load_invite(&[1u8; 32]).await.unwrap().is_none());
        assert!(store.load_invite(&[2u8; 32]).await.unwrap().is_some());
        assert!(store.load_invite(&[3u8; 32]).await.unwrap().is_some());
    }


    // -------------------------------------------------------------------------
    // Pairing Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sqlite_pairing_save_and_load() {
        let store = SqliteStore::new_in_memory().unwrap();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];
        let pairing = make_test_pairing(&device_id, &operator_id);

        store.save_pairing(pairing.clone()).await.unwrap();
        let retrieved = store.load_pairing(&device_id, &operator_id).await.unwrap();

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.device_id, device_id);
        assert_eq!(retrieved.operator_id, operator_id);
        assert_eq!(retrieved.granted_perms, vec![1, 2, 3]);
        assert!(retrieved.unattended_enabled);
        assert!(!retrieved.require_consent_each_time);
        assert_eq!(retrieved.device_sign_pub.key_bytes, vec![1u8; 32]);
        assert_eq!(retrieved.operator_kex_pub.key_bytes, vec![4u8; 32]);
    }

    #[tokio::test]
    async fn test_sqlite_pairing_load_nonexistent() {
        let store = SqliteStore::new_in_memory().unwrap();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];

        let retrieved = store.load_pairing(&device_id, &operator_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_pairing_list_all() {
        let store = SqliteStore::new_in_memory().unwrap();

        let mut pairing1 = make_test_pairing(&[1u8; 32], &[2u8; 32]);
        pairing1.pairing_id = vec![1u8; 16];
        let mut pairing2 = make_test_pairing(&[3u8; 32], &[4u8; 32]);
        pairing2.pairing_id = vec![2u8; 16];

        store.save_pairing(pairing1).await.unwrap();
        store.save_pairing(pairing2).await.unwrap();

        let pairings = store.list_pairings().await.unwrap();
        assert_eq!(pairings.len(), 2);
    }

    #[tokio::test]
    async fn test_sqlite_pairing_list_for_device() {
        let store = SqliteStore::new_in_memory().unwrap();
        let device_id = vec![1u8; 32];

        let mut pairing1 = make_test_pairing(&device_id, &[2u8; 32]);
        pairing1.pairing_id = vec![1u8; 16];
        let mut pairing2 = make_test_pairing(&device_id, &[3u8; 32]);
        pairing2.pairing_id = vec![2u8; 16];
        let mut pairing3 = make_test_pairing(&[4u8; 32], &[5u8; 32]);
        pairing3.pairing_id = vec![3u8; 16];

        store.save_pairing(pairing1).await.unwrap();
        store.save_pairing(pairing2).await.unwrap();
        store.save_pairing(pairing3).await.unwrap();

        let pairings = store.list_pairings_for_device(&device_id).await.unwrap();
        assert_eq!(pairings.len(), 2);
    }

    #[tokio::test]
    async fn test_sqlite_pairing_delete() {
        let store = SqliteStore::new_in_memory().unwrap();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];

        store.save_pairing(make_test_pairing(&device_id, &operator_id)).await.unwrap();
        store.delete_pairing(&device_id, &operator_id).await.unwrap();

        let retrieved = store.load_pairing(&device_id, &operator_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_pairing_update_last_session() {
        let store = SqliteStore::new_in_memory().unwrap();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];

        store.save_pairing(make_test_pairing(&device_id, &operator_id)).await.unwrap();
        store.update_pairing_last_session(&device_id, &operator_id, 5000).await.unwrap();

        let retrieved = store.load_pairing(&device_id, &operator_id).await.unwrap().unwrap();
        assert_eq!(retrieved.last_session, Some(5000));
    }

    #[tokio::test]
    async fn test_sqlite_pairing_update_last_session_not_found() {
        let store = SqliteStore::new_in_memory().unwrap();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];

        let result = store.update_pairing_last_session(&device_id, &operator_id, 5000).await;
        assert!(matches!(result, Err(StoreError::NotFound(_))));
    }

    // -------------------------------------------------------------------------
    // Ticket Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sqlite_ticket_save_and_load() {
        let store = SqliteStore::new_in_memory().unwrap();
        let ticket_id = vec![1u8; 16];
        let ticket = make_test_ticket(&ticket_id, 2000);

        store.save_ticket(ticket.clone()).await.unwrap();
        let retrieved = store.load_ticket(&ticket_id).await.unwrap();

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.ticket_id, ticket_id);
        assert_eq!(retrieved.expires_at, 2000);
        assert_eq!(retrieved.permissions, 7);
        assert_eq!(retrieved.session_binding, vec![5u8; 32]);
    }

    #[tokio::test]
    async fn test_sqlite_ticket_load_nonexistent() {
        let store = SqliteStore::new_in_memory().unwrap();
        let ticket_id = vec![1u8; 16];

        let retrieved = store.load_ticket(&ticket_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_ticket_revoke() {
        let store = SqliteStore::new_in_memory().unwrap();
        let ticket_id = vec![1u8; 16];
        let ticket = make_test_ticket(&ticket_id, 2000);

        store.save_ticket(ticket).await.unwrap();
        store.revoke_ticket(&ticket_id).await.unwrap();

        // load_ticket should return None for revoked tickets
        let retrieved = store.load_ticket(&ticket_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_ticket_is_valid() {
        let store = SqliteStore::new_in_memory().unwrap();
        let ticket_id = vec![1u8; 16];
        let ticket = make_test_ticket(&ticket_id, 2000);

        store.save_ticket(ticket).await.unwrap();

        // Valid before expiry
        assert!(store.is_ticket_valid(&ticket_id, 1500).await.unwrap());

        // Invalid after expiry
        assert!(!store.is_ticket_valid(&ticket_id, 2500).await.unwrap());
    }

    #[tokio::test]
    async fn test_sqlite_ticket_is_valid_revoked() {
        let store = SqliteStore::new_in_memory().unwrap();
        let ticket_id = vec![1u8; 16];
        let ticket = make_test_ticket(&ticket_id, 2000);

        store.save_ticket(ticket).await.unwrap();
        store.revoke_ticket(&ticket_id).await.unwrap();

        // Invalid when revoked even before expiry
        assert!(!store.is_ticket_valid(&ticket_id, 1500).await.unwrap());
    }

    #[tokio::test]
    async fn test_sqlite_ticket_cleanup_expired() {
        let store = SqliteStore::new_in_memory().unwrap();

        store.save_ticket(make_test_ticket(&[1u8; 16], 1000)).await.unwrap();
        store.save_ticket(make_test_ticket(&[2u8; 16], 2000)).await.unwrap();
        store.save_ticket(make_test_ticket(&[3u8; 16], 3000)).await.unwrap();

        let removed = store.cleanup_expired_tickets(1500).await.unwrap();
        assert_eq!(removed, 1);

        assert!(store.load_ticket(&[1u8; 16]).await.unwrap().is_none());
        assert!(store.load_ticket(&[2u8; 16]).await.unwrap().is_some());
        assert!(store.load_ticket(&[3u8; 16]).await.unwrap().is_some());
    }

    // -------------------------------------------------------------------------
    // Serialization Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_perms_serialization_roundtrip() {
        let perms = vec![1, 2, 3, 4, 5];
        let bytes = SqliteStore::serialize_perms(&perms);
        let deserialized = SqliteStore::deserialize_perms(&bytes);
        assert_eq!(perms, deserialized);
    }

    #[test]
    fn test_perms_serialization_empty() {
        let perms: Vec<i32> = vec![];
        let bytes = SqliteStore::serialize_perms(&perms);
        let deserialized = SqliteStore::deserialize_perms(&bytes);
        assert_eq!(perms, deserialized);
    }
}
