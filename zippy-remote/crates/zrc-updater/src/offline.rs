//! Offline update support.
//!
//! This module provides functionality for updating ZRC in environments
//! without internet access. It supports:
//! - Importing update files from local storage (USB, network share)
//! - Exporting update packages for distribution
//! - Verifying offline update files
//!
//! # Requirements
//! - Requirement 10.1: Support manual update file import
//! - Requirement 10.2: Verify imported update files
//! - Requirement 10.4: Provide update file export for distribution
//! - Requirement 10.5: Document offline update process
//! - Requirement 10.6: Verify offline update signatures

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use semver::Version;
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::artifact::ArtifactVerifier;
use crate::channel::UpdateChannel;
use crate::error::UpdateError;
use crate::manifest::{ManifestVerifier, SignedManifest, UpdateManifest};

/// Magic bytes for offline update package files.
/// "ZRCU" in ASCII (Zippy Remote Control Update)
const PACKAGE_MAGIC: &[u8; 4] = b"ZRCU";

/// Current package format version.
const PACKAGE_VERSION: u8 = 1;

/// Offline update package containing manifest and artifact.
///
/// The package format is:
/// - 4 bytes: Magic ("ZRCU")
/// - 1 byte: Package version
/// - 4 bytes: Manifest length (big-endian u32)
/// - N bytes: Signed manifest JSON
/// - Remaining: Artifact binary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineUpdatePackage {
    /// The signed manifest
    pub manifest: SignedManifest,
    /// Path to the artifact file (when loaded)
    #[serde(skip)]
    pub artifact_path: Option<PathBuf>,
    /// Artifact data (when in memory)
    #[serde(skip)]
    pub artifact_data: Option<Vec<u8>>,
}

impl OfflineUpdatePackage {
    /// Create a new offline update package.
    ///
    /// # Arguments
    ///
    /// * `manifest` - The signed update manifest
    /// * `artifact_data` - The update artifact binary data
    pub fn new(manifest: SignedManifest, artifact_data: Vec<u8>) -> Self {
        Self {
            manifest,
            artifact_path: None,
            artifact_data: Some(artifact_data),
        }
    }

    /// Get the artifact data if available.
    pub fn artifact_data(&self) -> Option<&[u8]> {
        self.artifact_data.as_deref()
    }

    /// Get the artifact path if available.
    pub fn artifact_path(&self) -> Option<&Path> {
        self.artifact_path.as_deref()
    }
}

/// Manages offline update operations.
///
/// # Requirements
/// - Requirement 10.1: Support manual update file import
/// - Requirement 10.2: Verify imported update files
/// - Requirement 10.4: Provide update file export for distribution
///
/// # Example
///
/// ```ignore
/// use zrc_updater::offline::OfflineUpdateManager;
///
/// let manager = OfflineUpdateManager::new(manifest_verifier, artifact_verifier);
///
/// // Import an update file
/// let package = manager.import_update_file(Path::new("/media/usb/update.zrcu"))?;
///
/// // Export an update for distribution
/// manager.export_update_file(&manifest, &artifact_path, Path::new("update.zrcu"))?;
/// ```
pub struct OfflineUpdateManager {
    /// Manifest verifier for signature verification
    manifest_verifier: ManifestVerifier,
    /// Artifact verifier for hash verification
    artifact_verifier: ArtifactVerifier,
    /// Staging directory for extracted artifacts
    staging_dir: PathBuf,
}

impl OfflineUpdateManager {
    /// Create a new offline update manager.
    ///
    /// # Arguments
    ///
    /// * `manifest_verifier` - Verifier for manifest signatures
    /// * `artifact_verifier` - Verifier for artifact hashes
    /// * `staging_dir` - Directory for staging extracted artifacts
    pub fn new(
        manifest_verifier: ManifestVerifier,
        artifact_verifier: ArtifactVerifier,
        staging_dir: PathBuf,
    ) -> Self {
        Self {
            manifest_verifier,
            artifact_verifier,
            staging_dir,
        }
    }

    /// Get the staging directory.
    pub fn staging_dir(&self) -> &Path {
        &self.staging_dir
    }

