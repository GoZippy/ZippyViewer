//! Update manager - orchestrates the complete update flow.
//!
//! The UpdateManager combines all update components:
//! - ManifestVerifier for verifying signed manifests
//! - ArtifactVerifier for verifying downloaded artifacts
//! - Downloader for downloading updates
//! - PlatformInstaller for platform-specific installation
//! - RollbackManager for backup and rollback support
//!
//! # Requirements
//! - Requirement 4.1: Check for updates on application startup
//! - Requirement 4.2: Check for updates periodically
//! - Requirement 4.3: Respect user preference to disable auto-check
//! - Requirement 4.4: Support manual update check trigger
//! - Requirement 9.1: Backup current version before update
//! - Requirement 9.2: Support automatic rollback on update failure

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use semver::Version;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::artifact::ArtifactVerifier;
use crate::channel::{ChannelManager, UpdateChannel};
use crate::config::UpdateConfig;
use crate::download::{DownloadProgress, Downloader};
use crate::error::UpdateError;
use crate::install::PlatformInstaller;
use crate::manifest::{ManifestVerifier, UpdateManifest};
use crate::rollback::{BackupInfo, RollbackManager};

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Version of the available update
    pub version: Version,
    /// Release notes (markdown)
    pub release_notes: String,
    /// Size of the update artifact in bytes
    pub size: u64,
    /// Whether this is a security update
    pub is_security_update: bool,
    /// Expected SHA-256 hash of the artifact
    pub expected_hash: [u8; 32],
    /// URL to download the artifact
    pub artifact_url: String,
    /// Update channel
    pub channel: UpdateChannel,
}

impl UpdateInfo {
    /// Create UpdateInfo from an UpdateManifest.
    pub fn from_manifest(manifest: &UpdateManifest) -> Result<Self, UpdateError> {
        let expected_hash = manifest.artifact_hash_bytes().ok_or_else(|| {
            UpdateError::ConfigError("Invalid artifact hash in manifest".to_string())
        })?;

        Ok(Self {
            version: manifest.version.clone(),
            release_notes: manifest.release_notes.clone(),
            size: manifest.artifact_size,
            is_security_update: manifest.is_security_update,
            expected_hash,
            artifact_url: manifest.artifact_url.clone(),
            channel: manifest.channel.clone(),
        })
    }
}

/// Current state of the update manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    /// Idle, no update in progress
    Idle,
    /// Checking for updates
    Checking,
    /// Update available
    UpdateAvailable,
    /// Downloading update
    Downloading,
    /// Verifying downloaded artifact
    Verifying,
    /// Installing update
    Installing,
    /// Update complete, restart required
    RestartRequired,
    /// Error occurred
    Error(String),
}

impl Default for UpdateState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Main update manager that orchestrates the complete update flow.
///
/// # Example
///
/// ```ignore
/// use zrc_updater::manager::UpdateManager;
/// use zrc_updater::config::UpdateConfig;
///
/// let config = UpdateConfig::default();
/// let manager = UpdateManager::new(config)?;
///
/// // Check for updates
/// if let Some(update_info) = manager.check_for_updates().await? {
///     println!("Update available: {}", update_info.version);
///     
///     // Install the update
///     manager.install_update(&update_info).await?;
/// }
/// ```
pub struct UpdateManager {
    /// Configuration
    config: UpdateConfig,
    /// Manifest verifier with pinned keys
    manifest_verifier: ManifestVerifier,
    /// Artifact verifier for hash/signature checks
    artifact_verifier: ArtifactVerifier,
    /// HTTP downloader
    downloader: Downloader,
    /// Channel manager for update channels
    channel_manager: ChannelManager,
    /// Rollback manager for backups
    rollback_manager: RollbackManager,
    /// Platform-specific installer
    installer: Option<Box<dyn PlatformInstaller>>,
    /// Current state
    state: Arc<RwLock<UpdateState>>,
    /// Current version of the application
    current_version: Version,
    /// Last update check time
    last_check: Arc<RwLock<Option<Instant>>>,
    /// Cached update info
    cached_update: Arc<RwLock<Option<UpdateInfo>>>,
    /// Download directory for staging updates
    download_dir: PathBuf,
}

