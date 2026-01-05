//! Clipboard synchronization via WebRTC DataChannel.

use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("clipboard read failed: {0}")]
    ReadFailed(String),
    #[error("clipboard write failed: {0}")]
    WriteFailed(String),
    #[error("format not supported: {0}")]
    FormatNotSupported(String),
    #[error("size limit exceeded: {0} bytes")]
    SizeLimitExceeded(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardFormat {
    Text,
    ImagePng,
}

pub struct ClipboardSync {
    max_size: usize,
    last_sequence: u64,
    #[cfg(windows)]
    clipboard: Option<zrc_platform_win::clipboard::WinClipboard>,
}

impl ClipboardSync {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            last_sequence: 0,
            #[cfg(windows)]
            clipboard: None,
        }
    }

    #[cfg(windows)]
    pub fn with_windows_clipboard(mut self) -> Result<Self, ClipboardError> {
        let clipboard = zrc_platform_win::clipboard::WinClipboard::new()
            .map_err(|e| ClipboardError::ReadFailed(e.to_string()))?;
        self.clipboard = Some(clipboard);
        Ok(self)
    }

    pub async fn read_text(&self) -> Result<String, ClipboardError> {
        #[cfg(windows)]
        {
            if let Some(ref clipboard) = self.clipboard {
                clipboard.read_text()
                    .map_err(|e| ClipboardError::ReadFailed(e.to_string()))?
                    .ok_or_else(|| ClipboardError::ReadFailed("Clipboard is empty".to_string()))
            } else {
                Err(ClipboardError::ReadFailed("Clipboard not initialized".to_string()))
            }
        }
        #[cfg(not(windows))]
        {
            Err(ClipboardError::ReadFailed("Clipboard not supported on this platform".to_string()))
        }
    }

    pub async fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        if text.len() > self.max_size {
            return Err(ClipboardError::SizeLimitExceeded(text.len()));
        }

        #[cfg(windows)]
        {
            if let Some(ref clipboard) = self.clipboard {
                clipboard.write_text(text)
                    .map_err(|e| ClipboardError::WriteFailed(e.to_string()))
            } else {
                Err(ClipboardError::WriteFailed("Clipboard not initialized".to_string()))
            }
        }
        #[cfg(not(windows))]
        {
            Err(ClipboardError::WriteFailed("Clipboard not supported on this platform".to_string()))
        }
    }

    pub async fn read_image(&self) -> Result<Vec<u8>, ClipboardError> {
        // TODO: Implement image reading
        warn!("Image clipboard reading not yet implemented");
        Err(ClipboardError::FormatNotSupported("Image".to_string()))
    }

    pub async fn write_image(&self, data: &[u8]) -> Result<(), ClipboardError> {
        if data.len() > self.max_size {
            return Err(ClipboardError::SizeLimitExceeded(data.len()));
        }

        // TODO: Implement image writing
        warn!("Image clipboard writing not yet implemented");
        Err(ClipboardError::FormatNotSupported("Image".to_string()))
    }
}
