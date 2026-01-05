//! Storage abstraction for ZRC pairings, invites, and tickets.
//!
//! This module defines the `Store` trait and provides an in-memory implementation
//! for testing and MVP use cases.
//!
//! Requirements: 8.1, 8.2, 8.3, 8.4

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::RwLock;

use zrc_proto::v1::{PublicKeyV1, SessionTicketV1};

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during store operations.
/// Requirements: 11.1
#[derive(Debug, Error, Clone)]
pub enum StoreError {
    #[error("record not found: {0}")]
    NotFound(String),

    #[error("record already exists: {0}")]
    AlreadyExists(String),

    #[error("storage operation failed: {0}")]
    OperationFailed(String),

    #[error("data corruption detected: {0}")]
    DataCorruption(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

// ============================================================================
// Data Models
// ============================================================================

/// Record for storing invite data with its secret.
/// Requirements: 8.2
#[derive(Clone, Debug)]
pub struct InviteRecord {
    /// Device identifier (32 bytes)
    pub device_id: Vec<u8>,
    /// Random invite secret (32 bytes) - used for HMAC verification
    pub invite_secret: [u8; 32],
    /// Unix timestamp when invite expires
    pub expires_at_unix: u64,
}


/// Record for storing pairing data between device and operator.
/// Requirements: 8.1, 8.6
#[derive(Clone, Debug)]
pub struct PairingRecord {
    /// Unique pairing identifier (16 bytes)
    pub pairing_id: Vec<u8>,
    /// Device identifier (32 bytes)
    pub device_id: Vec<u8>,
    /// Operator identifier (32 bytes)
    pub operator_id: Vec<u8>,

    /// Device's Ed25519 signing public key
    pub device_sign_pub: PublicKeyV1,
    /// Device's X25519 key exchange public key
    pub device_kex_pub: PublicKeyV1,

    /// Operator's Ed25519 signing public key
    pub operator_sign_pub: PublicKeyV1,
    /// Operator's X25519 key exchange public key
    pub operator_kex_pub: PublicKeyV1,

    /// Granted permissions as i32 values (PermissionV1 enum)
    pub granted_perms: Vec<i32>,
    /// Whether unattended access is enabled
    pub unattended_enabled: bool,
    /// Whether consent is required for each session
    pub require_consent_each_time: bool,

    /// Unix timestamp when pairing was issued
    pub issued_at: u64,
    /// Unix timestamp of last session (optional)
    pub last_session: Option<u64>,
}

/// Record for storing session ticket data.
/// Requirements: 8.3
#[derive(Clone, Debug)]
pub struct TicketRecord {
    /// Unique ticket identifier (16 bytes)
    pub ticket_id: Vec<u8>,
    /// Session identifier (32 bytes)
    pub session_id: Vec<u8>,
    /// Operator identifier (32 bytes)
    pub operator_id: Vec<u8>,
    /// Device identifier (32 bytes)
    pub device_id: Vec<u8>,
    /// Granted permissions bitmask
    pub permissions: u32,
    /// Unix timestamp when ticket expires
    pub expires_at: u64,
    /// Session binding hash
    pub session_binding: Vec<u8>,
    /// Whether ticket has been revoked
    pub revoked: bool,
    /// Unix timestamp when ticket was issued
    pub issued_at: u64,
}

impl From<&SessionTicketV1> for TicketRecord {
    fn from(ticket: &SessionTicketV1) -> Self {
        Self {
            ticket_id: ticket.ticket_id.clone(),
            session_id: ticket.session_id.clone(),
            operator_id: ticket.operator_id.clone(),
            device_id: ticket.device_id.clone(),
            permissions: ticket.permissions,
            expires_at: ticket.expires_at,
            session_binding: ticket.session_binding.clone(),
            revoked: false,
            issued_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}


// ============================================================================
// Store Trait
// ============================================================================

/// Storage abstraction for ZRC data persistence.
///
/// This trait defines async methods for storing and retrieving:
/// - Invites: Out-of-band pairing data
/// - Pairings: Established trust relationships
/// - Tickets: Session capability tokens
///
/// Requirements: 8.1, 8.2, 8.3
#[async_trait]
pub trait Store: Send + Sync {
    // -------------------------------------------------------------------------
    // Invite Operations (Requirements: 8.2)
    // -------------------------------------------------------------------------

    /// Save an invite record with its secret.
    ///
    /// # Arguments
    /// * `invite` - The invite record to save
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(StoreError)` if the operation fails
    async fn save_invite(&self, invite: InviteRecord) -> Result<(), StoreError>;

    /// Retrieve an invite by device ID.
    ///
    /// # Arguments
    /// * `device_id` - The device identifier (32 bytes)
    ///
    /// # Returns
    /// * `Ok(Some(invite))` if found
    /// * `Ok(None)` if not found
    /// * `Err(StoreError)` if the operation fails
    async fn load_invite(&self, device_id: &[u8]) -> Result<Option<InviteRecord>, StoreError>;

    /// Delete an invite by device ID.
    ///
    /// # Arguments
    /// * `device_id` - The device identifier (32 bytes)
    ///
    /// # Returns
    /// * `Ok(())` on success (even if invite didn't exist)
    /// * `Err(StoreError)` if the operation fails
    async fn delete_invite(&self, device_id: &[u8]) -> Result<(), StoreError>;

    /// Clean up expired invites.
    ///
    /// # Arguments
    /// * `current_time` - Current Unix timestamp
    ///
    /// # Returns
    /// * `Ok(count)` - Number of invites deleted
    /// * `Err(StoreError)` if the operation fails
    async fn cleanup_expired_invites(&self, current_time: u64) -> Result<usize, StoreError>;

    // -------------------------------------------------------------------------
    // Pairing Operations (Requirements: 8.1)
    // -------------------------------------------------------------------------

    /// Save a pairing record.
    ///
    /// # Arguments
    /// * `pairing` - The pairing record to save
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(StoreError)` if the operation fails
    async fn save_pairing(&self, pairing: PairingRecord) -> Result<(), StoreError>;

    /// Retrieve a pairing by device and operator IDs.
    ///
    /// # Arguments
    /// * `device_id` - The device identifier (32 bytes)
    /// * `operator_id` - The operator identifier (32 bytes)
    ///
    /// # Returns
    /// * `Ok(Some(pairing))` if found
    /// * `Ok(None)` if not found
    /// * `Err(StoreError)` if the operation fails
    async fn load_pairing(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
    ) -> Result<Option<PairingRecord>, StoreError>;

    /// List all pairings.
    ///
    /// # Returns
    /// * `Ok(pairings)` - Vector of all pairing records
    /// * `Err(StoreError)` if the operation fails
    async fn list_pairings(&self) -> Result<Vec<PairingRecord>, StoreError>;

    /// List pairings for a specific device.
    ///
    /// # Arguments
    /// * `device_id` - The device identifier (32 bytes)
    ///
    /// # Returns
    /// * `Ok(pairings)` - Vector of pairing records for the device
    /// * `Err(StoreError)` if the operation fails
    async fn list_pairings_for_device(
        &self,
        device_id: &[u8],
    ) -> Result<Vec<PairingRecord>, StoreError>;

    /// Delete a pairing by device and operator IDs.
    ///
    /// # Arguments
    /// * `device_id` - The device identifier (32 bytes)
    /// * `operator_id` - The operator identifier (32 bytes)
    ///
    /// # Returns
    /// * `Ok(())` on success (even if pairing didn't exist)
    /// * `Err(StoreError)` if the operation fails
    async fn delete_pairing(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
    ) -> Result<(), StoreError>;

    /// Update the last session timestamp for a pairing.
    ///
    /// # Arguments
    /// * `device_id` - The device identifier (32 bytes)
    /// * `operator_id` - The operator identifier (32 bytes)
    /// * `timestamp` - Unix timestamp of the session
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(StoreError::NotFound)` if pairing doesn't exist
    async fn update_pairing_last_session(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
        timestamp: u64,
    ) -> Result<(), StoreError>;

    // -------------------------------------------------------------------------
    // Ticket Operations (Requirements: 8.3)
    // -------------------------------------------------------------------------

    /// Save a session ticket.
    ///
    /// # Arguments
    /// * `ticket` - The ticket record to save
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(StoreError)` if the operation fails
    async fn save_ticket(&self, ticket: TicketRecord) -> Result<(), StoreError>;

    /// Retrieve a ticket by ticket ID.
    ///
    /// # Arguments
    /// * `ticket_id` - The ticket identifier (16 bytes)
    ///
    /// # Returns
    /// * `Ok(Some(ticket))` if found and not revoked
    /// * `Ok(None)` if not found or revoked
    /// * `Err(StoreError)` if the operation fails
    async fn load_ticket(&self, ticket_id: &[u8]) -> Result<Option<TicketRecord>, StoreError>;

    /// Revoke a ticket by ticket ID.
    ///
    /// # Arguments
    /// * `ticket_id` - The ticket identifier (16 bytes)
    ///
    /// # Returns
    /// * `Ok(())` on success (even if ticket didn't exist)
    /// * `Err(StoreError)` if the operation fails
    async fn revoke_ticket(&self, ticket_id: &[u8]) -> Result<(), StoreError>;

    /// Clean up expired tickets.
    ///
    /// # Arguments
    /// * `current_time` - Current Unix timestamp
    ///
    /// # Returns
    /// * `Ok(count)` - Number of tickets deleted
    /// * `Err(StoreError)` if the operation fails
    async fn cleanup_expired_tickets(&self, current_time: u64) -> Result<usize, StoreError>;

    /// Check if a ticket is valid (exists, not revoked, not expired).
    ///
    /// # Arguments
    /// * `ticket_id` - The ticket identifier (16 bytes)
    /// * `current_time` - Current Unix timestamp
    ///
    /// # Returns
    /// * `Ok(true)` if ticket is valid
    /// * `Ok(false)` if ticket is invalid, revoked, or expired
    /// * `Err(StoreError)` if the operation fails
    async fn is_ticket_valid(
        &self,
        ticket_id: &[u8],
        current_time: u64,
    ) -> Result<bool, StoreError>;
}


// ============================================================================
// In-Memory Store Implementation
// ============================================================================

/// Thread-safe in-memory store implementation for testing and MVP.
///
/// Uses `RwLock` for concurrent access with multiple readers or single writer.
///
/// Requirements: 8.4
#[derive(Default, Clone)]
pub struct InMemoryStore {
    /// Invites indexed by device_id
    invites: Arc<RwLock<HashMap<Vec<u8>, InviteRecord>>>,
    /// Pairings indexed by (device_id, operator_id)
    pairings: Arc<RwLock<HashMap<(Vec<u8>, Vec<u8>), PairingRecord>>>,
    /// Tickets indexed by ticket_id
    tickets: Arc<RwLock<HashMap<Vec<u8>, TicketRecord>>>,
}

/// Type alias for backward compatibility with existing code
pub type MemoryStore = InMemoryStore;

impl InMemoryStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            invites: Arc::new(RwLock::new(HashMap::new())),
            pairings: Arc::new(RwLock::new(HashMap::new())),
            tickets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new in-memory store wrapped in an Arc for sharing.
    pub fn new_shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    // -------------------------------------------------------------------------
    // Backward-compatible methods (legacy API)
    // These methods maintain compatibility with existing code
    // -------------------------------------------------------------------------

    /// Legacy: Save an invite (backward compatible with old API)
    pub async fn put_invite(&self, invite: InviteRecord) {
        let mut invites = self.invites.write().await;
        invites.insert(invite.device_id.clone(), invite);
    }

    /// Legacy: Get an invite (backward compatible with old API)
    /// Returns Option directly instead of Result
    pub async fn get_invite(&self, device_id: &[u8]) -> Option<InviteRecord> {
        let invites = self.invites.read().await;
        invites.get(device_id).cloned()
    }

    /// Legacy: Remove an invite (backward compatible with old API)
    pub async fn remove_invite(&self, device_id: &[u8]) {
        let mut invites = self.invites.write().await;
        invites.remove(device_id);
    }

    /// Legacy: Save a pairing (backward compatible with old API)
    pub async fn put_pairing(&self, pairing: PairingRecord) {
        let mut pairings = self.pairings.write().await;
        let key = (pairing.device_id.clone(), pairing.operator_id.clone());
        pairings.insert(key, pairing);
    }

    /// Legacy: Get a pairing (backward compatible with old API)
    /// Returns Option directly instead of Result
    pub async fn get_pairing(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
    ) -> Option<PairingRecord> {
        let pairings = self.pairings.read().await;
        let key = (device_id.to_vec(), operator_id.to_vec());
        pairings.get(&key).cloned()
    }

    /// Legacy: Check if paired (backward compatible with old API)
    pub async fn is_paired(&self, device_id: &[u8], operator_id: &[u8]) -> bool {
        self.get_pairing(device_id, operator_id).await.is_some()
    }

    /// Legacy: Get a ticket (backward compatible with old API)
    /// Returns Option directly instead of Result
    pub async fn get_ticket(&self, ticket_id: &[u8]) -> Option<TicketRecord> {
        let tickets = self.tickets.read().await;
        match tickets.get(ticket_id) {
            Some(ticket) if !ticket.revoked => Some(ticket.clone()),
            _ => None,
        }
    }
}

#[async_trait]
impl Store for InMemoryStore {
    // -------------------------------------------------------------------------
    // Invite Operations
    // -------------------------------------------------------------------------

    async fn save_invite(&self, invite: InviteRecord) -> Result<(), StoreError> {
        let mut invites = self.invites.write().await;
        invites.insert(invite.device_id.clone(), invite);
        Ok(())
    }

    async fn load_invite(&self, device_id: &[u8]) -> Result<Option<InviteRecord>, StoreError> {
        let invites = self.invites.read().await;
        Ok(invites.get(device_id).cloned())
    }

    async fn delete_invite(&self, device_id: &[u8]) -> Result<(), StoreError> {
        let mut invites = self.invites.write().await;
        invites.remove(device_id);
        Ok(())
    }

    async fn cleanup_expired_invites(&self, current_time: u64) -> Result<usize, StoreError> {
        let mut invites = self.invites.write().await;
        let before_count = invites.len();
        invites.retain(|_, invite| invite.expires_at_unix > current_time);
        Ok(before_count - invites.len())
    }

    // -------------------------------------------------------------------------
    // Pairing Operations
    // -------------------------------------------------------------------------

    async fn save_pairing(&self, pairing: PairingRecord) -> Result<(), StoreError> {
        let mut pairings = self.pairings.write().await;
        let key = (pairing.device_id.clone(), pairing.operator_id.clone());
        pairings.insert(key, pairing);
        Ok(())
    }

    async fn load_pairing(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
    ) -> Result<Option<PairingRecord>, StoreError> {
        let pairings = self.pairings.read().await;
        let key = (device_id.to_vec(), operator_id.to_vec());
        Ok(pairings.get(&key).cloned())
    }

    async fn list_pairings(&self) -> Result<Vec<PairingRecord>, StoreError> {
        let pairings = self.pairings.read().await;
        Ok(pairings.values().cloned().collect())
    }

    async fn list_pairings_for_device(
        &self,
        device_id: &[u8],
    ) -> Result<Vec<PairingRecord>, StoreError> {
        let pairings = self.pairings.read().await;
        Ok(pairings
            .values()
            .filter(|p| p.device_id == device_id)
            .cloned()
            .collect())
    }

    async fn delete_pairing(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
    ) -> Result<(), StoreError> {
        let mut pairings = self.pairings.write().await;
        let key = (device_id.to_vec(), operator_id.to_vec());
        pairings.remove(&key);
        Ok(())
    }

    async fn update_pairing_last_session(
        &self,
        device_id: &[u8],
        operator_id: &[u8],
        timestamp: u64,
    ) -> Result<(), StoreError> {
        let mut pairings = self.pairings.write().await;
        let key = (device_id.to_vec(), operator_id.to_vec());
        match pairings.get_mut(&key) {
            Some(pairing) => {
                pairing.last_session = Some(timestamp);
                Ok(())
            }
            None => Err(StoreError::NotFound(format!(
                "pairing for device {:?} and operator {:?}",
                device_id, operator_id
            ))),
        }
    }

    // -------------------------------------------------------------------------
    // Ticket Operations
    // -------------------------------------------------------------------------

    async fn save_ticket(&self, ticket: TicketRecord) -> Result<(), StoreError> {
        let mut tickets = self.tickets.write().await;
        tickets.insert(ticket.ticket_id.clone(), ticket);
        Ok(())
    }

    async fn load_ticket(&self, ticket_id: &[u8]) -> Result<Option<TicketRecord>, StoreError> {
        let tickets = self.tickets.read().await;
        match tickets.get(ticket_id) {
            Some(ticket) if !ticket.revoked => Ok(Some(ticket.clone())),
            _ => Ok(None),
        }
    }

    async fn revoke_ticket(&self, ticket_id: &[u8]) -> Result<(), StoreError> {
        let mut tickets = self.tickets.write().await;
        if let Some(ticket) = tickets.get_mut(ticket_id) {
            ticket.revoked = true;
        }
        Ok(())
    }

    async fn cleanup_expired_tickets(&self, current_time: u64) -> Result<usize, StoreError> {
        let mut tickets = self.tickets.write().await;
        let before_count = tickets.len();
        tickets.retain(|_, ticket| ticket.expires_at > current_time);
        Ok(before_count - tickets.len())
    }

    async fn is_ticket_valid(
        &self,
        ticket_id: &[u8],
        current_time: u64,
    ) -> Result<bool, StoreError> {
        let tickets = self.tickets.read().await;
        match tickets.get(ticket_id) {
            Some(ticket) => Ok(!ticket.revoked && ticket.expires_at > current_time),
            None => Ok(false),
        }
    }
}


// ============================================================================
// Helper Functions
// ============================================================================

/// Check if a device and operator are paired.
///
/// Convenience function that wraps `Store::load_pairing`.
pub async fn is_paired(
    store: &dyn Store,
    device_id: &[u8],
    operator_id: &[u8],
) -> Result<bool, StoreError> {
    Ok(store.load_pairing(device_id, operator_id).await?.is_some())
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
            invite_secret: [0u8; 32],
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
                key_bytes: vec![0u8; 32],
            },
            device_kex_pub: PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: vec![0u8; 32],
            },
            operator_sign_pub: PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: vec![0u8; 32],
            },
            operator_kex_pub: PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: vec![0u8; 32],
            },
            granted_perms: vec![1, 2], // VIEW, CONTROL
            unattended_enabled: false,
            require_consent_each_time: true,
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
            permissions: 3, // VIEW | CONTROL
            expires_at,
            session_binding: vec![0u8; 32],
            revoked: false,
            issued_at: 1000,
        }
    }

    // -------------------------------------------------------------------------
    // Invite Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_invite_save_and_get() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];
        let invite = make_test_invite(&device_id, 2000);

        store.put_invite(invite.clone()).await;
        let retrieved = store.get_invite(&device_id).await;

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.device_id, device_id);
        assert_eq!(retrieved.expires_at_unix, 2000);
    }

    #[tokio::test]
    async fn test_invite_get_nonexistent() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];

        let retrieved = store.get_invite(&device_id).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_invite_delete() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];
        let invite = make_test_invite(&device_id, 2000);

        store.put_invite(invite).await;
        store.remove_invite(&device_id).await;

        let retrieved = store.get_invite(&device_id).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_invite_cleanup_expired() {
        let store = InMemoryStore::new();

        // Save invites with different expiry times
        store.put_invite(make_test_invite(&[1u8; 32], 1000)).await;
        store.put_invite(make_test_invite(&[2u8; 32], 2000)).await;
        store.put_invite(make_test_invite(&[3u8; 32], 3000)).await;

        // Cleanup at time 1500 - should remove first invite
        let removed = store.cleanup_expired_invites(1500).await.unwrap();
        assert_eq!(removed, 1);

        // Verify remaining invites
        assert!(store.get_invite(&[1u8; 32]).await.is_none());
        assert!(store.get_invite(&[2u8; 32]).await.is_some());
        assert!(store.get_invite(&[3u8; 32]).await.is_some());
    }

    // -------------------------------------------------------------------------
    // Pairing Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_pairing_save_and_get() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];
        let pairing = make_test_pairing(&device_id, &operator_id);

        store.put_pairing(pairing.clone()).await;
        let retrieved = store.get_pairing(&device_id, &operator_id).await;

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.device_id, device_id);
        assert_eq!(retrieved.operator_id, operator_id);
    }

    #[tokio::test]
    async fn test_pairing_get_nonexistent() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];

        let retrieved = store.get_pairing(&device_id, &operator_id).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_pairing_list_all() {
        let store = InMemoryStore::new();

        store.put_pairing(make_test_pairing(&[1u8; 32], &[2u8; 32])).await;
        store.put_pairing(make_test_pairing(&[3u8; 32], &[4u8; 32])).await;

        let pairings = store.list_pairings().await.unwrap();
        assert_eq!(pairings.len(), 2);
    }

    #[tokio::test]
    async fn test_pairing_list_for_device() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];

        store.put_pairing(make_test_pairing(&device_id, &[2u8; 32])).await;
        store.put_pairing(make_test_pairing(&device_id, &[3u8; 32])).await;
        store.put_pairing(make_test_pairing(&[4u8; 32], &[5u8; 32])).await;

        let pairings = store.list_pairings_for_device(&device_id).await.unwrap();
        assert_eq!(pairings.len(), 2);
    }

    #[tokio::test]
    async fn test_pairing_delete() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];

        store.put_pairing(make_test_pairing(&device_id, &operator_id)).await;
        store.delete_pairing(&device_id, &operator_id).await.unwrap();

        let retrieved = store.get_pairing(&device_id, &operator_id).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_pairing_update_last_session() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];

        store.put_pairing(make_test_pairing(&device_id, &operator_id)).await;
        store.update_pairing_last_session(&device_id, &operator_id, 5000).await.unwrap();

        let retrieved = store.get_pairing(&device_id, &operator_id).await.unwrap();
        assert_eq!(retrieved.last_session, Some(5000));
    }

    #[tokio::test]
    async fn test_pairing_update_last_session_not_found() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];

        let result = store.update_pairing_last_session(&device_id, &operator_id, 5000).await;
        assert!(matches!(result, Err(StoreError::NotFound(_))));
    }

    // -------------------------------------------------------------------------
    // Ticket Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_ticket_save_and_get() {
        let store = InMemoryStore::new();
        let ticket_id = vec![1u8; 16];
        let ticket = make_test_ticket(&ticket_id, 2000);

        store.save_ticket(ticket.clone()).await.unwrap();
        let retrieved = store.get_ticket(&ticket_id).await;

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.ticket_id, ticket_id);
        assert_eq!(retrieved.expires_at, 2000);
    }

    #[tokio::test]
    async fn test_ticket_get_nonexistent() {
        let store = InMemoryStore::new();
        let ticket_id = vec![1u8; 16];

        let retrieved = store.get_ticket(&ticket_id).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_ticket_revoke() {
        let store = InMemoryStore::new();
        let ticket_id = vec![1u8; 16];
        let ticket = make_test_ticket(&ticket_id, 2000);

        store.save_ticket(ticket).await.unwrap();
        store.revoke_ticket(&ticket_id).await.unwrap();

        // get_ticket should return None for revoked tickets
        let retrieved = store.get_ticket(&ticket_id).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_ticket_is_valid() {
        let store = InMemoryStore::new();
        let ticket_id = vec![1u8; 16];
        let ticket = make_test_ticket(&ticket_id, 2000);

        store.save_ticket(ticket).await.unwrap();

        // Valid before expiry
        assert!(store.is_ticket_valid(&ticket_id, 1500).await.unwrap());

        // Invalid after expiry
        assert!(!store.is_ticket_valid(&ticket_id, 2500).await.unwrap());
    }

    #[tokio::test]
    async fn test_ticket_is_valid_revoked() {
        let store = InMemoryStore::new();
        let ticket_id = vec![1u8; 16];
        let ticket = make_test_ticket(&ticket_id, 2000);

        store.save_ticket(ticket).await.unwrap();
        store.revoke_ticket(&ticket_id).await.unwrap();

        // Invalid when revoked even before expiry
        assert!(!store.is_ticket_valid(&ticket_id, 1500).await.unwrap());
    }

    #[tokio::test]
    async fn test_ticket_cleanup_expired() {
        let store = InMemoryStore::new();

        store.save_ticket(make_test_ticket(&[1u8; 16], 1000)).await.unwrap();
        store.save_ticket(make_test_ticket(&[2u8; 16], 2000)).await.unwrap();
        store.save_ticket(make_test_ticket(&[3u8; 16], 3000)).await.unwrap();

        let removed = store.cleanup_expired_tickets(1500).await.unwrap();
        assert_eq!(removed, 1);

        assert!(store.get_ticket(&[1u8; 16]).await.is_none());
        assert!(store.get_ticket(&[2u8; 16]).await.is_some());
        assert!(store.get_ticket(&[3u8; 16]).await.is_some());
    }

    // -------------------------------------------------------------------------
    // Helper Function Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_is_paired_helper() {
        let store = InMemoryStore::new();
        let device_id = vec![1u8; 32];
        let operator_id = vec![2u8; 32];

        // Not paired initially
        assert!(!is_paired(&store, &device_id, &operator_id).await.unwrap());

        // Paired after saving
        store.put_pairing(make_test_pairing(&device_id, &operator_id)).await;
        assert!(is_paired(&store, &device_id, &operator_id).await.unwrap());
    }
}