    /// Import and verify an offline update file.
    ///
    /// This method:
    /// 1. Reads the update package file
    /// 2. Verifies the package format
    /// 3. Extracts and verifies the manifest signature
    /// 4. Extracts the artifact to staging directory
    /// 5. Verifies the artifact hash
    ///
    /// # Requirements
    /// - Requirement 10.1: Support manual update file import
    /// - Requirement 10.2: Verify imported update files
    /// - Requirement 10.6: Verify offline update signatures
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the offline update package file (.zrcu)
    ///
    /// # Returns
    ///
    /// The verified update manifest and path to the extracted artifact.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be read
    /// - Package format is invalid
    /// - Manifest signature verification fails
    /// - Artifact hash verification fails
    pub fn import_update_file(&self, path: &Path) -> Result<(UpdateManifest, PathBuf), UpdateError> {
        info!("Importing offline update from: {:?}", path);

        // Read the package file
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();

        // Verify minimum size (magic + version + manifest length)
        if file_size < 9 {
            return Err(UpdateError::ConfigError(
                "Invalid update package: file too small".to_string(),
            ));
        }

        // Read and verify magic bytes
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if &magic != PACKAGE_MAGIC {
            return Err(UpdateError::ConfigError(
                "Invalid update package: wrong magic bytes".to_string(),
            ));
        }

        // Read and verify version
        let mut version = [0u8; 1];
        file.read_exact(&mut version)?;
        if version[0] != PACKAGE_VERSION {
            return Err(UpdateError::ConfigError(format!(
                "Unsupported package version: {} (expected {})",
                version[0], PACKAGE_VERSION
            )));
        }

        // Read manifest length
        let mut manifest_len_bytes = [0u8; 4];
        file.read_exact(&mut manifest_len_bytes)?;
        let manifest_len = u32::from_be_bytes(manifest_len_bytes) as usize;

        // Sanity check manifest length
        if manifest_len > 10 * 1024 * 1024 {
            // 10MB max for manifest
            return Err(UpdateError::ConfigError(
                "Invalid update package: manifest too large".to_string(),
            ));
        }

        // Read manifest data
        let mut manifest_data = vec![0u8; manifest_len];
        file.read_exact(&mut manifest_data)?;

        // Verify and parse manifest (Requirement 10.6)
        debug!("Verifying manifest signature...");
        let manifest = self.manifest_verifier.verify_and_parse(&manifest_data)?;
        info!(
            "Manifest verified: version={}, platform={}",
            manifest.version, manifest.platform
        );

        // Create staging directory if needed
        fs::create_dir_all(&self.staging_dir)?;

        // Generate artifact filename
        let artifact_filename = format!(
            "update-{}-{}.bin",
            manifest.version,
            chrono::Utc::now().timestamp()
        );
        let artifact_path = self.staging_dir.join(&artifact_filename);

        // Read and write artifact to staging
        debug!("Extracting artifact to: {:?}", artifact_path);
        let mut artifact_file = File::create(&artifact_path)?;
        let mut buffer = [0u8; 8192];
        let mut total_written = 0u64;

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            artifact_file.write_all(&buffer[..bytes_read])?;
            total_written += bytes_read as u64;
        }
        artifact_file.flush()?;

        // Verify artifact size
        if total_written != manifest.artifact_size {
            // Clean up
            let _ = fs::remove_file(&artifact_path);
            return Err(UpdateError::SizeMismatch {
                expected: manifest.artifact_size,
                actual: total_written,
            });
        }

        // Verify artifact hash (Requirement 10.2)
        debug!("Verifying artifact hash...");
        let expected_hash = manifest.artifact_hash_bytes().ok_or_else(|| {
            UpdateError::ConfigError("Invalid artifact hash in manifest".to_string())
        })?;

        if let Err(e) = self.artifact_verifier.verify(&artifact_path, &expected_hash) {
            // Clean up on verification failure
            let _ = fs::remove_file(&artifact_path);
            return Err(e);
        }

        info!(
            "Offline update imported successfully: version={}, size={}",
            manifest.version, total_written
        );

