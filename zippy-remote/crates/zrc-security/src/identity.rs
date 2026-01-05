//! Identity pinning and verification for MITM protection.
//!
//! Requirements: 2.1, 2.2, 2.4

use std::collections::HashMap;
use std::time::SystemTime;
use constant_time_eq::constant_time_eq;
use zrc_proto::v1::PublicKeyBundleV1;
use crate::error::SecurityError;

/// A peer ID is a 32-byte identifier derived from the signing public key.
pub type PeerId = [u8; 32];

/// Pinned identity information for a peer.
#[derive(Debug, Clone)]
pub struct PinnedIdentity {
    /// Ed25519 signing public key (32 bytes)
    pub sign_pub: [u8; 32],
    /// X25519 key exchange public key (32 bytes)
    pub kex_pub: [u8; 32],
    /// When this identity was first pinned
    pub pinned_at: SystemTime,
    /// When this identity was last verified
    pub last_verified: SystemTime,
}

/// Identity verifier for MITM protection.
///
/// Maintains a database of pinned identities and verifies that peers
/// present the same keys on every connection after initial pairing.
///
/// Requirements: 2.1, 2.2, 2.4
pub struct IdentityVerifier {
    /// Map from peer ID to pinned identity
    pinned_keys: HashMap<PeerId, PinnedIdentity>,
}

impl IdentityVerifier {
    /// Create a new identity verifier.
    pub fn new() -> Self {
        Self {
            pinned_keys: HashMap::new(),
        }
    }

    /// Verify that a peer's presented keys match the pinned identity.
    ///
    /// Uses constant-time comparison to prevent timing attacks.
    ///
    /// Requirements: 2.2, 2.4
    pub fn verify_identity(
        &mut self,
        peer_id: &PeerId,
        presented_keys: &PublicKeyBundleV1,
    ) -> Result<(), SecurityError> {
        // Extract keys, ensuring they are 32 bytes
        let sign_pub = presented_keys.sign_pub.as_slice();
        let kex_pub = presented_keys.kex_pub.as_slice();

        if sign_pub.len() != 32 {
            return Err(SecurityError::InvalidKeyLength {
                expected: 32,
                got: sign_pub.len(),
            });
        }
        if kex_pub.len() != 32 {
            return Err(SecurityError::InvalidKeyLength {
                expected: 32,
                got: kex_pub.len(),
            });
        }

        // Safe conversion: we've already validated length is 32
        let sign_pub_array: [u8; 32] = sign_pub.try_into()
            .map_err(|_| SecurityError::InvalidKeyLength {
                expected: 32,
                got: sign_pub.len(),
            })?;
        let kex_pub_array: [u8; 32] = kex_pub.try_into()
            .map_err(|_| SecurityError::InvalidKeyLength {
                expected: 32,
                got: kex_pub.len(),
            })?;

        let pinned = self.pinned_keys.get(peer_id)
            .ok_or_else(|| SecurityError::UnknownPeer {
                peer_id: peer_id.to_vec(),
            })?;

        // Constant-time comparison to prevent timing attacks
        if !constant_time_eq(&pinned.sign_pub, &sign_pub_array) {
            return Err(SecurityError::IdentityMismatch {
                peer_id: peer_id.to_vec(),
                expected: hex::encode(&pinned.sign_pub[..8]),
                received: hex::encode(&sign_pub_array[..8]),
            });
        }

        if !constant_time_eq(&pinned.kex_pub, &kex_pub_array) {
            return Err(SecurityError::IdentityMismatch {
                peer_id: peer_id.to_vec(),
                expected: hex::encode(&pinned.kex_pub[..8]),
                received: hex::encode(&kex_pub_array[..8]),
            });
        }

        // Update last verified time
        if let Some(pinned) = self.pinned_keys.get_mut(peer_id) {
            pinned.last_verified = SystemTime::now();
        }

        Ok(())
    }

