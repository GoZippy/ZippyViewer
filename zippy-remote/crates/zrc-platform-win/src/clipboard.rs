#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use bytes::Bytes;
use thiserror::Error;
use windows::Win32::{
    Foundation::*,
    System::DataExchange::*,
    System::Memory::*,
};

// Clipboard format constants
const CF_UNICODETEXT_VAL: u32 = 13;
const CF_DIB_VAL: u32 = 8;
const CF_DIBV5_VAL: u32 = 17;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("clipboard open failed")]
    OpenFailed,
    #[error("clipboard format not available")]
    FormatNotAvailable,
    #[error("clipboard read failed")]
    ReadFailed,
    #[error("clipboard write failed")]
    WriteFailed,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Windows clipboard access
pub struct WinClipboard {
    hwnd: Option<HWND>, // For clipboard viewer chain
}

impl WinClipboard {
    /// Create clipboard handler
    pub fn new() -> Result<Self, ClipboardError> {
        Ok(Self { hwnd: None })
    }

    /// Read text from clipboard
    pub fn read_text(&self) -> Result<Option<String>, ClipboardError> {
        unsafe {
            if OpenClipboard(self.hwnd).is_err() {
                return Err(ClipboardError::OpenFailed);
            }

            let handle = GetClipboardData(CF_UNICODETEXT_VAL);
            if handle.is_err() {
                let _ = CloseClipboard();
                return Ok(None);
            }
            let handle = handle.unwrap();

            let hglobal = HGLOBAL(handle.0);
            let ptr = GlobalLock(hglobal) as *const u16;
            if ptr.is_null() {
                let _ = CloseClipboard();
                return Err(ClipboardError::ReadFailed);
            }

            // Find null terminator
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }

            let slice = std::slice::from_raw_parts(ptr, len);
            let text = String::from_utf16_lossy(slice);

            let _ = GlobalUnlock(hglobal);
            let _ = CloseClipboard();

            Ok(Some(text))
        }
    }

    /// Write text to clipboard
    pub fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        unsafe {
            if OpenClipboard(self.hwnd).is_err() {
                return Err(ClipboardError::OpenFailed);
            }

            if EmptyClipboard().is_err() {
                let _ = CloseClipboard();
                return Err(ClipboardError::WriteFailed);
            }

            let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let size = utf16.len() * 2;

            let handle = GlobalAlloc(GMEM_MOVEABLE, size);
            if handle.is_err() {
                let _ = CloseClipboard();
                return Err(ClipboardError::WriteFailed);
            }
            let handle = handle.unwrap();

            let ptr = GlobalLock(handle) as *mut u16;
            if ptr.is_null() {
                let _ = GlobalFree(Some(handle));
                let _ = CloseClipboard();
                return Err(ClipboardError::WriteFailed);
            }

            std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());

            let _ = GlobalUnlock(handle);

            let result = SetClipboardData(CF_UNICODETEXT_VAL, Some(HANDLE(handle.0)));
            if result.is_err() {
                let _ = GlobalFree(Some(handle));
                let _ = CloseClipboard();
                return Err(ClipboardError::WriteFailed);
            }

            let _ = CloseClipboard();
            Ok(())
        }
    }

    /// Read image from clipboard
    pub fn read_image(&self) -> Result<Option<Bytes>, ClipboardError> {
        unsafe {
            if OpenClipboard(self.hwnd).is_err() {
                return Err(ClipboardError::OpenFailed);
            }

            // Try CF_DIBV5 first, then CF_DIB
            let handle = GetClipboardData(CF_DIBV5_VAL);
            let format = if handle.is_err() {
                match GetClipboardData(CF_DIB_VAL) {
                    Ok(h) => h,
                    Err(_) => {
                        let _ = CloseClipboard();
                        return Ok(None);
                    }
                }
            } else {
                handle.unwrap()
            };

            let hglobal = HGLOBAL(format.0);
            let ptr = GlobalLock(hglobal) as *const u8;
            if ptr.is_null() {
                let _ = CloseClipboard();
                return Err(ClipboardError::ReadFailed);
            }

            let size = GlobalSize(hglobal);
            let data = std::slice::from_raw_parts(ptr, size).to_vec();

            let _ = GlobalUnlock(hglobal);
            let _ = CloseClipboard();

            Ok(Some(Bytes::from(data)))
        }
    }

    /// Write image to clipboard
    pub fn write_image(&self, image_data: &[u8]) -> Result<(), ClipboardError> {
        unsafe {
            if OpenClipboard(self.hwnd).is_err() {
                return Err(ClipboardError::OpenFailed);
            }

            if EmptyClipboard().is_err() {
                let _ = CloseClipboard();
                return Err(ClipboardError::WriteFailed);
            }

            let handle = GlobalAlloc(GMEM_MOVEABLE, image_data.len());
            if handle.is_err() {
                let _ = CloseClipboard();
                return Err(ClipboardError::WriteFailed);
            }
            let handle = handle.unwrap();

            let ptr = GlobalLock(handle) as *mut u8;
            if ptr.is_null() {
                let _ = GlobalFree(Some(handle));
                let _ = CloseClipboard();
                return Err(ClipboardError::WriteFailed);
            }

            std::ptr::copy_nonoverlapping(image_data.as_ptr(), ptr, image_data.len());

            let _ = GlobalUnlock(handle);

            let result = SetClipboardData(CF_DIB_VAL, Some(HANDLE(handle.0)));
            if result.is_err() {
                let _ = GlobalFree(Some(handle));
                let _ = CloseClipboard();
                return Err(ClipboardError::WriteFailed);
            }

            let _ = CloseClipboard();
            Ok(())
        }
    }

    /// Get clipboard sequence number for change detection
    pub fn sequence_number(&self) -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }
}
