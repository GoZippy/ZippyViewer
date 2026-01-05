//! Update manifest verification.
//!
//! Handles verification of signed update manifests using Ed25519 signatures.
//!
//! # Security
//!
//! This module implements secure manifest verification:
//! - Ed25519 signature verification against pinned public keys
//! - Multi-signature support with configurable threshold
//! - Timestamp validation to prevent replay attacks
//! - Platform matching to prevent cross-platform attacks

use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use semver::Version;

use crate::channel::UpdateChannel;
use crate::error::UpdateError;

/// Maximum age of a manifest in seconds (7 days).
const MAX_MANIFEST_AGE_SECS: u64 = 7 * 24 * 60 * 60;

/// Maximum future timestamp tolerance in seconds (1 hour).
const MAX_FUTURE_TOLERANCE_SECS: u64 = 60 * 60;

/// Verifies update manifest signatures against pinned public keys.
///
/// # Requirements
/// - Requirements 1.1: Verify manifest signature using pinned public key
/// - Requirements 1.6: Support multiple signing keys for rotation
///
/// # Example
///
/// ```ignore
/// use ed25519_dalek::VerifyingKey;
/// use zrc_updater::ManifestVerifier;
///
/// let keys = vec![/* trusted public keys */];
/// let verifier = ManifestVerifier::new(keys, 1);
///
/// let manifest_data = /* downloaded manifest bytes */;
/// let manifest = verifier.verify_and_parse(&manifest_data)?;
/// ```
pub struct ManifestVerifier {
    /// Pinned public keys for manifest signing.
    /// Multiple keys support key rotation (Requirement 1.6).
    trusted_keys: Vec<VerifyingKey>,
    /// Minimum required valid signatures.
    /// Must be at least 1 for security.
    threshold: usize,
    /// Expected platform string for this system.
    /// Used to reject manifests targeting other platforms.
    expected_platform: String,
}

impl ManifestVerifier {
    /// Create a new verifier with pinned keys and signature threshold.
    ///
    /// # Arguments
    ///
    /// * `trusted_keys` - Ed25519 public keys trusted for signing manifests
    /// * `threshold` - Minimum number of valid signatures required
    ///
    /// # Panics
    ///
    /// Panics if threshold is 0 (would allow unsigned manifests).
    pub fn new(trusted_keys: Vec<VerifyingKey>, threshold: usize) -> Self {
        assert!(threshold > 0, "signature threshold must be at least 1");
        Self {
            trusted_keys,
            threshold,
            expected_platform: current_platform(),
        }
    }

    /// Create a verifier with a custom expected platform.
    ///
    /// Useful for testing or cross-platform scenarios.
    pub fn with_platform(
        trusted_keys: Vec<VerifyingKey>,
        threshold: usize,
        expected_platform: String,
    ) -> Self {
        assert!(threshold > 0, "signature threshold must be at least 1");
        Self {
            trusted_keys,
            threshold,
            expected_platform,
        }
    }

    /// Get the trusted public keys.
    pub fn trusted_keys(&self) -> &[VerifyingKey] {
        &self.trusted_keys
    }

    /// Get the signature threshold.
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Get the expected platform.
    pub fn expected_platform(&self) -> &str {
        &self.expected_platform
    }

    /// Verify manifest signature and parse contents.
    ///
    /// This method performs the following verifications:
    /// 1. Parse the signed manifest JSON
    /// 2. Verify timestamp is recent (within 7 days) - Requirement 1.3
    /// 3. Verify timestamp is not in the future
    /// 4. Verify at least `threshold` valid signatures - Requirements 1.1, 1.2
    /// 5. Parse the inner manifest JSON
    /// 6. Verify platform matches current system - Requirement 1.5
    ///
    /// # Arguments
    ///
    /// * `data` - Raw bytes of the signed manifest JSON
    ///
    /// # Returns
    ///
    /// The parsed `UpdateManifest` if all verifications pass.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - JSON parsing fails
    /// - Timestamp is too old (> 7 days)
    /// - Timestamp is in the future (> 1 hour tolerance)
    /// - Insufficient valid signatures
    /// - Platform mismatch
    pub fn verify_and_parse(&self, data: &[u8]) -> Result<UpdateManifest, UpdateError> {
        // Parse the signed manifest envelope
        let signed_manifest: SignedManifest = serde_json::from_slice(data)?;

        // Verify timestamp is recent (Requirement 1.3)
        self.verify_timestamp(signed_manifest.timestamp)?;

        // Verify signatures (Requirements 1.1, 1.2, 1.6)
        let valid_signatures = self.count_valid_signatures(&signed_manifest)?;
        if valid_signatures < self.threshold {
            tracing::error!(
                required = self.threshold,
                found = valid_signatures,
                "Insufficient valid signatures on manifest"
            );
            return Err(UpdateError::InsufficientSignatures {
                required: self.threshold,
                found: valid_signatures,
            });
        }

        tracing::debug!(
            valid_signatures,
            threshold = self.threshold,
            "Manifest signature verification passed"
        );

        // Parse the inner manifest
        let manifest: UpdateManifest = serde_json::from_str(&signed_manifest.manifest)?;

        // Verify platform matches (Requirement 1.5)
        if manifest.platform != self.expected_platform {
            tracing::error!(
                expected = %self.expected_platform,
                actual = %manifest.platform,
                "Platform mismatch in manifest"
            );
            return Err(UpdateError::PlatformMismatch {
                expected: self.expected_platform.clone(),
                actual: manifest.platform.clone(),
            });
        }

        tracing::info!(
            version = %manifest.version,
            platform = %manifest.platform,
            channel = %manifest.channel,
            "Manifest verified successfully"
        );

        Ok(manifest)
    }

