//! Relay token validation and verification

use dashmap::DashMap;
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Sha256, Digest};
use thiserror::Error;

/// Relay token for allocation authorization
#[derive(Debug, Clone)]
pub struct RelayTokenV1 {
    pub relay_id: [u8; 16],
    pub allocation_id: [u8; 16],
    pub device_id: [u8; 32],
    pub peer_id: [u8; 32],
    pub expires_at: u64,
    pub bandwidth_limit: u32,  // bytes/sec
    pub quota_bytes: u64,
    pub signature: [u8; 64],   // Ed25519 by device
}

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Token expired")]
    Expired,
    #[error("Allocation ID mismatch")]
    AllocationIdMismatch,
    #[error("Invalid public key")]
    InvalidPublicKey,
}

impl RelayTokenV1 {
    /// Verify token signature
    pub fn verify(&self, device_pub: &[u8; 32]) -> Result<(), TokenError> {
        if device_pub.len() != 32 {
            return Err(TokenError::InvalidPublicKey);
        }

        let verifying_key = VerifyingKey::from_bytes(
            device_pub.try_into().map_err(|_| TokenError::InvalidPublicKey)?
        ).map_err(|_| TokenError::InvalidPublicKey)?;

        let signature = Signature::from_bytes(&self.signature);

        let message = self.signature_input();
        verifying_key.verify_strict(&message, &signature)
            .map_err(|_| TokenError::InvalidSignature)
    }

    /// Check if token is expired
    pub fn is_expired(&self, now: u64) -> bool {
        self.expires_at <= now
    }

    /// Verify allocation_id matches
    pub fn verify_allocation_id(&self, requested_id: &[u8; 16]) -> Result<(), TokenError> {
        if &self.allocation_id != requested_id {
            return Err(TokenError::AllocationIdMismatch);
        }
        Ok(())
    }

    /// Compute signature input (all fields except signature)
    fn signature_input(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(&self.relay_id);
        hasher.update(&self.allocation_id);
        hasher.update(&self.device_id);
        hasher.update(&self.peer_id);
        hasher.update(&self.expires_at.to_le_bytes());
        hasher.update(&self.bandwidth_limit.to_le_bytes());
        hasher.update(&self.quota_bytes.to_le_bytes());
        hasher.finalize().to_vec()
    }
}

/// Cached verified token information
#[derive(Debug, Clone)]
struct VerifiedToken {
    device_id: [u8; 32],
    expires_at: u64,
    verified_at: u64,
}

/// Token verifier with caching support
pub struct TokenVerifier {
    /// Pinned device public keys (optional, for known devices)
    pinned_keys: DashMap<[u8; 32], [u8; 32]>,
    /// Token cache to avoid repeated signature verification
    verified_cache: DashMap<[u8; 16], VerifiedToken>, // Keyed by allocation_id
}

impl TokenVerifier {
    pub fn new() -> Self {
        Self {
            pinned_keys: DashMap::new(),
            verified_cache: DashMap::new(),
        }
    }

    /// Pin device public key
    pub fn pin_device(&self, device_id: [u8; 32], pub_key: [u8; 32]) {
        self.pinned_keys.insert(device_id, pub_key);
    }

    /// Get pinned key for device
    pub fn get_pinned(&self, device_id: &[u8; 32]) -> Option<[u8; 32]> {
        self.pinned_keys.get(device_id).map(|entry| *entry.value())
    }

    /// Verify relay token with caching
    pub fn verify(
        &self,
        token: &RelayTokenV1,
        device_pub: &[u8; 32],
        now: u64,
    ) -> Result<(), TokenError> {
        // Check expiration first (fast path)
        if token.is_expired(now) {
            // Remove from cache if expired
            self.verified_cache.remove(&token.allocation_id);
            return Err(TokenError::Expired);
        }

        // Check cache
        if let Some(cached) = self.verified_cache.get(&token.allocation_id) {
            let cached = cached.value();
            // Verify it's the same device and still valid
            if cached.device_id == token.device_id
                && cached.expires_at == token.expires_at
                && !token.is_expired(now)
            {
                // Cache hit - skip signature verification
                return Ok(());
            } else {
                // Cache entry is stale, remove it
                self.verified_cache.remove(&token.allocation_id);
            }
        }

        // Verify signature
        token.verify(device_pub)?;

        // Cache the verified token
        self.verified_cache.insert(
            token.allocation_id,
            VerifiedToken {
                device_id: token.device_id,
                expires_at: token.expires_at,
                verified_at: now,
            },
        );

        Ok(())
    }

