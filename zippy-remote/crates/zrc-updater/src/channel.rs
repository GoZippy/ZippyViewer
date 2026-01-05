//! Update channel management.
//!
//! Handles update channel selection and persistence.
//!
//! # Channels
//!
//! The updater supports multiple release channels:
//! - **Stable**: Production releases, most tested
//! - **Beta**: Pre-release testing, may have bugs
//! - **Nightly**: Development builds, least stable
//! - **Custom**: Enterprise/custom update servers
//!
//! # Persistence
//!
//! Channel selection is persisted to a JSON config file and
//! loaded on startup.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::UpdateError;

/// Update channel for release tracks.
///
/// Channels are ordered by stability: Stable > Beta > Nightly > Custom.
/// Switching to a less stable channel triggers a warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    /// Production releases - most stable, thoroughly tested
    Stable,
    /// Pre-release testing - may contain bugs
    Beta,
    /// Development builds - least stable, for testing only
    Nightly,
    /// Custom/enterprise channel with custom URL
    #[serde(rename = "custom")]
    Custom(String),
}

impl UpdateChannel {
    /// Get the stability level of this channel (higher = more stable).
    pub fn stability_level(&self) -> u8 {
        match self {
            Self::Stable => 3,
            Self::Beta => 2,
            Self::Nightly => 1,
            Self::Custom(_) => 0,
        }
    }

    /// Check if this channel is more stable than another.
    pub fn is_more_stable_than(&self, other: &Self) -> bool {
        self.stability_level() > other.stability_level()
    }
}

impl Default for UpdateChannel {
    fn default() -> Self {
        Self::Stable
    }
}

impl std::fmt::Display for UpdateChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stable => write!(f, "stable"),
            Self::Beta => write!(f, "beta"),
            Self::Nightly => write!(f, "nightly"),
            Self::Custom(url) => write!(f, "custom({})", url),
        }
    }
}

/// Manages update channel selection and persistence.
///
/// The ChannelManager handles:
/// - Loading/saving channel selection to disk
/// - Generating manifest URLs for each channel
/// - Warning when switching to less stable channels
///
/// # Example
///
/// ```no_run
/// use zrc_updater::channel::{ChannelManager, UpdateChannel};
/// use std::path::PathBuf;
///
/// // Load or create channel manager
/// let mut manager = ChannelManager::load(PathBuf::from("channel.json")).unwrap();
///
/// // Get current channel
/// println!("Current channel: {}", manager.current_channel());
///
/// // Get manifest URL
/// println!("Manifest URL: {}", manager.manifest_url());
///
/// // Switch to beta channel
/// manager.set_channel(UpdateChannel::Beta).unwrap();
/// ```
pub struct ChannelManager {
    /// Current update channel
    current_channel: UpdateChannel,
    /// Path to config file for persistence
    config_path: PathBuf,
}