    /// Verify the manifest timestamp is within acceptable bounds.
    ///
    /// - Must not be older than 7 days (Requirement 1.3)
    /// - Must not be more than 1 hour in the future (clock skew tolerance)
    fn verify_timestamp(&self, timestamp: u64) -> Result<(), UpdateError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| UpdateError::ConfigError(format!("system time error: {}", e)))?
            .as_secs();

        // Check if manifest is too old
        if timestamp < now.saturating_sub(MAX_MANIFEST_AGE_SECS) {
            tracing::error!(
                manifest_timestamp = timestamp,
                current_time = now,
                max_age_secs = MAX_MANIFEST_AGE_SECS,
                "Manifest timestamp is too old"
            );
            return Err(UpdateError::ManifestTooOld);
        }

        // Check if manifest is from the future (with tolerance for clock skew)
        if timestamp > now + MAX_FUTURE_TOLERANCE_SECS {
            tracing::error!(
                manifest_timestamp = timestamp,
                current_time = now,
                tolerance_secs = MAX_FUTURE_TOLERANCE_SECS,
                "Manifest timestamp is in the future"
            );
            return Err(UpdateError::ManifestFromFuture);
        }

        Ok(())
    }

    /// Count the number of valid signatures on the manifest.
    ///
    /// A signature is valid if:
    /// - It can be verified against one of the trusted keys
    /// - Each key can only validate one signature (no double-counting)
    fn count_valid_signatures(&self, signed_manifest: &SignedManifest) -> Result<usize, UpdateError> {
        let manifest_bytes = signed_manifest.manifest.as_bytes();
        let mut valid_count = 0;
        let mut used_keys = vec![false; self.trusted_keys.len()];

        for sig in &signed_manifest.signatures {
            // Try each trusted key that hasn't been used yet
            for (i, key) in self.trusted_keys.iter().enumerate() {
                if used_keys[i] {
                    continue;
                }

                if key.verify(manifest_bytes, &sig.signature).is_ok() {
                    valid_count += 1;
                    used_keys[i] = true;
                    tracing::debug!(
                        key_id = %sig.key_id,
                        "Valid signature found"
                    );
                    break;
                }
            }
        }

        Ok(valid_count)
    }
}

/// A signed update manifest with signatures and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    /// JSON string of UpdateManifest
    pub manifest: String,
    /// Signatures from signing keys
    pub signatures: Vec<ManifestSignature>,
    /// Unix timestamp when manifest was signed
    pub timestamp: u64,
}

impl SignedManifest {
    /// Create a new signed manifest.
    ///
    /// # Arguments
    ///
    /// * `manifest` - The serialized UpdateManifest JSON string
    /// * `signatures` - Signatures from one or more signing keys
    /// * `timestamp` - Unix timestamp when the manifest was signed
    pub fn new(manifest: String, signatures: Vec<ManifestSignature>, timestamp: u64) -> Self {
        Self {
            manifest,
            signatures,
            timestamp,
        }
    }

    /// Get the manifest JSON string.
    pub fn manifest_json(&self) -> &str {
        &self.manifest
    }

    /// Get the signatures.
    pub fn signatures(&self) -> &[ManifestSignature] {
        &self.signatures
    }

    /// Get the timestamp.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

/// A signature on the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSignature {
    /// Identifier for the signing key
    pub key_id: String,
    /// Ed25519 signature bytes (base64 encoded in JSON)
    #[serde(with = "signature_serde")]
    pub signature: ed25519_dalek::Signature,
}

