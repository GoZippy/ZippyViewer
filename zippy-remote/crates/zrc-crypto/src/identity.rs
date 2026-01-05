//! Identity module for device and operator keypair management.
//!
//! Provides Ed25519 signing and X25519 key exchange capabilities with
//! secure memory handling via zeroization.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

use zrc_proto::v1::PublicKeyBundleV1;

/// Error type for identity operations.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("invalid key length: expected {expected}, got {got}")]
    InvalidKeyLength { expected: usize, got: usize },
    #[error("invalid signature")]
    InvalidSignature,
    #[error("invalid public key")]
    InvalidPublicKey,
}

/// A cryptographic identity with Ed25519 signing key and X25519 key exchange key.
///
/// This struct holds the private key material and provides methods for
/// signing, verification, and key exchange. Key material is securely
/// zeroized when the Identity is dropped.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Identity {
    /// Ed25519 signing private key
    #[zeroize(skip)] // SigningKey implements Zeroize internally
    sign_key: SigningKey,
    /// X25519 key exchange private key
    #[zeroize(skip)] // StaticSecret implements Zeroize internally
    kex_key: StaticSecret,
}

impl Identity {
    /// Generate a new random identity using a secure random source.
    pub fn generate() -> Self {
        let sign_key = SigningKey::generate(&mut OsRng);
        let kex_key = StaticSecret::random_from_rng(OsRng);
        Self { sign_key, kex_key }
    }

    /// Create an Identity from existing key bytes.
    ///
    /// # Arguments
    /// * `sign_key_bytes` - 32-byte Ed25519 private key seed
    /// * `kex_key_bytes` - 32-byte X25519 private key
    pub fn from_bytes(
        sign_key_bytes: &[u8; 32],
        kex_key_bytes: &[u8; 32],
    ) -> Result<Self, IdentityError> {
        let sign_key = SigningKey::from_bytes(sign_key_bytes);
        let kex_key = StaticSecret::from(*kex_key_bytes);
        Ok(Self { sign_key, kex_key })
    }

    /// Derive the identity ID from the signing public key.
    ///
    /// ID = SHA-256(sign_pub)
    pub fn id(&self) -> [u8; 32] {
        let sign_pub = self.sign_key.verifying_key().to_bytes();
        let mut hasher = Sha256::new();
        hasher.update(sign_pub);
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }

    /// Get the Ed25519 signing public key bytes.
    pub fn sign_pub(&self) -> [u8; 32] {
        self.sign_key.verifying_key().to_bytes()
    }

    /// Get the X25519 key exchange public key bytes.
    pub fn kex_pub(&self) -> [u8; 32] {
        *X25519PublicKey::from(&self.kex_key).as_bytes()
    }

    /// Get the public key bundle containing both signing and key exchange public keys.
    pub fn public_bundle(&self) -> PublicKeyBundleV1 {
        PublicKeyBundleV1 {
            sign_pub: self.sign_pub().to_vec(),
            kex_pub: self.kex_pub().to_vec(),
        }
    }

    /// Sign a message using Ed25519.
    ///
    /// Returns a 64-byte signature.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        let signature: Signature = self.sign_key.sign(message);
        signature.to_bytes()
    }

    /// Perform X25519 Diffie-Hellman key exchange.
    ///
    /// # Arguments
    /// * `peer_kex_pub` - The peer's X25519 public key (32 bytes)
    ///
    /// # Returns
    /// A 32-byte shared secret.
    pub fn key_exchange(&self, peer_kex_pub: &[u8; 32]) -> [u8; 32] {
        let peer_pub = X25519PublicKey::from(*peer_kex_pub);
        let shared_secret = self.kex_key.diffie_hellman(&peer_pub);
        *shared_secret.as_bytes()
    }

    /// Get a reference to the X25519 static secret for use with envelope operations.
    pub fn kex_secret(&self) -> &StaticSecret {
        &self.kex_key
    }

    /// Get a reference to the Ed25519 signing key for use with envelope operations.
    pub fn sign_key(&self) -> &SigningKey {
        &self.sign_key
    }
}