    /// Verify token and check allocation_id match
    pub fn verify_with_allocation_id(
        &self,
        token: &RelayTokenV1,
        device_pub: &[u8; 32],
        requested_id: &[u8; 16],
        now: u64,
    ) -> Result<(), TokenError> {
        // Verify allocation_id matches
        token.verify_allocation_id(requested_id)?;

        // Verify token
        self.verify(token, device_pub, now)
    }

    /// Refresh token (accept new token for same allocation)
    pub fn refresh_token(
        &self,
        old_token: &RelayTokenV1,
        new_token: &RelayTokenV1,
        device_pub: &[u8; 32],
        now: u64,
    ) -> Result<(), TokenError> {
        // Verify new token
        self.verify(new_token, device_pub, now)?;

        // Verify it's for the same allocation
        if old_token.allocation_id != new_token.allocation_id {
            return Err(TokenError::AllocationIdMismatch);
        }

        // Verify it's from the same device
        if old_token.device_id != new_token.device_id {
            return Err(TokenError::InvalidSignature);
        }

        // Update cache with new token
        self.verified_cache.insert(
            new_token.allocation_id,
            VerifiedToken {
                device_id: new_token.device_id,
                expires_at: new_token.expires_at,
                verified_at: now,
            },
        );

        Ok(())
    }

    /// Clean up expired cache entries
    pub fn cleanup_expired(&self, now: u64) {
        self.verified_cache.retain(|_id, token| {
            token.expires_at > now
        });
    }

    /// Clear cache for a specific allocation
    pub fn clear_cache(&self, allocation_id: &[u8; 16]) {
        self.verified_cache.remove(allocation_id);
    }
}

impl Default for TokenVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use ed25519_dalek::{SigningKey, VerifyingKey, Signer};
    use rand_core::OsRng;

    /// Property 1: Token Signature Verification
    /// Validates: Requirements 1.2, 1.3
    /// 
    /// Property: For any valid token signed with a private key,
    /// verification with the corresponding public key must succeed.
    /// For any token with an invalid signature or wrong public key,
    /// verification must fail.
    #[test]
    fn prop_token_signature_verification() {
        proptest!(|(
            relay_id in prop::array::uniform32(0u8..=255u8),
            allocation_id in prop::array::uniform16(0u8..=255u8),
            device_id in prop::array::uniform32(0u8..=255u8),
            peer_id in prop::array::uniform32(0u8..=255u8),
            expires_at in 0u64..=u64::MAX,
            bandwidth_limit in 1u32..=100_000_000u32,
            quota_bytes in 1u64..=10_000_000_000u64,
        )| {
            // Generate keypair
            let signing_key = SigningKey::generate(&mut OsRng);
            let verifying_key = signing_key.verifying_key();
            let device_pub = verifying_key.to_bytes();

            // Create token
            let mut token = RelayTokenV1 {
                relay_id: relay_id[..16].try_into().unwrap(),
                allocation_id,
                device_id,
                peer_id,
                expires_at,
                bandwidth_limit,
                quota_bytes,
                signature: [0u8; 64],
            };

            // Sign token
            let message = token.signature_input();
            let signature = signing_key.sign(&message);
            token.signature = signature.to_bytes();

            // Verify with correct key - must succeed
            prop_assert!(token.verify(&device_pub).is_ok());

            // Verify with wrong key - must fail
            let wrong_key = SigningKey::generate(&mut OsRng).verifying_key().to_bytes();
            prop_assert!(token.verify(&wrong_key).is_err());

            // Corrupt signature - must fail
            let mut corrupted_token = token.clone();
            corrupted_token.signature[0] = corrupted_token.signature[0].wrapping_add(1);
            prop_assert!(corrupted_token.verify(&device_pub).is_err());
        });
    }
}
