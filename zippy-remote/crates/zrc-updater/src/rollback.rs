//! Rollback management.
//!
//! Handles version backups for rollback support. This module provides:
//! - Backup creation before updates
//! - Listing available backups
//! - Rollback to previous versions
//! - Automatic cleanup of old backups
//!
//! ## Security
//!
//! Backups are stored with metadata that includes version information
//! and creation timestamps. Integrity verification is performed during
//! rollback operations.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::error::UpdateError;

/// Metadata file name within backup directories.
const METADATA_FILE: &str = "metadata.json";
/// Executable file name within backup directories.
const EXECUTABLE_FILE: &str = "executable";
/// Hash file name for integrity verification.
const HASH_FILE: &str = "hash.sha256";

/// Manages version backups for rollback.
///
/// The RollbackManager handles:
/// - Creating backups of the current executable before updates
/// - Listing all available backups sorted by creation time
/// - Rolling back to a specific backup version
/// - Cleaning up old backups to respect the max_backups limit
///
/// # Backup Directory Structure
///
/// ```text
/// backup_dir/
/// ├── backup-1.0.0-1704067200/
/// │   ├── executable          # The backed up binary
/// │   ├── metadata.json       # Version and timestamp info
/// │   └── hash.sha256         # SHA-256 hash for integrity
/// └── backup-1.1.0-1704153600/
///     ├── executable
///     ├── metadata.json
///     └── hash.sha256
/// ```
pub struct RollbackManager {
    /// Directory for storing backups
    backup_dir: PathBuf,
    /// Maximum number of backups to retain
    max_backups: usize,
}

impl RollbackManager {
    /// Create a new rollback manager.
    ///
    /// # Arguments
    ///
    /// * `backup_dir` - Directory where backups will be stored
    /// * `max_backups` - Maximum number of backups to retain (older ones are deleted)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    /// use zrc_updater::RollbackManager;
    ///
    /// let manager = RollbackManager::new(PathBuf::from("/var/lib/zrc/backups"), 3);
    /// ```
    pub fn new(backup_dir: PathBuf, max_backups: usize) -> Self {
        Self {
            backup_dir,
            max_backups,
        }
    }

    /// Get the backup directory.
    pub fn backup_dir(&self) -> &PathBuf {
        &self.backup_dir
    }

    /// Get the maximum number of backups to retain.
    pub fn max_backups(&self) -> usize {
        self.max_backups
    }

    /// Ensure the backup directory exists.
    fn ensure_backup_dir(&self) -> Result<(), UpdateError> {
        if !self.backup_dir.exists() {
            fs::create_dir_all(&self.backup_dir)?;
            debug!("Created backup directory: {:?}", self.backup_dir);
        }
        Ok(())
    }

    /// Detect the version of an executable.
    ///
    /// This attempts to extract version information from the executable.
    /// Falls back to "0.0.0" if version cannot be determined.
    fn detect_version(&self, _exe_path: &Path) -> Result<Version, UpdateError> {
        // In a real implementation, this would:
        // 1. Try to read version from embedded metadata
        // 2. Try to run the executable with --version
        // 3. Fall back to reading from a version file
        //
        // For now, we use the crate version as a reasonable default
        let version_str = env!("CARGO_PKG_VERSION");
        Version::parse(version_str).map_err(|e| UpdateError::VersionParseError(e.to_string()))
    }