impl ManifestSignature {
    /// Create a new manifest signature.
    ///
    /// # Arguments
    ///
    /// * `key_id` - Identifier for the signing key
    /// * `signature` - The Ed25519 signature
    pub fn new(key_id: String, signature: ed25519_dalek::Signature) -> Self {
        Self { key_id, signature }
    }
}

/// The update manifest describing an available update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    /// Version of the update
    pub version: Version,
    /// Target platform (e.g., "windows-x86_64", "macos-aarch64")
    pub platform: String,
    /// Update channel
    pub channel: UpdateChannel,
    /// URL to download the artifact
    pub artifact_url: String,
    /// SHA-256 hash of the artifact (hex encoded)
    pub artifact_hash: String,
    /// Size of the artifact in bytes
    pub artifact_size: u64,
    /// Release notes (markdown)
    pub release_notes: String,
    /// Whether this is a security update
    pub is_security_update: bool,
    /// Minimum version required for delta update (if applicable)
    pub min_version: Option<Version>,
}

impl UpdateManifest {
    /// Create a new update manifest.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        version: Version,
        platform: String,
        channel: UpdateChannel,
        artifact_url: String,
        artifact_hash: String,
        artifact_size: u64,
        release_notes: String,
        is_security_update: bool,
        min_version: Option<Version>,
    ) -> Self {
        Self {
            version,
            platform,
            channel,
            artifact_url,
            artifact_hash,
            artifact_size,
            release_notes,
            is_security_update,
            min_version,
        }
    }

    /// Check if this is a security update.
    pub fn is_security_update(&self) -> bool {
        self.is_security_update
    }

    /// Get the artifact hash as bytes.
    ///
    /// Returns None if the hash is not valid hex.
    pub fn artifact_hash_bytes(&self) -> Option<[u8; 32]> {
        let bytes = hex::decode(&self.artifact_hash).ok()?;
        bytes.try_into().ok()
    }
}

/// Get the current platform string.
///
/// Returns a string like "windows-x86_64", "macos-aarch64", "linux-x86_64".
pub fn current_platform() -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "unknown"
    };

    format!("{}-{}", os, arch)
}

/// Serde helper for Ed25519 signatures.
mod signature_serde {
    use ed25519_dalek::Signature;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(sig: &Signature, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = sig.to_bytes();
        let b64 = base64_encode(&bytes);
        b64.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Signature, D::Error>
    where
        D: Deserializer<'de>,
    {
        let b64 = String::deserialize(deserializer)?;
        let bytes = base64_decode(&b64).map_err(serde::de::Error::custom)?;
        let bytes: [u8; 64] = bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("invalid signature length"))?;
        Ok(Signature::from_bytes(&bytes))
    }

