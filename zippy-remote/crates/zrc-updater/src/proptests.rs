//! Property-based tests for zrc-updater.
//!
//! These tests use proptest to verify correctness properties across
//! randomly generated inputs.
//!
//! # Properties Tested
//!
//! - Property 1: Manifest Signature Verification (Requirements 1.1, 1.2, 1.6)
//! - Property 2: Artifact Hash Verification (Requirements 2.1, 2.2, 2.5)
//! - Property 3: Rollback Availability (Requirements 9.1, 9.2, 9.3)
//! - Property 5: Channel Isolation (Requirements 3.1, 3.7)

#![cfg(test)]

use proptest::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

use ed25519_dalek::{Signer, SigningKey};
use semver::Version;
use sha2::{Digest, Sha256};

use crate::artifact::ArtifactVerifier;
use crate::channel::{ChannelManager, UpdateChannel};
use crate::manifest::{current_platform, ManifestSignature, ManifestVerifier, SignedManifest};
use crate::rollback::RollbackManager;

// =============================================================================
// Generators
// =============================================================================

/// Generate a random Ed25519 signing key from 32 random bytes.
fn arb_signing_key() -> impl Strategy<Value = SigningKey> {
    prop::array::uniform32(any::<u8>()).prop_map(|bytes| SigningKey::from_bytes(&bytes))
}

/// Generate a random version.
fn arb_version() -> impl Strategy<Value = Version> {
    (0u64..100, 0u64..100, 0u64..100).prop_map(|(major, minor, patch)| {
        Version::new(major, minor, patch)
    })
}

/// Generate random artifact content (1 to 10KB).
fn arb_artifact_content() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 1..10240)
}

/// Generate a valid timestamp (within the last 6 days to be safe).
fn arb_valid_timestamp() -> impl Strategy<Value = u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Generate timestamps within the last 6 days (well within 7-day limit)
    let six_days_ago = now.saturating_sub(6 * 24 * 60 * 60);
    six_days_ago..=now
}

/// Generate an old timestamp (more than 7 days ago).
fn arb_old_timestamp() -> impl Strategy<Value = u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Generate timestamps from 8-30 days ago
    let thirty_days_ago = now.saturating_sub(30 * 24 * 60 * 60);
    let eight_days_ago = now.saturating_sub(8 * 24 * 60 * 60);
    thirty_days_ago..=eight_days_ago
}