        Ok((manifest, artifact_path))
    }


    /// Export an update package for offline distribution.
    ///
    /// This method creates a self-contained update package that can be
    /// transferred via USB drive, network share, or other offline means.
    ///
    /// # Requirements
    /// - Requirement 10.4: Provide update file export for distribution
    ///
    /// # Arguments
    ///
    /// * `signed_manifest` - The signed update manifest
    /// * `artifact_path` - Path to the update artifact
    /// * `output_path` - Path where the package should be written
    ///
    /// # Returns
    ///
    /// The size of the created package in bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Artifact file cannot be read
    /// - Output file cannot be written
    /// - Artifact hash doesn't match manifest
    pub fn export_update_file(
        &self,
        signed_manifest: &SignedManifest,
        artifact_path: &Path,
        output_path: &Path,
    ) -> Result<u64, UpdateError> {
        info!("Exporting offline update to: {:?}", output_path);

        // Parse manifest to get expected hash
        let manifest: UpdateManifest = serde_json::from_str(&signed_manifest.manifest)?;

        // Verify artifact before packaging
        let expected_hash = manifest.artifact_hash_bytes().ok_or_else(|| {
            UpdateError::ConfigError("Invalid artifact hash in manifest".to_string())
        })?;

        debug!("Verifying artifact before export...");
        self.artifact_verifier.verify(artifact_path, &expected_hash)?;

        // Verify artifact size
        let artifact_metadata = fs::metadata(artifact_path)?;
        if artifact_metadata.len() != manifest.artifact_size {
            return Err(UpdateError::SizeMismatch {
                expected: manifest.artifact_size,
                actual: artifact_metadata.len(),
            });
        }

        // Serialize manifest
        let manifest_json = serde_json::to_vec(signed_manifest)?;
        let manifest_len = manifest_json.len() as u32;

        // Create output file
        let mut output_file = File::create(output_path)?;

        // Write header
        output_file.write_all(PACKAGE_MAGIC)?;
        output_file.write_all(&[PACKAGE_VERSION])?;
        output_file.write_all(&manifest_len.to_be_bytes())?;

        // Write manifest
        output_file.write_all(&manifest_json)?;

        // Write artifact
        let mut artifact_file = File::open(artifact_path)?;
        let mut buffer = [0u8; 8192];
        loop {
            let bytes_read = artifact_file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            output_file.write_all(&buffer[..bytes_read])?;
        }

        output_file.flush()?;

        // Get final size
        let output_metadata = fs::metadata(output_path)?;
        let package_size = output_metadata.len();

        info!(
            "Offline update exported: version={}, package_size={}",
            manifest.version, package_size
        );

        Ok(package_size)
    }

    /// Verify an offline update file without extracting.
    ///
    /// This is useful for checking if an update file is valid before
    /// committing to the full import process.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the offline update package file
    ///
    /// # Returns
    ///
    /// Information about the update if valid.
    ///
    /// # Errors
    ///
    /// Returns an error if the package is invalid or verification fails.
    pub fn verify_update_file(&self, path: &Path) -> Result<OfflineUpdateInfo, UpdateError> {
        info!("Verifying offline update file: {:?}", path);

        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();

        // Verify minimum size
        if file_size < 9 {
            return Err(UpdateError::ConfigError(
                "Invalid update package: file too small".to_string(),
            ));
        }

        // Read and verify magic bytes
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if &magic != PACKAGE_MAGIC {
            return Err(UpdateError::ConfigError(
                "Invalid update package: wrong magic bytes".to_string(),
            ));
        }

        // Read and verify version
        let mut version = [0u8; 1];
        file.read_exact(&mut version)?;
        if version[0] != PACKAGE_VERSION {
            return Err(UpdateError::ConfigError(format!(
                "Unsupported package version: {} (expected {})",
                version[0], PACKAGE_VERSION
            )));
        }

        // Read manifest length
        let mut manifest_len_bytes = [0u8; 4];
        file.read_exact(&mut manifest_len_bytes)?;
        let manifest_len = u32::from_be_bytes(manifest_len_bytes) as usize;

        // Sanity check
        if manifest_len > 10 * 1024 * 1024 {
            return Err(UpdateError::ConfigError(
                "Invalid update package: manifest too large".to_string(),
            ));
        }

        // Read manifest data
        let mut manifest_data = vec![0u8; manifest_len];
        file.read_exact(&mut manifest_data)?;

        // Verify manifest signature
        let manifest = self.manifest_verifier.verify_and_parse(&manifest_data)?;

        // Calculate expected artifact size from file
        let header_size = 4 + 1 + 4 + manifest_len as u64;
        let artifact_size_in_file = file_size - header_size;

        // Verify artifact size matches manifest
        if artifact_size_in_file != manifest.artifact_size {
            return Err(UpdateError::SizeMismatch {
                expected: manifest.artifact_size,
                actual: artifact_size_in_file,
            });
        }

        // Compute artifact hash without extracting
        debug!("Computing artifact hash...");
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        let actual_hash: [u8; 32] = hasher.finalize().into();

        // Verify hash
        let expected_hash = manifest.artifact_hash_bytes().ok_or_else(|| {
            UpdateError::ConfigError("Invalid artifact hash in manifest".to_string())
        })?;

        if actual_hash != expected_hash {
            return Err(UpdateError::HashMismatch {
                expected: hex::encode(expected_hash),
                actual: hex::encode(actual_hash),
            });
        }

        info!(
            "Offline update file verified: version={}, size={}",
            manifest.version, file_size
        );

        Ok(OfflineUpdateInfo {
            version: manifest.version.clone(),
            platform: manifest.platform.clone(),
            channel: manifest.channel.clone(),
            artifact_size: manifest.artifact_size,
            package_size: file_size,
            is_security_update: manifest.is_security_update,
            release_notes: manifest.release_notes.clone(),
        })
    }

    /// Clean up staging directory.
    ///
    /// Removes all files from the staging directory.
    pub fn cleanup_staging(&self) -> Result<(), UpdateError> {
        if self.staging_dir.exists() {
            for entry in fs::read_dir(&self.staging_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    fs::remove_file(&path)?;
                }
            }
        }
        Ok(())
    }
}