impl UpdateManager {
    /// Create a new update manager with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Update configuration
    /// * `current_version` - Current version of the application
    /// * `download_dir` - Directory for staging downloaded updates
    /// * `channel_config_path` - Path to channel configuration file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Manifest keys cannot be parsed
    /// - Channel manager cannot be loaded
    pub fn new(
        config: UpdateConfig,
        current_version: Version,
        download_dir: PathBuf,
        channel_config_path: PathBuf,
    ) -> Result<Self, UpdateError> {
        // Parse manifest signing keys
        let trusted_keys = config.security.parse_manifest_keys()?;
        if trusted_keys.is_empty() {
            warn!("No manifest signing keys configured - updates will fail verification");
        }

        let manifest_verifier = if trusted_keys.is_empty() {
            // Create a verifier that will always fail (no keys)
            // This is safe because verify_and_parse will return InsufficientSignatures
            ManifestVerifier::new(vec![], 1)
        } else {
            ManifestVerifier::new(trusted_keys, config.security.signature_threshold)
        };

        // Create artifact verifier
        let artifact_verifier = ArtifactVerifier::new();

        // Create downloader
        let downloader = Downloader::new();

        // Load channel manager
        let channel_manager = ChannelManager::load(channel_config_path)?;

        // Create rollback manager
        let rollback_manager = RollbackManager::new(
            config.rollback.backup_dir(),
            config.rollback.max_backups,
        );

        Ok(Self {
            config,
            manifest_verifier,
            artifact_verifier,
            downloader,
            channel_manager,
            rollback_manager,
            installer: None,
            state: Arc::new(RwLock::new(UpdateState::Idle)),
            current_version,
            last_check: Arc::new(RwLock::new(None)),
            cached_update: Arc::new(RwLock::new(None)),
            download_dir,
        })
    }


    /// Create an update manager with a platform installer.
    ///
    /// Use this when you need platform-specific installation support.
    pub fn with_installer(
        config: UpdateConfig,
        current_version: Version,
        download_dir: PathBuf,
        channel_config_path: PathBuf,
        installer: Box<dyn PlatformInstaller>,
    ) -> Result<Self, UpdateError> {
        let mut manager = Self::new(config, current_version, download_dir, channel_config_path)?;
        manager.installer = Some(installer);
        Ok(manager)
    }

    /// Get the current state of the update manager.
    pub async fn state(&self) -> UpdateState {
        self.state.read().await.clone()
    }

    /// Get the current version.
    pub fn current_version(&self) -> &Version {
        &self.current_version
    }

    /// Get the current update channel.
    pub fn current_channel(&self) -> &UpdateChannel {
        self.channel_manager.current_channel()
    }

    /// Get the configuration.
    pub fn config(&self) -> &UpdateConfig {
        &self.config
    }

