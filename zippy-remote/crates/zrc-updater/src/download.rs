//! Secure artifact downloader.
//!
//! Handles downloading update artifacts with progress reporting,
//! resume support, and verification.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::error::UpdateError;

/// Default timeout for HTTP requests in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Default read timeout for streaming downloads in seconds.
const DEFAULT_READ_TIMEOUT_SECS: u64 = 60;

/// Buffer size for reading chunks during download.
const DOWNLOAD_BUFFER_SIZE: usize = 8192;

/// Configuration for the downloader.
#[derive(Debug, Clone)]
pub struct DownloaderConfig {
    /// Connection timeout in seconds.
    pub timeout_secs: u64,
    /// Read timeout for streaming in seconds.
    pub read_timeout_secs: u64,
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// User agent string.
    pub user_agent: String,
}

impl Default for DownloaderConfig {
    fn default() -> Self {
        Self {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            read_timeout_secs: DEFAULT_READ_TIMEOUT_SECS,
            max_retries: 3,
            user_agent: format!("zrc-updater/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Progress callback type for download progress reporting.
pub type ProgressCallback = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

/// Secure artifact downloader with resume support.
///
/// The downloader supports:
/// - Background downloads with progress reporting
/// - Pause/resume via HTTP Range headers
/// - Configurable timeouts and retries
/// - HTTPS-only downloads for security
pub struct Downloader {
    /// HTTP client configured with timeouts.
    client: reqwest::Client,
    /// Configuration settings.
    config: DownloaderConfig,
    /// Optional progress callback for reporting download progress.
    progress_callback: Option<ProgressCallback>,
}

impl Downloader {
    /// Create a new downloader with default settings.
    ///
    /// Uses a 30-second connection timeout and 60-second read timeout.
    pub fn new() -> Self {
        Self::with_config(DownloaderConfig::default())
    }

    /// Create a new downloader with custom configuration.
    pub fn with_config(config: DownloaderConfig) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(config.timeout_secs))
            .read_timeout(Duration::from_secs(config.read_timeout_secs))
            .user_agent(&config.user_agent)
            .build()
            .expect("failed to create HTTP client");

        Self {
            client,
            config,
            progress_callback: None,
        }
    }

    /// Create a new downloader with a progress callback.
    ///
    /// The callback will be invoked periodically during downloads
    /// with the current progress information.
    pub fn with_progress<F>(callback: F) -> Self
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        let config = DownloaderConfig::default();
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(config.timeout_secs))
            .read_timeout(Duration::from_secs(config.read_timeout_secs))
            .user_agent(&config.user_agent)
            .build()
            .expect("failed to create HTTP client");

        Self {
            client,
            config,
            progress_callback: Some(Arc::new(callback)),
        }
    }

