//! Error types for the update system.

use thiserror::Error;

/// Errors that can occur during update operations.
#[derive(Debug, Error)]
pub enum UpdateError {
    /// Manifest signature verification failed
    #[error("manifest signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    /// Insufficient valid signatures on manifest
    #[error("insufficient signatures: required {required}, found {found}")]
    InsufficientSignatures { required: usize, found: usize },

    /// Manifest timestamp is too old (older than 7 days)
    #[error("manifest timestamp is too old")]
    ManifestTooOld,

    /// Manifest timestamp is in the future
    #[error("manifest timestamp is in the future")]
    ManifestFromFuture,

    /// Platform mismatch between manifest and current system
    #[error("platform mismatch: expected {expected}, got {actual}")]
    PlatformMismatch { expected: String, actual: String },

    /// Artifact hash does not match expected value
    #[error("hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    /// Artifact size does not match expected value
    #[error("size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: u64, actual: u64 },

    /// Download failed with HTTP status
    #[error("download failed with status {status}")]
    DownloadFailed { status: u16 },

    /// Download was interrupted
    #[error("download interrupted")]
    DownloadInterrupted,

    /// Network error during download
    #[error("network error: {0}")]
    NetworkError(String),

    /// Installation failed
    #[error("installation failed: {0}")]
    InstallationFailed(String),

    /// Rollback failed
    #[error("rollback failed: {0}")]
    RollbackFailed(String),

    /// Backup is corrupted or missing
    #[error("backup corrupted or missing")]
    BackupCorrupted,

    /// No backup available for rollback
    #[error("no backup available")]
    NoBackupAvailable,

    /// Code signature verification failed
    #[error("code signature verification failed: {0}")]
    CodeSignatureInvalid(String),

    /// Service management error
    #[error("service error: {0}")]
    ServiceError(String),

    /// Configuration error
    #[error("configuration error: {0}")]
    ConfigError(String),

    /// Channel not found or invalid
    #[error("invalid channel: {0}")]
    InvalidChannel(String),

    /// Version parsing error
    #[error("version parse error: {0}")]
    VersionParseError(String),

    /// JSON parsing error
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// HTTP request error
    #[error("HTTP error: {0}")]
    HttpError(String),
}

impl From<reqwest::Error> for UpdateError {
    fn from(err: reqwest::Error) -> Self {
        UpdateError::HttpError(err.to_string())
    }
}

impl From<ed25519_dalek::SignatureError> for UpdateError {
    fn from(err: ed25519_dalek::SignatureError) -> Self {
        UpdateError::SignatureVerificationFailed(err.to_string())
    }
}

impl From<semver::Error> for UpdateError {
    fn from(err: semver::Error) -> Self {
        UpdateError::VersionParseError(err.to_string())
    }
}