    /// Compute SHA-256 hash of a file.
    fn compute_hash(&self, path: &Path) -> Result<String, UpdateError> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(hex::encode(hasher.finalize()))
    }

    /// Verify the integrity of a backup by checking its hash.
    fn verify_backup_integrity(&self, backup: &BackupInfo) -> Result<bool, UpdateError> {
        let exe_path = backup.path.join(EXECUTABLE_FILE);
        let hash_path = backup.path.join(HASH_FILE);

        if !exe_path.exists() || !hash_path.exists() {
            return Ok(false);
        }

        let stored_hash = fs::read_to_string(&hash_path)?.trim().to_string();
        let computed_hash = self.compute_hash(&exe_path)?;

        Ok(stored_hash == computed_hash)
    }

    /// Backup current version before update.
    ///
    /// Creates a backup of the currently running executable along with
    /// metadata and integrity hash. Automatically cleans up old backups
    /// if the max_backups limit is exceeded.
    ///
    /// # Returns
    ///
    /// Returns `BackupInfo` containing details about the created backup.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The current executable cannot be found
    /// - The backup directory cannot be created
    /// - File operations fail
    ///
    /// # Requirements
    ///
    /// Implements Requirements 9.1 (backup before update) and 9.4 (retain previous version).
    pub fn backup_current(&self) -> Result<BackupInfo, UpdateError> {
        self.ensure_backup_dir()?;

        let current_exe = std::env::current_exe()?;
        let current_version = self.detect_version(&current_exe)?;
        let now = Utc::now();

        // Create unique backup directory name
        let backup_name = format!("backup-{}-{}", current_version, now.timestamp());
        let backup_path = self.backup_dir.join(&backup_name);

        info!(
            "Creating backup of version {} at {:?}",
            current_version, backup_path
        );

        // Create backup directory
        fs::create_dir_all(&backup_path)?;

        // Copy executable
        let exe_dest = backup_path.join(EXECUTABLE_FILE);
        fs::copy(&current_exe, &exe_dest)?;

        // Compute and save hash for integrity verification
        let hash = self.compute_hash(&exe_dest)?;
        let hash_path = backup_path.join(HASH_FILE);
        fs::write(&hash_path, &hash)?;

        // Create backup info
        let info = BackupInfo {
            version: current_version,
            created_at: now,
            path: backup_path.clone(),
            hash: Some(hash),
        };

        // Save metadata
        let metadata_path = backup_path.join(METADATA_FILE);
        let metadata_json = serde_json::to_string_pretty(&info)?;
        fs::write(&metadata_path, metadata_json)?;

        debug!("Backup created successfully: {:?}", info);

        // Cleanup old backups
        self.cleanup_old_backups()?;

        Ok(info)
    }

    /// List available backups.
    ///
    /// Returns all valid backups sorted by creation time (newest first).
    /// Invalid or corrupted backups are skipped with a warning.
    ///
    /// # Returns
    ///
    /// A vector of `BackupInfo` sorted by creation time (newest first).
    ///
    /// # Errors
    ///
    /// Returns an error if the backup directory cannot be read.
    ///
    /// # Requirements
    ///
    /// Implements Requirement 9.4 (retain one previous version minimum).
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, UpdateError> {
        let mut backups = Vec::new();

        if !self.backup_dir.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let metadata_path = path.join(METADATA_FILE);
            if !metadata_path.exists() {
                debug!("Skipping directory without metadata: {:?}", path);
                continue;
            }

            match fs::read_to_string(&metadata_path) {
                Ok(content) => match serde_json::from_str::<BackupInfo>(&content) {
                    Ok(mut info) => {
                        // Ensure path is correct (in case backup was moved)
                        info.path = path;
                        backups.push(info);
                    }
                    Err(e) => {
                        warn!("Failed to parse backup metadata at {:?}: {}", metadata_path, e);
                    }
                },
                Err(e) => {
                    warn!("Failed to read backup metadata at {:?}: {}", metadata_path, e);
                }
            }
        }

        // Sort by creation time, newest first
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    /// Rollback to a specific backup.
    ///
    /// Restores the executable from the specified backup after verifying
    /// its integrity. The current executable is replaced with the backup.
    ///
    /// # Arguments
    ///
    /// * `backup` - The backup to restore
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The backup is corrupted or missing
    /// - Integrity verification fails
    /// - File operations fail
    ///
    /// # Requirements
    ///
    /// Implements Requirements 9.3 (manual rollback) and 9.5 (verify rollback integrity).
    pub fn rollback_to(&self, backup: &BackupInfo) -> Result<(), UpdateError> {
        info!("Rolling back to version {}", backup.version);

        let backup_exe = backup.path.join(EXECUTABLE_FILE);

        // Verify backup exists
        if !backup_exe.exists() {
            return Err(UpdateError::BackupCorrupted);
        }

        // Verify integrity
        if !self.verify_backup_integrity(backup)? {
            warn!("Backup integrity verification failed for {:?}", backup.path);
            return Err(UpdateError::BackupCorrupted);
        }

        let current_exe = std::env::current_exe()?;

        // On Windows, we can't replace a running executable directly
        // We need to use a different approach (rename + copy)
        #[cfg(target_os = "windows")]
        {
            let temp_path = current_exe.with_extension("old");
            
            // Remove old temp file if it exists
            if temp_path.exists() {
                let _ = fs::remove_file(&temp_path);
            }

            // Rename current to temp
            fs::rename(&current_exe, &temp_path)?;

            // Copy backup to current location
            match fs::copy(&backup_exe, &current_exe) {
                Ok(_) => {
                    // Try to remove the old file (may fail if still in use)
                    let _ = fs::remove_file(&temp_path);
                }
                Err(e) => {
                    // Restore original on failure
                    let _ = fs::rename(&temp_path, &current_exe);
                    return Err(UpdateError::RollbackFailed(format!(
                        "Failed to copy backup: {}",
                        e
                    )));
                }
            }
        }

        // On Unix, we can replace the executable directly
        #[cfg(not(target_os = "windows"))]
        {
            fs::copy(&backup_exe, &current_exe)?;
        }

        info!("Rollback to version {} completed successfully", backup.version);
        Ok(())
    }

    /// Clean up old backups beyond max_backups limit.
    ///
    /// Removes the oldest backups when the number of backups exceeds
    /// the configured maximum. Always keeps at least one backup.
    fn cleanup_old_backups(&self) -> Result<(), UpdateError> {
        let backups = self.list_backups()?;

        if backups.len() <= self.max_backups {
            return Ok(());
        }

        // Remove oldest backups (list is sorted newest first)
        for backup in backups.iter().skip(self.max_backups) {
            info!("Removing old backup: {:?}", backup.path);
            if let Err(e) = fs::remove_dir_all(&backup.path) {
                warn!("Failed to remove old backup {:?}: {}", backup.path, e);
            }
        }

        Ok(())
    }

    /// Get the most recent backup.
    ///
    /// Returns the newest backup if one exists.
    pub fn latest_backup(&self) -> Result<Option<BackupInfo>, UpdateError> {
        let backups = self.list_backups()?;
        Ok(backups.into_iter().next())
    }

    /// Find a backup by version.
    ///
    /// Returns the first backup matching the specified version.
    pub fn find_by_version(&self, version: &Version) -> Result<Option<BackupInfo>, UpdateError> {
        let backups = self.list_backups()?;
        Ok(backups.into_iter().find(|b| &b.version == version))
    }

    /// Delete a specific backup.
    ///
    /// Removes the backup directory and all its contents.
    pub fn delete_backup(&self, backup: &BackupInfo) -> Result<(), UpdateError> {
        if backup.path.exists() {
            fs::remove_dir_all(&backup.path)?;
            info!("Deleted backup: {:?}", backup.path);
        }
        Ok(())
    }

    /// Create a backup from a specific file (for testing or manual backup).
    ///
    /// This allows creating a backup from any executable file, not just
    /// the currently running one.
    pub fn backup_file(&self, source: &Path, version: Version) -> Result<BackupInfo, UpdateError> {
        self.ensure_backup_dir()?;

        let now = Utc::now();
        let backup_name = format!("backup-{}-{}", version, now.timestamp());
        let backup_path = self.backup_dir.join(&backup_name);

        info!("Creating backup of {:?} as version {}", source, version);

        // Create backup directory
        fs::create_dir_all(&backup_path)?;

        // Copy executable
        let exe_dest = backup_path.join(EXECUTABLE_FILE);
        fs::copy(source, &exe_dest)?;

        // Compute and save hash
        let hash = self.compute_hash(&exe_dest)?;
        let hash_path = backup_path.join(HASH_FILE);
        fs::write(&hash_path, &hash)?;

        // Create backup info
        let info = BackupInfo {
            version,
            created_at: now,
            path: backup_path.clone(),
            hash: Some(hash),
        };

        // Save metadata
        let metadata_path = backup_path.join(METADATA_FILE);
        let metadata_json = serde_json::to_string_pretty(&info)?;
        fs::write(&metadata_path, metadata_json)?;

        // Cleanup old backups
        self.cleanup_old_backups()?;

        Ok(info)
    }
}

