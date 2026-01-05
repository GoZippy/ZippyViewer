//! File transfer via WebRTC DataChannel.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum FileTransferError {
    #[error("file not found: {0}")]
    FileNotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("transfer failed: {0}")]
    TransferFailed(String),
    #[error("integrity check failed")]
    IntegrityCheckFailed,
}

pub struct FileTransfer {
    download_dir: PathBuf,
    max_file_size: u64,
}

impl FileTransfer {
    pub fn new(download_dir: PathBuf, max_file_size: u64) -> Self {
        Self {
            download_dir,
            max_file_size,
        }
    }

    pub async fn handle_download(&self, file_path: PathBuf) -> Result<Vec<u8>, FileTransferError> {
        // TODO: Implement file download
        warn!("File download not yet implemented");
        Err(FileTransferError::TransferFailed("Not implemented".to_string()))
    }

    pub async fn handle_upload(&self, file_name: String, data: Vec<u8>) -> Result<(), FileTransferError> {
        if data.len() as u64 > self.max_file_size {
            return Err(FileTransferError::TransferFailed(
                format!("File size {} exceeds limit {}", data.len(), self.max_file_size)
            ));
        }

        // TODO: Implement file upload with integrity check
        warn!("File upload not yet implemented");
        Err(FileTransferError::TransferFailed("Not implemented".to_string()))
    }

    pub async fn resume_transfer(&self, transfer_id: &str) -> Result<(), FileTransferError> {
        // TODO: Implement transfer resume
        warn!("Transfer resume not yet implemented");
        Err(FileTransferError::TransferFailed("Not implemented".to_string()))
    }
}
