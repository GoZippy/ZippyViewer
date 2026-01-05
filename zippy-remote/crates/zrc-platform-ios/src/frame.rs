//! Frame data types for iOS

use uniffi::Record;

/// Frame data structure exposed to Swift
#[derive(Debug, Clone, Record)]
pub struct FrameData {
    /// Frame pixel data (BGRA8 format)
    pub data: Vec<u8>,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Timestamp in milliseconds since epoch
    pub timestamp: u64,
}

impl FrameData {
    /// Create new frame data
    pub fn new(data: Vec<u8>, width: u32, height: u32, timestamp: u64) -> Self {
        Self {
            data,
            width,
            height,
            timestamp,
        }
    }
}