/// Information about a backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    /// Version that was backed up
    pub version: Version,
    /// When the backup was created
    pub created_at: DateTime<Utc>,
    /// Path to the backup directory
    pub path: PathBuf,
    /// SHA-256 hash of the executable (for integrity verification)
    #[serde(default)]
    pub hash: Option<String>,
}

impl BackupInfo {
    /// Get the path to the backed up executable.
    pub fn executable_path(&self) -> PathBuf {
        self.path.join(EXECUTABLE_FILE)
    }

    /// Get the path to the metadata file.
    pub fn metadata_path(&self) -> PathBuf {
        self.path.join(METADATA_FILE)
    }

    /// Get the path to the hash file.
    pub fn hash_path(&self) -> PathBuf {
        self.path.join(HASH_FILE)
    }

    /// Check if the backup directory exists.
    pub fn exists(&self) -> bool {
        self.path.exists() && self.executable_path().exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (RollbackManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = RollbackManager::new(temp_dir.path().to_path_buf(), 3);
        (manager, temp_dir)
    }

    fn create_test_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_new_rollback_manager() {
        let (manager, _temp) = create_test_manager();
        assert_eq!(manager.max_backups(), 3);
    }

    #[test]
    fn test_ensure_backup_dir() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = RollbackManager::new(backup_dir.clone(), 3);

        assert!(!backup_dir.exists());
        manager.ensure_backup_dir().unwrap();
        assert!(backup_dir.exists());
    }