    /// Pin a new identity (first contact or after SAS verification).
    ///
    /// Requirements: 2.1
    pub fn pin_identity(
        &mut self,
        peer_id: PeerId,
        keys: PublicKeyBundleV1,
    ) -> Result<(), SecurityError> {
        // Extract keys, ensuring they are 32 bytes
        let sign_pub = keys.sign_pub.as_slice();
        let kex_pub = keys.kex_pub.as_slice();

        if sign_pub.len() != 32 {
            return Err(SecurityError::InvalidKeyLength {
                expected: 32,
                got: sign_pub.len(),
            });
        }
        if kex_pub.len() != 32 {
            return Err(SecurityError::InvalidKeyLength {
                expected: 32,
                got: kex_pub.len(),
            });
        }

        let sign_pub_array: [u8; 32] = sign_pub.try_into()
            .map_err(|_| SecurityError::InvalidKeyLength {
                expected: 32,
                got: sign_pub.len(),
            })?;
        let kex_pub_array: [u8; 32] = kex_pub.try_into()
            .map_err(|_| SecurityError::InvalidKeyLength {
                expected: 32,
                got: kex_pub.len(),
            })?;

        let now = SystemTime::now();
        let pinned = PinnedIdentity {
            sign_pub: sign_pub_array,
            kex_pub: kex_pub_array,
            pinned_at: now,
            last_verified: now,
        };

        self.pinned_keys.insert(peer_id, pinned);
        Ok(())
    }

    /// Check if a peer ID is already pinned.
    pub fn is_pinned(&self, peer_id: &PeerId) -> bool {
        self.pinned_keys.contains_key(peer_id)
    }

    /// Remove a pinned identity (for key rotation or unpairing).
    pub fn unpin_identity(&mut self, peer_id: &PeerId) -> bool {
        self.pinned_keys.remove(peer_id).is_some()
    }

    /// Get all pinned peer IDs.
    pub fn pinned_peers(&self) -> Vec<PeerId> {
        self.pinned_keys.keys().copied().collect()
    }
}

impl Default for IdentityVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_keys(sign: u8, kex: u8) -> PublicKeyBundleV1 {
        PublicKeyBundleV1 {
            sign_pub: vec![sign; 32],
            kex_pub: vec![kex; 32],
        }
    }

    fn make_test_peer_id(id: u8) -> PeerId {
        [id; 32]
    }

    #[test]
    fn test_pin_and_verify_identity() {
        let mut verifier = IdentityVerifier::new();
        let peer_id = make_test_peer_id(1);
        let keys = make_test_keys(0xAA, 0xBB);

        // Pin the identity
        verifier.pin_identity(peer_id, keys.clone()).unwrap();

        // Verify it matches
        assert!(verifier.verify_identity(&peer_id, &keys).is_ok());
    }

    #[test]
    fn test_verify_rejects_mismatched_keys() {
        let mut verifier = IdentityVerifier::new();
        let peer_id = make_test_peer_id(1);
        let keys1 = make_test_keys(0xAA, 0xBB);
        let keys2 = make_test_keys(0xCC, 0xBB); // Different sign key

        verifier.pin_identity(peer_id, keys1).unwrap();

        // Should reject mismatched keys
        assert!(matches!(
            verifier.verify_identity(&peer_id, &keys2),
            Err(SecurityError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn test_verify_rejects_unknown_peer() {
        let mut verifier = IdentityVerifier::new();
        let peer_id = make_test_peer_id(1);
        let keys = make_test_keys(0xAA, 0xBB);

        // Should reject unknown peer
        assert!(matches!(
            verifier.verify_identity(&peer_id, &keys),
            Err(SecurityError::UnknownPeer { .. })
        ));
    }

    #[test]
    fn test_unpin_identity() {
        let mut verifier = IdentityVerifier::new();
        let peer_id = make_test_peer_id(1);
        let keys = make_test_keys(0xAA, 0xBB);

        verifier.pin_identity(peer_id, keys.clone()).unwrap();
        assert!(verifier.is_pinned(&peer_id));

        verifier.unpin_identity(&peer_id);
        assert!(!verifier.is_pinned(&peer_id));
    }
}