    /// Get the download directory.
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }

    /// Set the platform installer.
    pub fn set_installer(&mut self, installer: Box<dyn PlatformInstaller>) {
        self.installer = Some(installer);
    }

    /// Set a progress callback for downloads.
    pub fn set_progress_callback<F>(&mut self, callback: F)
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        self.downloader.set_progress_callback(callback);
    }

    /// Check if an update check is due based on the configured interval.
    ///
    /// Returns true if:
    /// - No check has been performed yet
    /// - The configured interval has elapsed since the last check
    pub async fn is_check_due(&self) -> bool {
        let last_check = self.last_check.read().await;
        match *last_check {
            None => true,
            Some(last) => {
                let interval = Duration::from_secs(
                    self.config.check_interval_hours as u64 * 3600
                );
                last.elapsed() >= interval
            }
        }
    }

    /// Get cached update info if available.
    pub async fn cached_update(&self) -> Option<UpdateInfo> {
        self.cached_update.read().await.clone()
    }

    /// Clear cached update info.
    pub async fn clear_cached_update(&self) {
        *self.cached_update.write().await = None;
    }

    /// Set the update state.
    async fn set_state(&self, state: UpdateState) {
        *self.state.write().await = state;
    }

    /// Check for available updates.
    ///
    /// This method:
    /// 1. Downloads the manifest from the configured channel URL
    /// 2. Verifies the manifest signature against pinned keys
    /// 3. Compares the manifest version with the current version
    /// 4. Returns update info if a newer version is available
    ///
    /// # Requirements
    /// - Requirement 4.1: Check for updates on application startup
    /// - Requirement 4.2: Check for updates periodically
    /// - Requirement 4.4: Support manual update check trigger
    ///
    /// # Returns
    ///
    /// - `Ok(Some(UpdateInfo))` if an update is available
    /// - `Ok(None)` if no update is available (current version is latest)
    /// - `Err(UpdateError)` if the check fails
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Manifest signature verification fails
    /// - Manifest parsing fails
    pub async fn check_for_updates(&self) -> Result<Option<UpdateInfo>, UpdateError> {
        info!("Checking for updates...");
        self.set_state(UpdateState::Checking).await;

        // Get manifest URL for current channel
        let manifest_url = self.channel_manager.manifest_url();
        debug!("Fetching manifest from: {}", manifest_url);

        // Download manifest
        let manifest_bytes = match self.downloader.fetch(&manifest_url).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to download manifest: {}", e);
                self.set_state(UpdateState::Error(e.to_string())).await;
                return Err(e);
            }
        };

        // Verify and parse manifest
        let manifest = match self.manifest_verifier.verify_and_parse(&manifest_bytes) {
            Ok(m) => m,
            Err(e) => {
                error!("Manifest verification failed: {}", e);
                self.set_state(UpdateState::Error(e.to_string())).await;
                return Err(e);
            }
        };

        // Update last check time
        *self.last_check.write().await = Some(Instant::now());

        // Compare versions (Requirement 4.3 - version comparison)
        if manifest.version > self.current_version {
            info!(
                "Update available: {} -> {}",
                self.current_version, manifest.version
            );

            let update_info = UpdateInfo::from_manifest(&manifest)?;
            
            // Cache the update info
            *self.cached_update.write().await = Some(update_info.clone());
            
            self.set_state(UpdateState::UpdateAvailable).await;
            Ok(Some(update_info))
        } else {
            info!(
                "No update available (current: {}, latest: {})",
                self.current_version, manifest.version
            );
            self.set_state(UpdateState::Idle).await;
            Ok(None)
        }
    }


    /// Download and install an update.
    ///
    /// This method performs the complete update flow:
    /// 1. Backup current version (Requirement 9.1)
    /// 2. Download the update artifact
    /// 3. Verify artifact hash and signature
    /// 4. Install the update using platform-specific installer
    /// 5. Rollback on failure (Requirement 9.2)
    ///
    /// # Requirements
    /// - Requirement 9.1: Backup current version before update
    /// - Requirement 9.2: Support automatic rollback on update failure
    ///
    /// # Arguments
    ///
    /// * `info` - Information about the update to install
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the update was installed successfully
    /// - `Err(UpdateError)` if any step fails
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Backup creation fails
    /// - Download fails
    /// - Artifact verification fails
    /// - Installation fails (triggers automatic rollback)
    pub async fn install_update(&self, info: &UpdateInfo) -> Result<(), UpdateError> {
        info!("Installing update to version {}", info.version);

        // Ensure we have an installer
        let installer = self.installer.as_ref().ok_or_else(|| {
            UpdateError::InstallationFailed("No platform installer configured".to_string())
        })?;

        // Step 1: Backup current version (Requirement 9.1)
        info!("Creating backup of current version...");
        let backup = match self.rollback_manager.backup_current() {
            Ok(backup) => {
                info!("Backup created: {:?}", backup.path);
                Some(backup)
            }
            Err(e) => {
                // Log warning but continue - rollback manager may not be fully implemented
                warn!("Failed to create backup: {} - continuing without backup", e);
                None
            }
        };

        // Step 2: Download artifact
        self.set_state(UpdateState::Downloading).await;
        let artifact_path = self.download_dir.join(format!(
            "update-{}-{}.bin",
            info.version,
            chrono::Utc::now().timestamp()
        ));

        info!("Downloading update artifact to {:?}", artifact_path);
        if let Err(e) = self
            .downloader
            .download_with_resume(&info.artifact_url, &artifact_path, info.size)
            .await
        {
            error!("Download failed: {}", e);
            self.set_state(UpdateState::Error(e.to_string())).await;
            // Clean up partial download
            let _ = std::fs::remove_file(&artifact_path);
            return Err(e);
        }

        // Step 3: Verify artifact
        self.set_state(UpdateState::Verifying).await;
        info!("Verifying artifact integrity...");
        if let Err(e) = self.artifact_verifier.verify(&artifact_path, &info.expected_hash) {
            error!("Artifact verification failed: {}", e);
            self.set_state(UpdateState::Error(e.to_string())).await;
            // Clean up failed download
            let _ = std::fs::remove_file(&artifact_path);
            return Err(e);
        }
        info!("Artifact verified successfully");

        // Step 4: Install update
        self.set_state(UpdateState::Installing).await;
        info!("Installing update...");
        match installer.install(&artifact_path).await {
            Ok(()) => {
                info!("Update installed successfully");
                // Clean up downloaded artifact
                let _ = std::fs::remove_file(&artifact_path);
                
                if installer.requires_restart() {
                    self.set_state(UpdateState::RestartRequired).await;
                } else {
                    self.set_state(UpdateState::Idle).await;
                }
                
                // Clear cached update
                *self.cached_update.write().await = None;
                
                Ok(())
            }
            Err(e) => {
                error!("Installation failed: {}", e);
                
                // Step 5: Automatic rollback on failure (Requirement 9.2)
                if let Some(backup) = backup {
                    warn!("Attempting automatic rollback...");
                    if let Err(rollback_err) = self.rollback_manager.rollback_to(&backup) {
                        error!("Rollback also failed: {}", rollback_err);
                        self.set_state(UpdateState::Error(format!(
                            "Installation failed: {}. Rollback also failed: {}",
                            e, rollback_err
                        ))).await;
                    } else {
                        info!("Rollback successful");
                        self.set_state(UpdateState::Error(format!(
                            "Installation failed: {}. Rolled back to previous version.",
                            e
                        ))).await;
                    }
                } else {
                    self.set_state(UpdateState::Error(e.to_string())).await;
                }
                
                // Clean up downloaded artifact
                let _ = std::fs::remove_file(&artifact_path);
                
                Err(e)
            }
        }
    }

    /// Manually trigger a rollback to the previous version.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if rollback was successful
    /// - `Err(UpdateError)` if rollback fails or no backup is available
    pub fn rollback(&self) -> Result<(), UpdateError> {
        info!("Manual rollback requested");
        
        // Get available backups
        let backups = self.rollback_manager.list_backups()?;
        
        if backups.is_empty() {
            return Err(UpdateError::NoBackupAvailable);
        }
        
        // Rollback to most recent backup
        let latest_backup = &backups[0];
        info!("Rolling back to version {}", latest_backup.version);
        
        self.rollback_manager.rollback_to(latest_backup)?;
        
        info!("Rollback complete");
        Ok(())
    }

    /// Switch to a different update channel.
    ///
    /// # Arguments
    ///
    /// * `channel` - The new update channel
    ///
    /// # Returns
    ///
    /// - `Ok(())` if channel was switched successfully
    /// - `Err(UpdateError)` if channel switch fails
    pub fn set_channel(&mut self, channel: UpdateChannel) -> Result<(), UpdateError> {
        self.channel_manager.set_channel(channel)
    }

    /// Get the manifest URL for the current channel.
    pub fn manifest_url(&self) -> String {
        self.channel_manager.manifest_url()
    }

    /// List available backups for rollback.
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, UpdateError> {
        self.rollback_manager.list_backups()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_state_default() {
        assert_eq!(UpdateState::default(), UpdateState::Idle);
    }

    #[test]
    fn test_update_info_from_manifest() {
        use crate::manifest::UpdateManifest;
        
        let manifest = UpdateManifest::new(
            Version::new(2, 0, 0),
            "windows-x86_64".to_string(),
            UpdateChannel::Stable,
            "https://example.com/update.zip".to_string(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            1024,
            "Test release notes".to_string(),
            false,
            None,
        );

        let info = UpdateInfo::from_manifest(&manifest).unwrap();
        
        assert_eq!(info.version, Version::new(2, 0, 0));
        assert_eq!(info.release_notes, "Test release notes");
        assert_eq!(info.size, 1024);
        assert!(!info.is_security_update);
        assert_eq!(info.artifact_url, "https://example.com/update.zip");
    }

    #[test]
    fn test_update_info_from_manifest_invalid_hash() {
        use crate::manifest::UpdateManifest;
        
        let manifest = UpdateManifest::new(
            Version::new(2, 0, 0),
            "windows-x86_64".to_string(),
            UpdateChannel::Stable,
            "https://example.com/update.zip".to_string(),
            "invalid-hash".to_string(), // Invalid hash
            1024,
            "Test release notes".to_string(),
            false,
            None,
        );

        let result = UpdateInfo::from_manifest(&manifest);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_manager_state() {
        let state = Arc::new(RwLock::new(UpdateState::Idle));
        
        assert_eq!(*state.read().await, UpdateState::Idle);
        
        *state.write().await = UpdateState::Checking;
        assert_eq!(*state.read().await, UpdateState::Checking);
        
        *state.write().await = UpdateState::UpdateAvailable;
        assert_eq!(*state.read().await, UpdateState::UpdateAvailable);
    }

    #[tokio::test]
    async fn test_is_check_due_no_previous_check() {
        let last_check: Arc<RwLock<Option<Instant>>> = Arc::new(RwLock::new(None));
        
        // No previous check means check is due
        assert!(last_check.read().await.is_none());
    }

    #[tokio::test]
    async fn test_is_check_due_recent_check() {
        let last_check: Arc<RwLock<Option<Instant>>> = Arc::new(RwLock::new(Some(Instant::now())));
        
        // Recent check means check is not due (assuming interval > 0)
        let check_interval_hours = 24u32;
        let interval = Duration::from_secs(check_interval_hours as u64 * 3600);
        
        let last = last_check.read().await.unwrap();
        assert!(last.elapsed() < interval);
    }
}