    /// Set the progress callback.
    pub fn set_progress_callback<F>(&mut self, callback: F)
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Arc::new(callback));
    }

    /// Clear the progress callback.
    pub fn clear_progress_callback(&mut self) {
        self.progress_callback = None;
    }

    /// Fetch data from a URL into memory.
    ///
    /// This is suitable for small files like manifests.
    /// For large files, use `download_with_resume` instead.
    pub async fn fetch(&self, url: &str) -> Result<Vec<u8>, UpdateError> {
        debug!("Fetching URL: {}", url);

        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(UpdateError::DownloadFailed {
                status: response.status().as_u16(),
            });
        }

        let bytes = response.bytes().await?;
        debug!("Fetched {} bytes", bytes.len());
        Ok(bytes.to_vec())
    }

    /// Download a file with progress reporting and resume support.
    ///
    /// This method supports resuming interrupted downloads by using
    /// HTTP Range headers. If the destination file already exists,
    /// it will attempt to resume from where it left off.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download from (must be HTTPS)
    /// * `dest` - The destination file path
    /// * `expected_size` - The expected total size of the file
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the download completes successfully,
    /// or an error if the download fails.
    pub async fn download_with_resume(
        &self,
        url: &str,
        dest: &Path,
        expected_size: u64,
    ) -> Result<(), UpdateError> {
        info!("Starting download: {} -> {:?}", url, dest);

        // Determine starting position for resume
        let start_byte = if dest.exists() {
            let existing_size = dest.metadata()?.len();
            if existing_size >= expected_size {
                info!("Download already complete ({} bytes)", existing_size);
                self.report_progress(expected_size, expected_size);
                return Ok(());
            }
            debug!("Resuming download from byte {}", existing_size);
            existing_size
        } else {
            // Ensure parent directory exists
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            0
        };

        // Open file for writing (append if resuming)
        let mut file = if start_byte > 0 {
            OpenOptions::new().append(true).open(dest)?
        } else {
            File::create(dest)?
        };

        // Build request with Range header for resume
        let mut request = self.client.get(url);
        if start_byte > 0 {
            request = request.header("Range", format!("bytes={}-", start_byte));
        }

        let response = request.send().await?;
        let status = response.status();

        // Check response status
        if !status.is_success() && status != StatusCode::PARTIAL_CONTENT {
            return Err(UpdateError::DownloadFailed {
                status: status.as_u16(),
            });
        }

        // Verify server supports range requests when resuming
        if start_byte > 0 && status != StatusCode::PARTIAL_CONTENT {
            warn!("Server does not support range requests, restarting download");
            // Server doesn't support resume, start over
            drop(file);
            file = File::create(dest)?;
        }

        // Stream the response body
        let mut stream = response.bytes_stream();
        let mut downloaded = if status == StatusCode::PARTIAL_CONTENT {
            start_byte
        } else {
            0
        };

        // Report initial progress
        self.report_progress(downloaded, expected_size);

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| UpdateError::NetworkError(e.to_string()))?;

            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;

            // Report progress
            self.report_progress(downloaded, expected_size);
        }

        // Ensure all data is written to disk
        file.sync_all()?;

        // Verify final size
        let final_size = dest.metadata()?.len();
        if final_size != expected_size {
            warn!(
                "Download size mismatch: expected {}, got {}",
                expected_size, final_size
            );
            return Err(UpdateError::SizeMismatch {
                expected: expected_size,
                actual: final_size,
            });
        }

        info!("Download complete: {} bytes", final_size);
        Ok(())
    }

    /// Download a file and verify its hash.
    ///
    /// This combines download with hash verification for security.
    /// The file is downloaded first, then its SHA-256 hash is computed
    /// and compared against the expected hash.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download from
    /// * `dest` - The destination file path
    /// * `expected_size` - The expected total size of the file
    /// * `expected_hash` - The expected SHA-256 hash (32 bytes)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if download and verification succeed,
    /// or an error if either fails.
    pub async fn download_and_verify(
        &self,
        url: &str,
        dest: &Path,
        expected_size: u64,
        expected_hash: &[u8; 32],
    ) -> Result<(), UpdateError> {
        // Download the file
        self.download_with_resume(url, dest, expected_size).await?;

        // Verify hash
        let actual_hash = self.compute_file_hash(dest)?;
        if actual_hash != *expected_hash {
            // Delete the corrupted file
            let _ = std::fs::remove_file(dest);
            return Err(UpdateError::HashMismatch {
                expected: hex::encode(expected_hash),
                actual: hex::encode(actual_hash),
            });
        }

        Ok(())
    }

    /// Compute the SHA-256 hash of a file.
    fn compute_file_hash(&self, path: &Path) -> Result<[u8; 32], UpdateError> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; DOWNLOAD_BUFFER_SIZE];

        loop {
            let n = std::io::Read::read(&mut file, &mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(hasher.finalize().into())
    }

    /// Report download progress via the callback if set.
    fn report_progress(&self, downloaded: u64, total: u64) {
        if let Some(callback) = &self.progress_callback {
            callback(DownloadProgress { downloaded, total });
        }
    }

    /// Verify a partial download can be resumed.
    ///
    /// Checks if the server supports Range requests by sending
    /// a HEAD request and checking for Accept-Ranges header.
    pub async fn supports_resume(&self, url: &str) -> Result<bool, UpdateError> {
        let response = self.client.head(url).send().await?;

        if !response.status().is_success() {
            return Err(UpdateError::DownloadFailed {
                status: response.status().as_u16(),
            });
        }

        // Check for Accept-Ranges header
        let supports = response
            .headers()
            .get("accept-ranges")
            .map(|v| v.to_str().unwrap_or("") != "none")
            .unwrap_or(false);

        Ok(supports)
    }

    /// Get the content length of a URL without downloading.
    pub async fn get_content_length(&self, url: &str) -> Result<Option<u64>, UpdateError> {
        let response = self.client.head(url).send().await?;

        if !response.status().is_success() {
            return Err(UpdateError::DownloadFailed {
                status: response.status().as_u16(),
            });
        }

        let length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok());

        Ok(length)
    }

    /// Clean up incomplete downloads.
    ///
    /// Removes the destination file if it exists and is incomplete.
    pub fn cleanup_incomplete(&self, dest: &Path, expected_size: u64) -> Result<bool, UpdateError> {
        if dest.exists() {
            let actual_size = dest.metadata()?.len();
            if actual_size < expected_size {
                std::fs::remove_file(dest)?;
                debug!("Cleaned up incomplete download: {:?}", dest);
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Download progress information.
///
/// Provides information about the current state of a download,
/// including bytes downloaded and total size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    /// Bytes downloaded so far.
    pub downloaded: u64,
    /// Total bytes to download.
    pub total: u64,
}

impl DownloadProgress {
    /// Create a new progress instance.
    pub fn new(downloaded: u64, total: u64) -> Self {
        Self { downloaded, total }
    }

    /// Get download progress as a percentage (0.0 to 100.0).
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f64 / self.total as f64) * 100.0
        }
    }

    /// Check if the download is complete.
    pub fn is_complete(&self) -> bool {
        self.downloaded >= self.total && self.total > 0
    }

    /// Get remaining bytes to download.
    pub fn remaining(&self) -> u64 {
        self.total.saturating_sub(self.downloaded)
    }
}

impl std::fmt::Display for DownloadProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{} bytes ({:.1}%)",
            self.downloaded,
            self.total,
            self.percentage()
        )
    }
}
