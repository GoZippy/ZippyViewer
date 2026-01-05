//! Directory record signing module.
//!
//! Provides functions for signing and verifying directory records
//! that publish device presence information.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::hash::{derive_id, sha256};
use crate::identity::Identity;
use crate::transcript::Transcript;

/// Error type for directory operations.
#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("invalid key length: expected {expected}, got {got}")]
    InvalidKeyLength { expected: usize, got: usize },
    #[error("record expired: timestamp {timestamp} + ttl {ttl} < now {now}")]
    RecordExpired { timestamp: u64, ttl: u32, now: u64 },
    #[error("subject_id does not match signing key")]
    SubjectMismatch,
    #[error("signature verification failed")]
    SignatureVerificationFailed,
}

/// Compute the canonical signing bytes for a directory record.
///
/// The transcript includes all fields that should be signed,
/// in a canonical order with tags.
pub fn dir_record_sign_data(
    subject_id: &[u8],
    device_sign_pub: &[u8],
    endpoints_encoded: &[u8], // Pre-encoded endpoints
    ttl_seconds: u32,
    timestamp: u64,
) -> [u8; 32] {
    let mut t = Transcript::new("zrc_dir_record_v1");
    t.append_bytes(1, subject_id);
    t.append_bytes(2, device_sign_pub);
    t.append_bytes(3, endpoints_encoded);
    t.append_u64(4, ttl_seconds as u64);
    t.append_u64(5, timestamp);
    sha256(t.as_bytes())
}

/// Sign a directory record.
///
/// Returns the Ed25519 signature bytes (64 bytes).
pub fn sign_record(
    identity: &Identity,
    subject_id: &[u8],
    endpoints_encoded: &[u8],
    ttl_seconds: u32,
    timestamp: u64,
) -> Result<[u8; 64], DirectoryError> {
    // Verify subject_id matches the identity
    let derived_id = identity.id();
    if subject_id != derived_id.as_slice() {
        return Err(DirectoryError::SubjectMismatch);
    }

    let device_sign_pub = identity.sign_pub();
    let sign_data = dir_record_sign_data(
        subject_id,
        &device_sign_pub,
        endpoints_encoded,
        ttl_seconds,
        timestamp,
    );

    Ok(identity.sign(&sign_data))
}

/// Sign a directory record with a raw signing key.
pub fn sign_record_with_key(
    sign_key: &SigningKey,
    subject_id: &[u8],
    endpoints_encoded: &[u8],
    ttl_seconds: u32,
    timestamp: u64,
) -> Result<[u8; 64], DirectoryError> {
    // Verify subject_id matches the signing key
    let device_sign_pub = sign_key.verifying_key().to_bytes();
    let derived_id = derive_id(&device_sign_pub);
    if subject_id != derived_id.as_slice() {
        return Err(DirectoryError::SubjectMismatch);
    }

    let sign_data = dir_record_sign_data(
        subject_id,
        &device_sign_pub,
        endpoints_encoded,
        ttl_seconds,
        timestamp,
    );

    let signature: Signature = sign_key.sign(&sign_data);
    Ok(signature.to_bytes())
}