    fn base64_encode(data: &[u8]) -> String {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut encoder = Base64Encoder::new(&mut buf);
            encoder.write_all(data).unwrap();
        }
        String::from_utf8(buf).unwrap()
    }

    fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
        // Simple base64 decode
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = Vec::new();
        let mut buffer = 0u32;
        let mut bits = 0;

        for c in s.bytes() {
            if c == b'=' {
                break;
            }
            let val = alphabet
                .iter()
                .position(|&x| x == c)
                .ok_or_else(|| "invalid base64 character".to_string())?
                as u32;
            buffer = (buffer << 6) | val;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                result.push((buffer >> bits) as u8);
                buffer &= (1 << bits) - 1;
            }
        }
        Ok(result)
    }

    struct Base64Encoder<'a> {
        output: &'a mut Vec<u8>,
        buffer: u32,
        bits: u8,
    }

    impl<'a> Base64Encoder<'a> {
        fn new(output: &'a mut Vec<u8>) -> Self {
            Self {
                output,
                buffer: 0,
                bits: 0,
            }
        }
    }

    impl<'a> std::io::Write for Base64Encoder<'a> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            for &byte in buf {
                self.buffer = (self.buffer << 8) | byte as u32;
                self.bits += 8;
                while self.bits >= 6 {
                    self.bits -= 6;
                    let idx = ((self.buffer >> self.bits) & 0x3F) as usize;
                    self.output.push(alphabet[idx]);
                }
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            if self.bits > 0 {
                let idx = ((self.buffer << (6 - self.bits)) & 0x3F) as usize;
                self.output.push(alphabet[idx]);
                while self.bits < 6 {
                    self.output.push(b'=');
                    self.bits += 2;
                }
            }
            Ok(())
        }
    }

    impl<'a> Drop for Base64Encoder<'a> {
        fn drop(&mut self) {
            let _ = std::io::Write::flush(self);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{SigningKey, Signer};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Helper to create a test keypair
    fn create_test_keypair() -> (SigningKey, VerifyingKey) {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    /// Helper to create a second test keypair
    fn create_test_keypair_2() -> (SigningKey, VerifyingKey) {
        let signing_key = SigningKey::from_bytes(&[2u8; 32]);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    /// Helper to get current timestamp
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Helper to create a test manifest JSON
    fn create_test_manifest_json(platform: &str) -> String {
        serde_json::json!({
            "version": "1.2.3",
            "platform": platform,
            "channel": "stable",
            "artifact_url": "https://example.com/update.zip",
            "artifact_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "artifact_size": 1024,
            "release_notes": "Test release",
            "is_security_update": false,
            "min_version": null
        })
        .to_string()
    }

    /// Helper to create a signed manifest
    fn create_signed_manifest(
        manifest_json: &str,
        signing_keys: &[(SigningKey, &str)],
        timestamp: u64,
    ) -> SignedManifest {
        let signatures: Vec<ManifestSignature> = signing_keys
            .iter()
            .map(|(key, key_id)| {
                let signature = key.sign(manifest_json.as_bytes());
                ManifestSignature::new(key_id.to_string(), signature)
            })
            .collect();

        SignedManifest::new(manifest_json.to_string(), signatures, timestamp)
    }

    #[test]
    fn test_manifest_verifier_new() {
        let (_, verifying_key) = create_test_keypair();
        let verifier = ManifestVerifier::new(vec![verifying_key], 1);
        
        assert_eq!(verifier.threshold(), 1);
        assert_eq!(verifier.trusted_keys().len(), 1);
    }

    #[test]
    #[should_panic(expected = "signature threshold must be at least 1")]
    fn test_manifest_verifier_zero_threshold_panics() {
        let (_, verifying_key) = create_test_keypair();
        ManifestVerifier::new(vec![verifying_key], 0);
    }

    #[test]
    fn test_manifest_verifier_with_platform() {
        let (_, verifying_key) = create_test_keypair();
        let verifier = ManifestVerifier::with_platform(
            vec![verifying_key],
            1,
            "test-platform".to_string(),
        );
        
        assert_eq!(verifier.expected_platform(), "test-platform");
    }

    #[test]
    fn test_verify_valid_manifest() {
        let (signing_key, verifying_key) = create_test_keypair();
        let platform = current_platform();
        let manifest_json = create_test_manifest_json(&platform);
        let timestamp = current_timestamp();
        
        let signed_manifest = create_signed_manifest(
            &manifest_json,
            &[(signing_key, "key1")],
            timestamp,
        );
        
        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        
        let verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let result = verifier.verify_and_parse(&signed_data);
        
        assert!(result.is_ok());
        let manifest = result.unwrap();
        assert_eq!(manifest.version.to_string(), "1.2.3");
        assert_eq!(manifest.platform, platform);
    }

    #[test]
    fn test_verify_invalid_signature() {
        let (signing_key, _) = create_test_keypair();
        let (_, other_verifying_key) = create_test_keypair_2();
        
        let platform = current_platform();
        let manifest_json = create_test_manifest_json(&platform);
        let timestamp = current_timestamp();
        
        // Sign with one key but verify with another
        let signed_manifest = create_signed_manifest(
            &manifest_json,
            &[(signing_key, "key1")],
            timestamp,
        );
        
        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        
        let verifier = ManifestVerifier::new(vec![other_verifying_key], 1);
        let result = verifier.verify_and_parse(&signed_data);
        
        assert!(matches!(result, Err(UpdateError::InsufficientSignatures { required: 1, found: 0 })));
    }

    #[test]
    fn test_verify_manifest_too_old() {
        let (signing_key, verifying_key) = create_test_keypair();
        let platform = current_platform();
        let manifest_json = create_test_manifest_json(&platform);
        
        // Timestamp from 8 days ago
        let old_timestamp = current_timestamp() - (8 * 24 * 60 * 60);
        
        let signed_manifest = create_signed_manifest(
            &manifest_json,
            &[(signing_key, "key1")],
            old_timestamp,
        );
        
        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        
        let verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let result = verifier.verify_and_parse(&signed_data);
        
        assert!(matches!(result, Err(UpdateError::ManifestTooOld)));
    }

    #[test]
    fn test_verify_manifest_from_future() {
        let (signing_key, verifying_key) = create_test_keypair();
        let platform = current_platform();
        let manifest_json = create_test_manifest_json(&platform);
        
        // Timestamp 2 hours in the future
        let future_timestamp = current_timestamp() + (2 * 60 * 60);
        
        let signed_manifest = create_signed_manifest(
            &manifest_json,
            &[(signing_key, "key1")],
            future_timestamp,
        );
        
        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        
        let verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let result = verifier.verify_and_parse(&signed_data);
        
        assert!(matches!(result, Err(UpdateError::ManifestFromFuture)));
    }

    #[test]
    fn test_verify_platform_mismatch() {
        let (signing_key, verifying_key) = create_test_keypair();
        let manifest_json = create_test_manifest_json("wrong-platform");
        let timestamp = current_timestamp();
        
        let signed_manifest = create_signed_manifest(
            &manifest_json,
            &[(signing_key, "key1")],
            timestamp,
        );
        
        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        
        let verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let result = verifier.verify_and_parse(&signed_data);
        
        assert!(matches!(result, Err(UpdateError::PlatformMismatch { .. })));
    }

    #[test]
    fn test_verify_multi_signature_threshold() {
        let (signing_key1, verifying_key1) = create_test_keypair();
        let (signing_key2, verifying_key2) = create_test_keypair_2();
        
        let platform = current_platform();
        let manifest_json = create_test_manifest_json(&platform);
        let timestamp = current_timestamp();
        
        // Sign with both keys
        let signed_manifest = create_signed_manifest(
            &manifest_json,
            &[(signing_key1, "key1"), (signing_key2, "key2")],
            timestamp,
        );
        
        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        
        // Require 2 signatures
        let verifier = ManifestVerifier::new(vec![verifying_key1, verifying_key2], 2);
        let result = verifier.verify_and_parse(&signed_data);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_insufficient_signatures_for_threshold() {
        let (signing_key1, verifying_key1) = create_test_keypair();
        let (_, verifying_key2) = create_test_keypair_2();
        
        let platform = current_platform();
        let manifest_json = create_test_manifest_json(&platform);
        let timestamp = current_timestamp();
        
        // Sign with only one key
        let signed_manifest = create_signed_manifest(
            &manifest_json,
            &[(signing_key1, "key1")],
            timestamp,
        );
        
        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        
        // Require 2 signatures but only have 1
        let verifier = ManifestVerifier::new(vec![verifying_key1, verifying_key2], 2);
        let result = verifier.verify_and_parse(&signed_data);
        
        assert!(matches!(result, Err(UpdateError::InsufficientSignatures { required: 2, found: 1 })));
    }

    #[test]
    fn test_current_platform() {
        let platform = current_platform();
        
        // Should contain OS and arch separated by dash
        assert!(platform.contains('-'));
        
        #[cfg(target_os = "windows")]
        assert!(platform.starts_with("windows-"));
        
        #[cfg(target_os = "macos")]
        assert!(platform.starts_with("macos-"));
        
        #[cfg(target_os = "linux")]
        assert!(platform.starts_with("linux-"));
        
        #[cfg(target_arch = "x86_64")]
        assert!(platform.ends_with("-x86_64"));
        
        #[cfg(target_arch = "aarch64")]
        assert!(platform.ends_with("-aarch64"));
    }

    #[test]
    fn test_signed_manifest_accessors() {
        let manifest = SignedManifest::new(
            "test".to_string(),
            vec![],
            12345,
        );
        
        assert_eq!(manifest.manifest_json(), "test");
        assert!(manifest.signatures().is_empty());
        assert_eq!(manifest.timestamp(), 12345);
    }

    #[test]
    fn test_update_manifest_artifact_hash_bytes() {
        let manifest = UpdateManifest::new(
            Version::new(1, 0, 0),
            "test".to_string(),
            UpdateChannel::Stable,
            "https://example.com".to_string(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            1024,
            "notes".to_string(),
            false,
            None,
        );
        
        let hash_bytes = manifest.artifact_hash_bytes();
        assert!(hash_bytes.is_some());
        assert_eq!(hash_bytes.unwrap().len(), 32);
    }

    #[test]
    fn test_update_manifest_invalid_hash() {
        let manifest = UpdateManifest::new(
            Version::new(1, 0, 0),
            "test".to_string(),
            UpdateChannel::Stable,
            "https://example.com".to_string(),
            "invalid-hash".to_string(),
            1024,
            "notes".to_string(),
            false,
            None,
        );
        
        assert!(manifest.artifact_hash_bytes().is_none());
    }
}