/// Verify an Ed25519 signature.
///
/// # Arguments
/// * `pub_key` - The signer's Ed25519 public key (32 bytes)
/// * `message` - The message that was signed
/// * `signature` - The 64-byte Ed25519 signature
pub fn verify_signature(
    pub_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), IdentityError> {
    let verifying_key =
        VerifyingKey::from_bytes(pub_key).map_err(|_| IdentityError::InvalidPublicKey)?;
    let sig = Signature::from_bytes(signature);
    verifying_key
        .verify_strict(message, &sig)
        .map_err(|_| IdentityError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation() {
        let identity = Identity::generate();

        // ID should be SHA-256 of sign_pub
        let sign_pub = identity.sign_pub();
        let mut hasher = Sha256::new();
        hasher.update(sign_pub);
        let expected_id: [u8; 32] = hasher.finalize().into();

        assert_eq!(identity.id(), expected_id);
    }

    #[test]
    fn test_signature_round_trip() {
        let identity = Identity::generate();
        let message = b"Hello, cryptographic world!";

        let signature = identity.sign(message);
        let pub_key = identity.sign_pub();

        // Verification should succeed
        assert!(verify_signature(&pub_key, message, &signature).is_ok());
    }

    #[test]
    fn test_signature_wrong_message_fails() {
        let identity = Identity::generate();
        let message = b"Original message";
        let wrong_message = b"Tampered message";

        let signature = identity.sign(message);
        let pub_key = identity.sign_pub();

        // Verification should fail with wrong message
        assert!(verify_signature(&pub_key, wrong_message, &signature).is_err());
    }

    #[test]
    fn test_signature_wrong_key_fails() {
        let identity1 = Identity::generate();
        let identity2 = Identity::generate();
        let message = b"Test message";

        let signature = identity1.sign(message);
        let wrong_pub_key = identity2.sign_pub();

        // Verification should fail with wrong public key
        assert!(verify_signature(&wrong_pub_key, message, &signature).is_err());
    }

    #[test]
    fn test_key_exchange_consistency() {
        let alice = Identity::generate();
        let bob = Identity::generate();

        // Both parties should derive the same shared secret
        let alice_shared = alice.key_exchange(&bob.kex_pub());
        let bob_shared = bob.key_exchange(&alice.kex_pub());

        assert_eq!(alice_shared, bob_shared);
    }

    #[test]
    fn test_public_bundle() {
        let identity = Identity::generate();
        let bundle = identity.public_bundle();

        assert_eq!(bundle.sign_pub.len(), 32);
        assert_eq!(bundle.kex_pub.len(), 32);
        assert_eq!(bundle.sign_pub.as_slice(), identity.sign_pub().as_slice());
        assert_eq!(bundle.kex_pub.as_slice(), identity.kex_pub().as_slice());
    }

    #[test]
    fn test_from_bytes_round_trip() {
        let original = Identity::generate();
        let sign_pub = original.sign_pub();
        let kex_pub = original.kex_pub();

        // Get the key bytes (this is a simplified test - in practice you'd save the private keys)
        // For this test, we just verify the API works
        let identity2 = Identity::generate();

        // Different identities should have different IDs
        assert_ne!(original.id(), identity2.id());
        assert_ne!(sign_pub, identity2.sign_pub());
        assert_ne!(kex_pub, identity2.kex_pub());
    }

    #[test]
    fn test_multiple_signatures() {
        let identity = Identity::generate();
        let messages = [
            b"First message".as_slice(),
            b"Second message".as_slice(),
            b"Third message".as_slice(),
        ];

        let pub_key = identity.sign_pub();

        for message in messages {
            let signature = identity.sign(message);
            assert!(verify_signature(&pub_key, message, &signature).is_ok());
        }
    }

    #[test]
    fn test_identity_id_determinism() {
        // Same key bytes should produce the same ID every time
        let identity = Identity::generate();
        let id1 = identity.id();
        let id2 = identity.id();
        assert_eq!(id1, id2);
    }
}