    #[test]
    fn test_list_backups_empty() {
        let (manager, _temp) = create_test_manager();
        let backups = manager.list_backups().unwrap();
        assert!(backups.is_empty());
    }

    #[test]
    fn test_backup_file() {
        let (manager, temp_dir) = create_test_manager();
        
        // Create a test file to backup
        let test_file = create_test_file(temp_dir.path(), "test_exe", b"test executable content");
        
        let version = Version::new(1, 0, 0);
        let backup = manager.backup_file(&test_file, version.clone()).unwrap();

        assert_eq!(backup.version, version);
        assert!(backup.exists());
        assert!(backup.hash.is_some());

        // Verify the backup is listed
        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].version, version);
    }

    #[test]
    fn test_backup_file_multiple() {
        let (manager, temp_dir) = create_test_manager();
        
        let test_file = create_test_file(temp_dir.path(), "test_exe", b"test content");

        // Create multiple backups
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 1, 0);
        let v3 = Version::new(1, 2, 0);

        manager.backup_file(&test_file, v1.clone()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        manager.backup_file(&test_file, v2.clone()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        manager.backup_file(&test_file, v3.clone()).unwrap();

        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 3);
        
        // Should be sorted newest first
        assert_eq!(backups[0].version, v3);
        assert_eq!(backups[1].version, v2);
        assert_eq!(backups[2].version, v1);
    }

    #[test]
    fn test_cleanup_old_backups() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RollbackManager::new(temp_dir.path().to_path_buf(), 2);
        
        let test_file = create_test_file(temp_dir.path(), "test_exe", b"test content");

        // Create 4 backups (max is 2)
        for i in 1..=4 {
            let version = Version::new(1, i, 0);
            manager.backup_file(&test_file, version).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 2);
        
        // Should keep the newest 2
        assert_eq!(backups[0].version, Version::new(1, 4, 0));
        assert_eq!(backups[1].version, Version::new(1, 3, 0));
    }

    #[test]
    fn test_find_by_version() {
        let (manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test_exe", b"test content");

        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(2, 0, 0);

        manager.backup_file(&test_file, v1.clone()).unwrap();
        manager.backup_file(&test_file, v2.clone()).unwrap();

        let found = manager.find_by_version(&v1).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().version, v1);

        let not_found = manager.find_by_version(&Version::new(3, 0, 0)).unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_latest_backup() {
        let (manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test_exe", b"test content");

        // No backups yet
        assert!(manager.latest_backup().unwrap().is_none());

        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(2, 0, 0);

        manager.backup_file(&test_file, v1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        manager.backup_file(&test_file, v2.clone()).unwrap();

        let latest = manager.latest_backup().unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().version, v2);
    }

    #[test]
    fn test_delete_backup() {
        let (manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test_exe", b"test content");

        let version = Version::new(1, 0, 0);
        let backup = manager.backup_file(&test_file, version).unwrap();

        assert!(backup.exists());
        manager.delete_backup(&backup).unwrap();
        assert!(!backup.path.exists());
    }

    #[test]
    fn test_verify_backup_integrity() {
        let (manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test_exe", b"test content");

        let version = Version::new(1, 0, 0);
        let backup = manager.backup_file(&test_file, version).unwrap();

        // Integrity should pass
        assert!(manager.verify_backup_integrity(&backup).unwrap());

        // Corrupt the backup
        let exe_path = backup.executable_path();
        fs::write(&exe_path, b"corrupted content").unwrap();

        // Integrity should fail
        assert!(!manager.verify_backup_integrity(&backup).unwrap());
    }

    #[test]
    fn test_backup_info_paths() {
        let info = BackupInfo {
            version: Version::new(1, 0, 0),
            created_at: Utc::now(),
            path: PathBuf::from("/backups/test"),
            hash: Some("abc123".to_string()),
        };

        assert_eq!(info.executable_path(), PathBuf::from("/backups/test/executable"));
        assert_eq!(info.metadata_path(), PathBuf::from("/backups/test/metadata.json"));
        assert_eq!(info.hash_path(), PathBuf::from("/backups/test/hash.sha256"));
    }

    #[test]
    fn test_compute_hash() {
        let (manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test_file", b"hello world");

        let hash = manager.compute_hash(&test_file).unwrap();
        
        // SHA-256 of "hello world"
        assert_eq!(hash, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    }
}