/// Information about an offline update file.
#[derive(Debug, Clone)]
pub struct OfflineUpdateInfo {
    /// Version of the update
    pub version: Version,
    /// Target platform
    pub platform: String,
    /// Update channel
    pub channel: UpdateChannel,
    /// Size of the artifact in bytes
    pub artifact_size: u64,
    /// Total package size in bytes
    pub package_size: u64,
    /// Whether this is a security update
    pub is_security_update: bool,
    /// Release notes
    pub release_notes: String,
}

impl OfflineUpdateInfo {
    /// Check if this update is for the current platform.
    pub fn is_current_platform(&self) -> bool {
        self.platform == crate::manifest::current_platform()
    }
}

/// Get the recommended file extension for offline update packages.
pub fn package_extension() -> &'static str {
    "zrcu"
}

/// Generate a filename for an offline update package.
///
/// # Arguments
///
/// * `version` - The update version
/// * `platform` - The target platform
/// * `channel` - The update channel
pub fn generate_package_filename(version: &Version, platform: &str, channel: &UpdateChannel) -> String {
    format!(
        "zrc-update-{}-{}-{}.{}",
        version,
        platform,
        channel,
        package_extension()
    )
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ManifestSignature;
    use ed25519_dalek::{Signer, SigningKey};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::TempDir;

    /// Create a test keypair
    fn create_test_keypair() -> (SigningKey, ed25519_dalek::VerifyingKey) {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    /// Get current timestamp
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Create a test manifest
    fn create_test_manifest(platform: &str, artifact_hash: &str, artifact_size: u64) -> String {
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

    /// Create a signed manifest
    fn create_signed_manifest(
        manifest_json: &str,
        signing_key: &SigningKey,
        timestamp: u64,
    ) -> SignedManifest {
        let signature = signing_key.sign(manifest_json.as_bytes());
        SignedManifest::new(
            manifest_json.to_string(),
            vec![ManifestSignature::new("key1".to_string(), signature)],
            timestamp,
        )
    }

    /// Compute SHA-256 hash of data
    fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash: [u8; 32] = hasher.finalize().into();
        hex::encode(hash)
    }

    #[test]
    fn test_package_extension() {
        assert_eq!(package_extension(), "zrcu");
    }

    #[test]
    fn test_generate_package_filename() {
        let version = Version::new(1, 2, 3);
        let filename = generate_package_filename(&version, "windows-x86_64", &UpdateChannel::Stable);
        assert_eq!(filename, "zrc-update-1.2.3-windows-x86_64-stable.zrcu");
    }

    #[test]
    fn test_generate_package_filename_beta() {
        let version = Version::new(2, 0, 0);
        let filename = generate_package_filename(&version, "linux-x86_64", &UpdateChannel::Beta);
        assert_eq!(filename, "zrc-update-2.0.0-linux-x86_64-beta.zrcu");
    }

    #[test]
    fn test_offline_update_package_new() {
        let (signing_key, _) = create_test_keypair();
        let manifest_json = create_test_manifest("test", "abc123", 100);
        let signed_manifest = create_signed_manifest(&manifest_json, &signing_key, current_timestamp());
        
        let artifact_data = vec![1, 2, 3, 4, 5];
        let package = OfflineUpdatePackage::new(signed_manifest, artifact_data.clone());
        
        assert!(package.artifact_path().is_none());
        assert_eq!(package.artifact_data(), Some(artifact_data.as_slice()));
    }

    #[test]
    fn test_offline_update_info_is_current_platform() {
        let current = crate::manifest::current_platform();
        
        let info = OfflineUpdateInfo {
            version: Version::new(1, 0, 0),
            platform: current.clone(),
            channel: UpdateChannel::Stable,
            artifact_size: 1000,
            package_size: 2000,
            is_security_update: false,
            release_notes: "Test".to_string(),
        };
        
        assert!(info.is_current_platform());
        
        let info_other = OfflineUpdateInfo {
            version: Version::new(1, 0, 0),
            platform: "other-platform".to_string(),
            channel: UpdateChannel::Stable,
            artifact_size: 1000,
            package_size: 2000,
            is_security_update: false,
            release_notes: "Test".to_string(),
        };
        
        assert!(!info_other.is_current_platform());
    }

    #[test]
    fn test_export_and_import_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let staging_dir = temp_dir.path().join("staging");
        fs::create_dir_all(&staging_dir).unwrap();

        let (signing_key, verifying_key) = create_test_keypair();

        // Create test artifact
        let artifact_data = b"This is test artifact data for the update package.";
        let artifact_hash = compute_hash(artifact_data);
        let artifact_path = temp_dir.path().join("artifact.bin");
        fs::write(&artifact_path, artifact_data).unwrap();

        // Create manifest
        let platform = crate::manifest::current_platform();
        let manifest_json = create_test_manifest(&platform, &artifact_hash, artifact_data.len() as u64);
        let signed_manifest = create_signed_manifest(&manifest_json, &signing_key, current_timestamp());

        // Create manager
        let manifest_verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let artifact_verifier = ArtifactVerifier::new();
        let manager = OfflineUpdateManager::new(manifest_verifier, artifact_verifier, staging_dir.clone());

        // Export
        let package_path = temp_dir.path().join("update.zrcu");
        let package_size = manager
            .export_update_file(&signed_manifest, &artifact_path, &package_path)
            .unwrap();

        assert!(package_path.exists());
        assert!(package_size > 0);

        // Verify
        let info = manager.verify_update_file(&package_path).unwrap();
        assert_eq!(info.version, Version::new(2, 0, 0));
        assert_eq!(info.platform, platform);
        assert_eq!(info.artifact_size, artifact_data.len() as u64);

        // Import
        let (manifest, extracted_path) = manager.import_update_file(&package_path).unwrap();
        assert_eq!(manifest.version, Version::new(2, 0, 0));
        assert!(extracted_path.exists());

        // Verify extracted artifact matches original
        let extracted_data = fs::read(&extracted_path).unwrap();
        assert_eq!(extracted_data, artifact_data);

        // Cleanup
        manager.cleanup_staging().unwrap();
    }

    #[test]
    fn test_import_invalid_magic() {
        let temp_dir = TempDir::new().unwrap();
        let staging_dir = temp_dir.path().join("staging");
        fs::create_dir_all(&staging_dir).unwrap();

        let (_, verifying_key) = create_test_keypair();
        let manifest_verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let artifact_verifier = ArtifactVerifier::new();
        let manager = OfflineUpdateManager::new(manifest_verifier, artifact_verifier, staging_dir);

        // Create file with wrong magic
        let bad_file = temp_dir.path().join("bad.zrcu");
        fs::write(&bad_file, b"BADM12345").unwrap();

        let result = manager.import_update_file(&bad_file);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("wrong magic bytes"));
    }

    #[test]
    fn test_import_unsupported_version() {
        let temp_dir = TempDir::new().unwrap();
        let staging_dir = temp_dir.path().join("staging");
        fs::create_dir_all(&staging_dir).unwrap();

        let (_, verifying_key) = create_test_keypair();
        let manifest_verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let artifact_verifier = ArtifactVerifier::new();
        let manager = OfflineUpdateManager::new(manifest_verifier, artifact_verifier, staging_dir);

        // Create file with unsupported version
        let bad_file = temp_dir.path().join("bad.zrcu");
        let mut data = Vec::new();
        data.extend_from_slice(PACKAGE_MAGIC);
        data.push(99); // Unsupported version
        data.extend_from_slice(&[0, 0, 0, 10]); // Manifest length
        data.extend_from_slice(b"0123456789"); // Fake manifest
        fs::write(&bad_file, &data).unwrap();

        let result = manager.import_update_file(&bad_file);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported package version"));
    }

    #[test]
    fn test_import_file_too_small() {
        let temp_dir = TempDir::new().unwrap();
        let staging_dir = temp_dir.path().join("staging");
        fs::create_dir_all(&staging_dir).unwrap();

        let (_, verifying_key) = create_test_keypair();
        let manifest_verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let artifact_verifier = ArtifactVerifier::new();
        let manager = OfflineUpdateManager::new(manifest_verifier, artifact_verifier, staging_dir);

        // Create file that's too small
        let bad_file = temp_dir.path().join("small.zrcu");
        fs::write(&bad_file, b"ZRCU").unwrap();

        let result = manager.import_update_file(&bad_file);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("file too small"));
    }

    #[test]
    fn test_verify_hash_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let staging_dir = temp_dir.path().join("staging");
        fs::create_dir_all(&staging_dir).unwrap();

        let (signing_key, verifying_key) = create_test_keypair();

        // Create artifact with one hash
        let artifact_data = b"Original artifact data";
        let artifact_path = temp_dir.path().join("artifact.bin");
        fs::write(&artifact_path, artifact_data).unwrap();

        // Create manifest with different hash
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let platform = crate::manifest::current_platform();
        let manifest_json = create_test_manifest(&platform, wrong_hash, artifact_data.len() as u64);
        let signed_manifest = create_signed_manifest(&manifest_json, &signing_key, current_timestamp());

        let manifest_verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let artifact_verifier = ArtifactVerifier::new();
        let manager = OfflineUpdateManager::new(manifest_verifier, artifact_verifier, staging_dir);

        // Export should fail due to hash mismatch
        let package_path = temp_dir.path().join("update.zrcu");
        let result = manager.export_update_file(&signed_manifest, &artifact_path, &package_path);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            UpdateError::HashMismatch { .. } => {}
            e => panic!("Expected HashMismatch, got {:?}", e),
        }
    }

    #[test]
    fn test_cleanup_staging() {
        let temp_dir = TempDir::new().unwrap();
        let staging_dir = temp_dir.path().join("staging");
        fs::create_dir_all(&staging_dir).unwrap();

        // Create some files in staging
        fs::write(staging_dir.join("file1.bin"), b"test1").unwrap();
        fs::write(staging_dir.join("file2.bin"), b"test2").unwrap();

        let (_, verifying_key) = create_test_keypair();
        let manifest_verifier = ManifestVerifier::new(vec![verifying_key], 1);
        let artifact_verifier = ArtifactVerifier::new();
        let manager = OfflineUpdateManager::new(manifest_verifier, artifact_verifier, staging_dir.clone());

        // Verify files exist
        assert!(staging_dir.join("file1.bin").exists());
        assert!(staging_dir.join("file2.bin").exists());

        // Cleanup
        manager.cleanup_staging().unwrap();

        // Verify files are gone
        assert!(!staging_dir.join("file1.bin").exists());
        assert!(!staging_dir.join("file2.bin").exists());
    }
}
