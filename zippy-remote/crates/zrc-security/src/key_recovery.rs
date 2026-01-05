//! Key compromise recovery mechanisms.
//!
//! Requirements: 5.1, 5.2, 5.3, 5.5, 5.6

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use crate::error::SecurityError;
use crate::identity::{IdentityVerifier, PeerId};

/// Key rotation history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationEntry {
    /// When the rotation occurred
    pub rotated_at: u64,
    /// Previous key ID (hash of old public key)
    pub previous_key_id: Vec<u8>,
    /// New key ID (hash of new public key)
    pub new_key_id: Vec<u8>,
    /// Reason for rotation
    pub reason: String,
}

/// Key rotation manager.
///
/// Manages key rotation for devices and operators, including history
/// and propagation to paired endpoints.
///
/// Requirements: 5.1, 5.2, 5.3, 5.6
pub struct KeyRotationManager {
    /// Rotation history per peer ID
    rotation_history: HashMap<PeerId, Vec<KeyRotationEntry>>,
    /// Identity verifier (to update pinned keys)
    identity_verifier: IdentityVerifier,
}

impl KeyRotationManager {
    /// Create a new key rotation manager.
    pub fn new(identity_verifier: IdentityVerifier) -> Self {
        Self {
            rotation_history: HashMap::new(),
            identity_verifier,
        }
    }

    /// Rotate keys for a device or operator.
    ///
    /// Requirements: 5.1, 5.2
    pub fn rotate_keys(
        &mut self,
        peer_id: PeerId,
        old_key_id: Vec<u8>,
        new_key_id: Vec<u8>,
        reason: String,
    ) -> Result<(), SecurityError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| SecurityError::KeyRotationFailed {
                reason: format!("System time error: {}", e),
            })?
            .as_secs();

        let entry = KeyRotationEntry {
            rotated_at: now,
            previous_key_id: old_key_id,
            new_key_id: new_key_id.clone(),
            reason,
        };

        // Add to history
        self.rotation_history
            .entry(peer_id)
            .or_insert_with(Vec::new)
            .push(entry);

        // Unpin old identity (will need to re-pair with new key)
        self.identity_verifier.unpin_identity(&peer_id);

        Ok(())
    }

    /// Get rotation history for a peer.
    ///
    /// Requirements: 5.6
    pub fn get_rotation_history(&self, peer_id: &PeerId) -> Vec<KeyRotationEntry> {
        self.rotation_history
            .get(peer_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all peers that need to be notified of key rotation.
    ///
    /// Requirements: 5.3
    pub fn get_paired_peers(&self) -> Vec<PeerId> {
        self.identity_verifier.pinned_peers()
    }

    /// Get the identity verifier (for updating pinned keys after rotation).
    pub fn identity_verifier(&mut self) -> &mut IdentityVerifier {
        &mut self.identity_verifier
    }
}

/// Emergency key revocation.
///
/// Allows immediate revocation of compromised keys without waiting
/// for normal rotation procedures.
///
/// Requirements: 5.5
pub struct EmergencyRevocation {
    /// Revoked key IDs
    revoked_keys: std::collections::HashSet<Vec<u8>>,
}

impl EmergencyRevocation {
    /// Create a new emergency revocation manager.
    pub fn new() -> Self {
        Self {
            revoked_keys: std::collections::HashSet::new(),
        }
    }

    /// Revoke a key immediately.
    ///
    /// Requirements: 5.5
    pub fn revoke_key(&mut self, key_id: Vec<u8>) {
        self.revoked_keys.insert(key_id);
    }

    /// Check if a key is revoked.
    pub fn is_revoked(&self, key_id: &[u8]) -> bool {
        self.revoked_keys.contains(key_id)
    }

    /// Get all revoked key IDs.
    pub fn revoked_keys(&self) -> &std::collections::HashSet<Vec<u8>> {
        &self.revoked_keys
    }
}

impl Default for EmergencyRevocation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_peer_id(id: u8) -> PeerId {
        [id; 32]
    }

    #[test]
    fn test_key_rotation() {
        let identity_verifier = IdentityVerifier::new();
        let mut rotation_manager = KeyRotationManager::new(identity_verifier);

        let peer_id = make_test_peer_id(1);
        let old_key_id = vec![0xAA; 32];
        let new_key_id = vec![0xBB; 32];

        // Rotate keys
        assert!(rotation_manager.rotate_keys(
            peer_id,
            old_key_id.clone(),
            new_key_id.clone(),
            "test rotation".to_string(),
        ).is_ok());

        // Check history
        let history = rotation_manager.get_rotation_history(&peer_id);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].previous_key_id, old_key_id);
        assert_eq!(history[0].new_key_id, new_key_id);
    }

    #[test]
    fn test_emergency_revocation() {
        let mut revocation = EmergencyRevocation::new();
        let key_id = vec![0xAA; 32];

        // Key should not be revoked initially
        assert!(!revocation.is_revoked(&key_id));

        // Revoke the key
        revocation.revoke_key(key_id.clone());

        // Key should now be revoked
        assert!(revocation.is_revoked(&key_id));
    }
}
