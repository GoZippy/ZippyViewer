#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

use bytes::Bytes;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Failed to access clipboard: {0}")]
    AccessFailed(String),
    #[error("Format not available")]
    FormatNotAvailable,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// macOS clipboard access
pub struct MacClipboard {
    change_count: u64,
}

impl MacClipboard {
    /// Create clipboard handler
    pub fn new() -> Result<Self, ClipboardError> {
        // TODO: Get NSPasteboard general pasteboard
        // TODO: Get initial change count
        Ok(Self {
            change_count: 0,
        })
    }

    /// Read text from clipboard
    pub fn read_text(&self) -> Result<Option<String>, ClipboardError> {
        // TODO: Implement using NSPasteboardTypeString
        Ok(None)
    }

    /// Write text to clipboard
    pub fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        // TODO: Implement using NSPasteboardTypeString
        Ok(())
    }

    /// Read image from clipboard
    pub fn read_image(&self) -> Result<Option<Bytes>, ClipboardError> {
        // TODO: Implement using NSPasteboardTypePNG or TIFF
        Ok(None)
    }

    /// Write image to clipboard
    pub fn write_image(&self, data: &[u8]) -> Result<(), ClipboardError> {
        // TODO: Implement using NSPasteboardTypePNG
        Ok(())
    }

    /// Check if clipboard changed
    pub fn has_changed(&mut self) -> Result<bool, ClipboardError> {
        // TODO: Compare changeCount
        Ok(false)
    }
}