/// Verify a directory record signature and check expiration.
///
/// Checks:
/// 1. subject_id matches the device_sign_pub
/// 2. signature is valid
/// 3. record has not expired (timestamp + ttl > now)
pub fn verify_record(
    subject_id: &[u8],
    device_sign_pub: &[u8],
    endpoints_encoded: &[u8],
    ttl_seconds: u32,
    timestamp: u64,
    signature: &[u8],
    now: u64,
) -> Result<(), DirectoryError> {
    // Check key length
    if device_sign_pub.len() != 32 {
        return Err(DirectoryError::InvalidKeyLength {
            expected: 32,
            got: device_sign_pub.len(),
        });
    }

    // Check signature length
    if signature.len() != 64 {
        return Err(DirectoryError::InvalidKeyLength {
            expected: 64,
            got: signature.len(),
        });
    }

    // Verify subject_id matches the public key
    let derived_id = derive_id(device_sign_pub);
    if subject_id != derived_id.as_slice() {
        return Err(DirectoryError::SubjectMismatch);
    }

    // Check expiration
    let expires_at = timestamp.saturating_add(ttl_seconds as u64);
    if expires_at <= now {
        return Err(DirectoryError::RecordExpired {
            timestamp,
            ttl: ttl_seconds,
            now,
        });
    }

    // Verify signature
    let sign_data = dir_record_sign_data(
        subject_id,
        device_sign_pub,
        endpoints_encoded,
        ttl_seconds,
        timestamp,
    );

    let device_sign_pub_arr: [u8; 32] = device_sign_pub
        .try_into()
        .map_err(|_| DirectoryError::SignatureVerificationFailed)?;

    let verifying_key = VerifyingKey::from_bytes(&device_sign_pub_arr)
        .map_err(|_| DirectoryError::SignatureVerificationFailed)?;

    let sig_arr: [u8; 64] = signature
        .try_into()
        .map_err(|_| DirectoryError::SignatureVerificationFailed)?;
    let sig = Signature::from_bytes(&sig_arr);

    verifying_key
        .verify_strict(&sign_data, &sig)
        .map_err(|_| DirectoryError::SignatureVerificationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify_round_trip() {
        let identity = Identity::generate();
        let subject_id = identity.id();
        let endpoints = b"endpoint_data";
        let ttl = 3600u32; // 1 hour
        let timestamp = 1700000000u64;
        let now = 1700000000u64; // Same as timestamp - not expired yet

        let signature = sign_record(
            &identity,
            &subject_id,
            endpoints,
            ttl,
            timestamp,
        )
        .unwrap();

        assert!(verify_record(
            &subject_id,
            &identity.sign_pub(),
            endpoints,
            ttl,
            timestamp,
            &signature,
            now,
        )
        .is_ok());
    }

    #[test]
    fn test_record_expired() {
        let identity = Identity::generate();
        let subject_id = identity.id();
        let endpoints = b"endpoint_data";
        let ttl = 3600u32;
        let timestamp = 1700000000u64;
        let now = timestamp + ttl as u64 + 1; // Just past expiration

        let signature = sign_record(
            &identity,
            &subject_id,
            endpoints,
            ttl,
            timestamp,
        )
        .unwrap();

        assert!(matches!(
            verify_record(
                &subject_id,
                &identity.sign_pub(),
                endpoints,
                ttl,
                timestamp,
                &signature,
                now,
            ),
            Err(DirectoryError::RecordExpired { .. })
        ));
    }

    #[test]
    fn test_subject_mismatch() {
        let identity1 = Identity::generate();
        let identity2 = Identity::generate();
        let wrong_subject_id = identity2.id();
        let endpoints = b"endpoint_data";
        let ttl = 3600u32;
        let timestamp = 1700000000u64;

        // Trying to sign with wrong subject_id should fail
        assert!(matches!(
            sign_record(&identity1, &wrong_subject_id, endpoints, ttl, timestamp),
            Err(DirectoryError::SubjectMismatch)
        ));
    }

    #[test]
    fn test_tampered_endpoints() {
        let identity = Identity::generate();
        let subject_id = identity.id();
        let endpoints = b"endpoint_data";
        let tampered_endpoints = b"tampered_data";
        let ttl = 3600u32;
        let timestamp = 1700000000u64;
        let now = timestamp;

        let signature = sign_record(&identity, &subject_id, endpoints, ttl, timestamp).unwrap();

        // Verification with tampered endpoints should fail
        assert!(matches!(
            verify_record(
                &subject_id,
                &identity.sign_pub(),
                tampered_endpoints,
                ttl,
                timestamp,
                &signature,
                now,
            ),
            Err(DirectoryError::SignatureVerificationFailed)
        ));
    }

    #[test]
    fn test_tampered_signature() {
        let identity = Identity::generate();
        let subject_id = identity.id();
        let endpoints = b"endpoint_data";
        let ttl = 3600u32;
        let timestamp = 1700000000u64;
        let now = timestamp;

        let mut signature = sign_record(&identity, &subject_id, endpoints, ttl, timestamp).unwrap();
        signature[0] ^= 0xFF; // Tamper with signature

        assert!(matches!(
            verify_record(
                &subject_id,
                &identity.sign_pub(),
                endpoints,
                ttl,
                timestamp,
                &signature,
                now,
            ),
            Err(DirectoryError::SignatureVerificationFailed)
        ));
    }

    #[test]
    fn test_just_before_expiration() {
        let identity = Identity::generate();
        let subject_id = identity.id();
        let endpoints = b"endpoint_data";
        let ttl = 3600u32;
        let timestamp = 1700000000u64;
        let now = timestamp + ttl as u64 - 1; // 1 second before expiration

        let signature = sign_record(&identity, &subject_id, endpoints, ttl, timestamp).unwrap();

        // Should still be valid
        assert!(verify_record(
            &subject_id,
            &identity.sign_pub(),
            endpoints,
            ttl,
            timestamp,
            &signature,
            now,
        )
        .is_ok());
    }
}