/// Generate a random update channel.
fn arb_channel() -> impl Strategy<Value = UpdateChannel> {
    prop_oneof![
        Just(UpdateChannel::Stable),
        Just(UpdateChannel::Beta),
        Just(UpdateChannel::Nightly),
    ]
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a manifest JSON string.
fn create_manifest_json(platform: &str, artifact_hash: &str, artifact_size: u64) -> String {
    serde_json::json!({
        "version": "2.0.0",
        "platform": platform,
        "channel": "stable",
        "artifact_url": "https://example.com/update.bin",
        "artifact_hash": artifact_hash,
        "artifact_size": artifact_size,
        "release_notes": "Test update",
        "is_security_update": false,
        "min_version": null
    })
    .to_string()
}

/// Sign a manifest with the given keys.
fn sign_manifest(
    manifest_json: &str,
    signing_keys: &[(SigningKey, String)],
    timestamp: u64,
) -> SignedManifest {
    let signatures: Vec<ManifestSignature> = signing_keys
        .iter()
        .map(|(key, key_id)| {
            let signature = key.sign(manifest_json.as_bytes());
            ManifestSignature::new(key_id.clone(), signature)
        })
        .collect();

    SignedManifest::new(manifest_json.to_string(), signatures, timestamp)
}

/// Compute SHA-256 hash of data.
fn compute_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

// =============================================================================
// Property 1: Manifest Signature Verification
// **Validates: Requirements 1.1, 1.2, 1.6**
//
// *For any* update manifest, at least `threshold` valid signatures from
// trusted keys SHALL be verified before processing.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: zrc-updater, Property 1: Manifest Signature Verification**
    /// **Validates: Requirements 1.1, 1.2, 1.6**
    ///
    /// For any valid signing key and manifest content, a manifest signed
    /// with that key should be accepted when verified with the corresponding
    /// public key.
    #[test]
    fn prop_valid_signature_accepted(
        signing_key in arb_signing_key(),
        timestamp in arb_valid_timestamp(),
    ) {
        let verifying_key = signing_key.verifying_key();
        let platform = current_platform();
        let manifest_json = create_manifest_json(&platform, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", 1024);

        let signed_manifest = sign_manifest(
            &manifest_json,
            &[(signing_key, "key1".to_string())],
            timestamp,
        );

        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        let verifier = ManifestVerifier::new(vec![verifying_key], 1);

        let result = verifier.verify_and_parse(&signed_data);
        prop_assert!(result.is_ok(), "Valid signature should be accepted: {:?}", result.err());
    }

    /// **Feature: zrc-updater, Property 1: Invalid Signature Rejected**
    /// **Validates: Requirements 1.1, 1.2**
    ///
    /// For any two different signing keys, a manifest signed with one key
    /// should be rejected when verified with the other key's public key.
    #[test]
    fn prop_invalid_signature_rejected(
        signing_key1 in arb_signing_key(),
        signing_key2 in arb_signing_key(),
        timestamp in arb_valid_timestamp(),
    ) {
        // Ensure keys are different
        let key1_bytes = signing_key1.to_bytes();
        let key2_bytes = signing_key2.to_bytes();
        prop_assume!(key1_bytes != key2_bytes);

        let verifying_key2 = signing_key2.verifying_key();
        let platform = current_platform();
        let manifest_json = create_manifest_json(&platform, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", 1024);

        // Sign with key1 but verify with key2
        let signed_manifest = sign_manifest(
            &manifest_json,
            &[(signing_key1, "key1".to_string())],
            timestamp,
        );

        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        let verifier = ManifestVerifier::new(vec![verifying_key2], 1);

        let result = verifier.verify_and_parse(&signed_data);
        prop_assert!(result.is_err(), "Invalid signature should be rejected");
    }

    /// **Feature: zrc-updater, Property 1: Multi-Signature Threshold**
    /// **Validates: Requirements 1.6**
    ///
    /// For any set of N signing keys with threshold T <= N, a manifest
    /// signed with all N keys should be accepted.
    #[test]
    fn prop_multi_signature_threshold_met(
        key1 in arb_signing_key(),
        key2 in arb_signing_key(),
        timestamp in arb_valid_timestamp(),
    ) {
        let verifying_key1 = key1.verifying_key();
        let verifying_key2 = key2.verifying_key();
        let platform = current_platform();
        let manifest_json = create_manifest_json(&platform, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", 1024);

        // Sign with both keys
        let signed_manifest = sign_manifest(
            &manifest_json,
            &[(key1, "key1".to_string()), (key2, "key2".to_string())],
            timestamp,
        );

        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();

        // Require 2 signatures
        let verifier = ManifestVerifier::new(vec![verifying_key1, verifying_key2], 2);

        let result = verifier.verify_and_parse(&signed_data);
        prop_assert!(result.is_ok(), "Multi-signature threshold should be met: {:?}", result.err());
    }

    /// **Feature: zrc-updater, Property 1: Old Manifest Rejected**
    /// **Validates: Requirements 1.3**
    ///
    /// For any manifest with a timestamp older than 7 days, verification
    /// should fail regardless of signature validity.
    #[test]
    fn prop_old_manifest_rejected(
        signing_key in arb_signing_key(),
        old_timestamp in arb_old_timestamp(),
    ) {
        let verifying_key = signing_key.verifying_key();
        let platform = current_platform();
        let manifest_json = create_manifest_json(&platform, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", 1024);

        let signed_manifest = sign_manifest(
            &manifest_json,
            &[(signing_key, "key1".to_string())],
            old_timestamp,
        );

        let signed_data = serde_json::to_vec(&signed_manifest).unwrap();
        let verifier = ManifestVerifier::new(vec![verifying_key], 1);

        let result = verifier.verify_and_parse(&signed_data);
        prop_assert!(result.is_err(), "Old manifest should be rejected");
    }
}

// =============================================================================
// Property 2: Artifact Hash Verification
// **Validates: Requirements 2.1, 2.2, 2.5**
//
// *For any* downloaded artifact, the SHA-256 hash SHALL match the hash
// specified in the signed manifest.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: zrc-updater, Property 2: Correct Hash Accepted**
    /// **Validates: Requirements 2.1, 2.2**
    ///
    /// For any artifact content, computing its SHA-256 hash and verifying
    /// against that same hash should succeed.
    #[test]
    fn prop_correct_hash_accepted(content in arb_artifact_content()) {
        let temp_dir = TempDir::new().unwrap();
        let artifact_path = temp_dir.path().join("artifact.bin");
        std::fs::write(&artifact_path, &content).unwrap();

        let expected_hash = compute_hash(&content);
        let verifier = ArtifactVerifier::new();

        let result = verifier.verify(&artifact_path, &expected_hash);
        prop_assert!(result.is_ok(), "Correct hash should be accepted: {:?}", result.err());
    }

    /// **Feature: zrc-updater, Property 2: Incorrect Hash Rejected**
    /// **Validates: Requirements 2.5**
    ///
    /// For any artifact content and any different hash value, verification
    /// should fail.
    #[test]
    fn prop_incorrect_hash_rejected(
        content in arb_artifact_content(),
        wrong_hash_bytes in prop::array::uniform32(any::<u8>()),
    ) {
        let temp_dir = TempDir::new().unwrap();
        let artifact_path = temp_dir.path().join("artifact.bin");
        std::fs::write(&artifact_path, &content).unwrap();

        let correct_hash = compute_hash(&content);

        // Ensure the wrong hash is actually different
        prop_assume!(wrong_hash_bytes != correct_hash);

        let verifier = ArtifactVerifier::new();
        let result = verifier.verify(&artifact_path, &wrong_hash_bytes);

        prop_assert!(result.is_err(), "Incorrect hash should be rejected");
    }

    /// **Feature: zrc-updater, Property 2: Hash Computation Deterministic**
    /// **Validates: Requirements 2.2**
    ///
    /// For any artifact content, computing the hash twice should yield
    /// the same result.
    #[test]
    fn prop_hash_computation_deterministic(content in arb_artifact_content()) {
        let temp_dir = TempDir::new().unwrap();
        let artifact_path = temp_dir.path().join("artifact.bin");
        std::fs::write(&artifact_path, &content).unwrap();

        let verifier = ArtifactVerifier::new();

        let hash1 = verifier.compute_hash(&artifact_path).unwrap();
        let hash2 = verifier.compute_hash(&artifact_path).unwrap();

        prop_assert_eq!(hash1, hash2, "Hash computation should be deterministic");
    }
}

// =============================================================================
// Property 3: Rollback Availability
// **Validates: Requirements 9.1, 9.2, 9.3**
//
// *For any* update installation, a backup of the previous version SHALL
// exist until the update is confirmed successful.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: zrc-updater, Property 3: Backup Created Successfully**
    /// **Validates: Requirements 9.1**
    ///
    /// For any file content and version, creating a backup should succeed
    /// and the backup should be retrievable.
    #[test]
    fn prop_backup_created_and_retrievable(
        content in arb_artifact_content(),
        version in arb_version(),
    ) {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let source_file = temp_dir.path().join("source.bin");

        std::fs::write(&source_file, &content).unwrap();

        let manager = RollbackManager::new(backup_dir, 3);
        let backup = manager.backup_file(&source_file, version.clone()).unwrap();

        prop_assert!(backup.exists(), "Backup should exist after creation");
        prop_assert_eq!(&backup.version, &version, "Backup version should match");

        // Verify backup content matches original
        let backup_content = std::fs::read(backup.executable_path()).unwrap();
        prop_assert_eq!(backup_content, content, "Backup content should match original");
    }

    /// **Feature: zrc-updater, Property 3: Backup Listed After Creation**
    /// **Validates: Requirements 9.4**
    ///
    /// For any backup created, it should appear in the list of backups.
    #[test]
    fn prop_backup_listed_after_creation(
        content in arb_artifact_content(),
        version in arb_version(),
    ) {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let source_file = temp_dir.path().join("source.bin");

        std::fs::write(&source_file, &content).unwrap();

        let manager = RollbackManager::new(backup_dir, 3);
        let _backup = manager.backup_file(&source_file, version.clone()).unwrap();

        let backups = manager.list_backups().unwrap();
        prop_assert!(!backups.is_empty(), "Backup list should not be empty");

        let found = backups.iter().any(|b| b.version == version);
        prop_assert!(found, "Created backup should be in the list");
    }

    /// **Feature: zrc-updater, Property 3: Backup Integrity Verified**
    /// **Validates: Requirements 9.5**
    ///
    /// For any backup created, its integrity should be verifiable.
    #[test]
    fn prop_backup_integrity_verified(
        content in arb_artifact_content(),
        version in arb_version(),
    ) {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let source_file = temp_dir.path().join("source.bin");

        std::fs::write(&source_file, &content).unwrap();

        let manager = RollbackManager::new(backup_dir, 3);
        let backup = manager.backup_file(&source_file, version).unwrap();

        // Backup should have a hash
        prop_assert!(backup.hash.is_some(), "Backup should have integrity hash");

        // Hash should match the content
        let expected_hash = hex::encode(compute_hash(&content));
        prop_assert_eq!(backup.hash.as_ref().unwrap(), &expected_hash, "Backup hash should match content");
    }

    /// **Feature: zrc-updater, Property 3: Old Backups Cleaned Up**
    /// **Validates: Requirements 9.8**
    ///
    /// For any number of backups exceeding max_backups, only max_backups
    /// should be retained.
    #[test]
    fn prop_old_backups_cleaned_up(
        content in arb_artifact_content(),
    ) {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let source_file = temp_dir.path().join("source.bin");

        std::fs::write(&source_file, &content).unwrap();

        let max_backups = 2;
        let manager = RollbackManager::new(backup_dir, max_backups);

        // Create more backups than max_backups
        for i in 0..5 {
            let version = Version::new(1, i, 0);
            manager.backup_file(&source_file, version).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let backups = manager.list_backups().unwrap();
        prop_assert!(
            backups.len() <= max_backups,
            "Should have at most {} backups, got {}",
            max_backups,
            backups.len()
        );
    }
}

// =============================================================================
// Property 5: Channel Isolation
// **Validates: Requirements 3.1, 3.7**
//
// *For any* update check, only manifests from the configured channel
// SHALL be accepted.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: zrc-updater, Property 5: Channel URL Correct**
    /// **Validates: Requirements 3.7**
    ///
    /// For any channel, the manifest URL should contain the channel name.
    #[test]
    fn prop_channel_url_contains_channel_name(channel in arb_channel()) {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("channel.json");

        let manager = ChannelManager::with_channel(config_path, channel.clone());
        let url = manager.manifest_url();

        let channel_str = channel.to_string().to_lowercase();
        prop_assert!(
            url.to_lowercase().contains(&channel_str),
            "URL '{}' should contain channel '{}'",
            url,
            channel_str
        );
    }

    /// **Feature: zrc-updater, Property 5: Channel Persisted**
    /// **Validates: Requirements 3.4**
    ///
    /// For any channel set, saving and loading should preserve the channel.
    #[test]
    fn prop_channel_persisted(channel in arb_channel()) {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("channel.json");

        // Create with default and then set channel (which saves)
        let mut manager = ChannelManager::with_channel(config_path.clone(), UpdateChannel::Stable);
        manager.set_channel(channel.clone()).unwrap();

        // Load and verify
        let loaded = ChannelManager::load(config_path).unwrap();
        prop_assert_eq!(
            loaded.current_channel(),
            &channel,
            "Loaded channel should match saved channel"
        );
    }

    /// **Feature: zrc-updater, Property 5: Channel Switch Updates URL**
    /// **Validates: Requirements 3.5, 3.7**
    ///
    /// For any two different channels, switching channels should change
    /// the manifest URL.
    #[test]
    fn prop_channel_switch_updates_url(
        channel1 in arb_channel(),
        channel2 in arb_channel(),
    ) {
        // Only test if channels are different
        prop_assume!(channel1 != channel2);

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("channel.json");

        let mut manager = ChannelManager::with_channel(config_path, channel1);
        let url1 = manager.manifest_url();

        manager.set_channel(channel2).unwrap();
        let url2 = manager.manifest_url();

        prop_assert_ne!(url1, url2, "Different channels should have different URLs");
    }
}