impl ChannelManager {
    /// Create a new channel manager with default channel (Stable).
    ///
    /// The channel is not persisted until `set_channel` is called.
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            current_channel: UpdateChannel::default(),
            config_path,
        }
    }

    /// Create a channel manager with a specific initial channel.
    pub fn with_channel(config_path: PathBuf, channel: UpdateChannel) -> Self {
        Self {
            current_channel: channel,
            config_path,
        }
    }

    /// Load channel from config file.
    ///
    /// If the config file doesn't exist, returns a new manager with default channel.
    pub fn load(config_path: PathBuf) -> Result<Self, UpdateError> {
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let channel: UpdateChannel = serde_json::from_str(&content)?;
            tracing::debug!("Loaded update channel from config: {}", channel);
            Ok(Self {
                current_channel: channel,
                config_path,
            })
        } else {
            tracing::debug!("No channel config found, using default (stable)");
            Ok(Self::new(config_path))
        }
    }

    /// Get the current update channel.
    pub fn current_channel(&self) -> &UpdateChannel {
        &self.current_channel
    }

    /// Get the config file path.
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// Get manifest URL for current channel.
    ///
    /// Returns the appropriate manifest URL based on the current channel:
    /// - Stable: `https://updates.zippyremote.io/stable/manifest.json`
    /// - Beta: `https://updates.zippyremote.io/beta/manifest.json`
    /// - Nightly: `https://updates.zippyremote.io/nightly/manifest.json`
    /// - Custom: The custom URL provided
    pub fn manifest_url(&self) -> String {
        Self::manifest_url_for_channel(&self.current_channel)
    }

    /// Get manifest URL for a specific channel.
    pub fn manifest_url_for_channel(channel: &UpdateChannel) -> String {
        match channel {
            UpdateChannel::Stable => {
                "https://updates.zippyremote.io/stable/manifest.json".to_string()
            }
            UpdateChannel::Beta => {
                "https://updates.zippyremote.io/beta/manifest.json".to_string()
            }
            UpdateChannel::Nightly => {
                "https://updates.zippyremote.io/nightly/manifest.json".to_string()
            }
            UpdateChannel::Custom(url) => url.clone(),
        }
    }

    /// Switch update channel.
    ///
    /// Warns if switching to a less stable channel (e.g., Stable -> Beta).
    /// The new channel is persisted to the config file.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be written.
    pub fn set_channel(&mut self, channel: UpdateChannel) -> Result<(), UpdateError> {
        // Warn if switching to less stable channel
        if self.is_downgrade(&channel) {
            tracing::warn!(
                "Switching to less stable update channel: {} -> {}",
                self.current_channel,
                channel
            );
        } else if self.current_channel != channel {
            tracing::info!(
                "Switching update channel: {} -> {}",
                self.current_channel,
                channel
            );
        }

        self.current_channel = channel;
        self.save_config()?;
        Ok(())
    }

    /// Check if switching to new channel is a stability downgrade.
    ///
    /// A downgrade is when the new channel has lower stability than the current.
    pub fn is_downgrade(&self, new_channel: &UpdateChannel) -> bool {
        self.current_channel.is_more_stable_than(new_channel)
    }

    /// Check if switching to new channel is a stability upgrade.
    pub fn is_upgrade(&self, new_channel: &UpdateChannel) -> bool {
        new_channel.is_more_stable_than(&self.current_channel)
    }

    /// Save channel to config file.
    fn save_config(&self) -> Result<(), UpdateError> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.current_channel)?;
        std::fs::write(&self.config_path, content)?;
        tracing::debug!("Saved channel config to {:?}", self.config_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_update_channel_default() {
        assert_eq!(UpdateChannel::default(), UpdateChannel::Stable);
    }

    #[test]
    fn test_update_channel_display() {
        assert_eq!(UpdateChannel::Stable.to_string(), "stable");
        assert_eq!(UpdateChannel::Beta.to_string(), "beta");
        assert_eq!(UpdateChannel::Nightly.to_string(), "nightly");
        assert_eq!(
            UpdateChannel::Custom("https://example.com".to_string()).to_string(),
            "custom(https://example.com)"
        );
    }

    #[test]
    fn test_update_channel_stability_level() {
        assert_eq!(UpdateChannel::Stable.stability_level(), 3);
        assert_eq!(UpdateChannel::Beta.stability_level(), 2);
        assert_eq!(UpdateChannel::Nightly.stability_level(), 1);
        assert_eq!(
            UpdateChannel::Custom("url".to_string()).stability_level(),
            0
        );
    }

    #[test]
    fn test_update_channel_stability_comparison() {
        assert!(UpdateChannel::Stable.is_more_stable_than(&UpdateChannel::Beta));
        assert!(UpdateChannel::Beta.is_more_stable_than(&UpdateChannel::Nightly));
        assert!(UpdateChannel::Nightly.is_more_stable_than(&UpdateChannel::Custom("x".to_string())));
        assert!(!UpdateChannel::Beta.is_more_stable_than(&UpdateChannel::Stable));
    }

    #[test]
    fn test_update_channel_serialization() {
        let stable = UpdateChannel::Stable;
        let json = serde_json::to_string(&stable).unwrap();
        assert_eq!(json, "\"stable\"");

        let beta = UpdateChannel::Beta;
        let json = serde_json::to_string(&beta).unwrap();
        assert_eq!(json, "\"beta\"");

        let nightly = UpdateChannel::Nightly;
        let json = serde_json::to_string(&nightly).unwrap();
        assert_eq!(json, "\"nightly\"");

        let custom = UpdateChannel::Custom("https://example.com".to_string());
        let json = serde_json::to_string(&custom).unwrap();
        assert!(json.contains("custom"));
    }

    #[test]
    fn test_update_channel_deserialization() {
        let stable: UpdateChannel = serde_json::from_str("\"stable\"").unwrap();
        assert_eq!(stable, UpdateChannel::Stable);

        let beta: UpdateChannel = serde_json::from_str("\"beta\"").unwrap();
        assert_eq!(beta, UpdateChannel::Beta);

        let nightly: UpdateChannel = serde_json::from_str("\"nightly\"").unwrap();
        assert_eq!(nightly, UpdateChannel::Nightly);
    }

    #[test]
    fn test_channel_manager_new() {
        let manager = ChannelManager::new(PathBuf::from("test.json"));
        assert_eq!(manager.current_channel(), &UpdateChannel::Stable);
    }

    #[test]
    fn test_channel_manager_with_channel() {
        let manager =
            ChannelManager::with_channel(PathBuf::from("test.json"), UpdateChannel::Beta);
        assert_eq!(manager.current_channel(), &UpdateChannel::Beta);
    }

    #[test]
    fn test_channel_manager_manifest_url() {
        let manager = ChannelManager::new(PathBuf::from("test.json"));
        assert_eq!(
            manager.manifest_url(),
            "https://updates.zippyremote.io/stable/manifest.json"
        );

        let manager =
            ChannelManager::with_channel(PathBuf::from("test.json"), UpdateChannel::Beta);
        assert_eq!(
            manager.manifest_url(),
            "https://updates.zippyremote.io/beta/manifest.json"
        );

        let manager =
            ChannelManager::with_channel(PathBuf::from("test.json"), UpdateChannel::Nightly);
        assert_eq!(
            manager.manifest_url(),
            "https://updates.zippyremote.io/nightly/manifest.json"
        );

        let custom_url = "https://enterprise.example.com/updates/manifest.json".to_string();
        let manager = ChannelManager::with_channel(
            PathBuf::from("test.json"),
            UpdateChannel::Custom(custom_url.clone()),
        );
        assert_eq!(manager.manifest_url(), custom_url);
    }

    #[test]
    fn test_channel_manager_manifest_url_for_channel() {
        assert_eq!(
            ChannelManager::manifest_url_for_channel(&UpdateChannel::Stable),
            "https://updates.zippyremote.io/stable/manifest.json"
        );
        assert_eq!(
            ChannelManager::manifest_url_for_channel(&UpdateChannel::Beta),
            "https://updates.zippyremote.io/beta/manifest.json"
        );
        assert_eq!(
            ChannelManager::manifest_url_for_channel(&UpdateChannel::Nightly),
            "https://updates.zippyremote.io/nightly/manifest.json"
        );
    }

    #[test]
    fn test_channel_manager_is_downgrade() {
        let manager = ChannelManager::new(PathBuf::from("test.json")); // Stable
        assert!(manager.is_downgrade(&UpdateChannel::Beta));
        assert!(manager.is_downgrade(&UpdateChannel::Nightly));
        assert!(manager.is_downgrade(&UpdateChannel::Custom("x".to_string())));
        assert!(!manager.is_downgrade(&UpdateChannel::Stable));

        let manager =
            ChannelManager::with_channel(PathBuf::from("test.json"), UpdateChannel::Beta);
        assert!(!manager.is_downgrade(&UpdateChannel::Stable));
        assert!(!manager.is_downgrade(&UpdateChannel::Beta));
        assert!(manager.is_downgrade(&UpdateChannel::Nightly));
    }

    #[test]
    fn test_channel_manager_is_upgrade() {
        let manager =
            ChannelManager::with_channel(PathBuf::from("test.json"), UpdateChannel::Nightly);
        assert!(manager.is_upgrade(&UpdateChannel::Stable));
        assert!(manager.is_upgrade(&UpdateChannel::Beta));
        assert!(!manager.is_upgrade(&UpdateChannel::Nightly));
    }

    #[test]
    fn test_channel_manager_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("channel.json");

        // Create and set channel
        {
            let mut manager = ChannelManager::new(config_path.clone());
            manager.set_channel(UpdateChannel::Beta).unwrap();
        }

        // Load and verify
        {
            let manager = ChannelManager::load(config_path.clone()).unwrap();
            assert_eq!(manager.current_channel(), &UpdateChannel::Beta);
        }

        // Change to nightly
        {
            let mut manager = ChannelManager::load(config_path.clone()).unwrap();
            manager.set_channel(UpdateChannel::Nightly).unwrap();
        }

        // Verify nightly persisted
        {
            let manager = ChannelManager::load(config_path).unwrap();
            assert_eq!(manager.current_channel(), &UpdateChannel::Nightly);
        }
    }

    #[test]
    fn test_channel_manager_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.json");

        let manager = ChannelManager::load(config_path).unwrap();
        assert_eq!(manager.current_channel(), &UpdateChannel::Stable);
    }

    #[test]
    fn test_channel_manager_custom_channel_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("channel.json");

        let custom_url = "https://enterprise.example.com/updates/manifest.json".to_string();

        // Set custom channel
        {
            let mut manager = ChannelManager::new(config_path.clone());
            manager
                .set_channel(UpdateChannel::Custom(custom_url.clone()))
                .unwrap();
        }

        // Load and verify
        {
            let manager = ChannelManager::load(config_path).unwrap();
            assert_eq!(
                manager.current_channel(),
                &UpdateChannel::Custom(custom_url.clone())
            );
            assert_eq!(manager.manifest_url(), custom_url);
        }
    }
}
