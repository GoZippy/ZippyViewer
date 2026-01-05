//! # zrc-updater
//!
//! Secure automatic update system for ZRC (Zippy Remote Control).
//!
//! This crate handles:
//! - Update manifest verification with Ed25519 signatures
//! - Artifact download with resume support
//! - SHA-256 hash verification
//! - Platform-specific installation (Windows, macOS, Linux)
//! - Rollback support for failed updates
//!
//! ## Security
//!
//! Security is paramount in this crate:
//! - Manifests are verified against pinned public keys before processing
//! - Artifacts are hash-verified before any code execution
//! - Platform code signatures are verified where available
//! - Rollback is always available in case of failure

pub mod artifact;
pub mod channel;
pub mod config;
pub mod download;
pub mod error;
pub mod install;
pub mod manager;
pub mod manifest;
pub mod notification;
pub mod offline;
#[cfg(test)]
mod proptests;
pub mod rollback;

// Re-export main types for convenience
pub use artifact::ArtifactVerifier;
pub use channel::{ChannelManager, UpdateChannel};
pub use config::{RollbackConfig, SecurityConfig, UpdateConfig};
pub use download::{DownloadProgress, Downloader, DownloaderConfig};
pub use error::UpdateError;
pub use install::PlatformInstaller;
#[cfg(target_os = "windows")]
pub use install::{WindowsInstaller, verify_authenticode};
#[cfg(target_os = "macos")]
pub use install::{MacOSInstaller, verify_macos_code_signature};
#[cfg(target_os = "linux")]
pub use install::LinuxInstaller;
pub use manager::{UpdateInfo, UpdateManager, UpdateState};
pub use manifest::{current_platform, ManifestVerifier, SignedManifest, UpdateManifest, ManifestSignature};
pub use notification::{
    create_platform_backend, DeferredUpdate, NotificationBackend, NotificationConfig,
    NotificationContent, NotificationManager, NotificationResponse, NotificationState,
    StubNotificationBackend, UpdateUrgency,
};
pub use offline::{
    generate_package_filename, package_extension, OfflineUpdateInfo, OfflineUpdateManager,
    OfflineUpdatePackage,
};
pub use rollback::{BackupInfo, RollbackManager};
