//! Frame data structures for Android

use prost::Message;
use zrc_proto::v1::FrameMetadataV1;

/// Frame data received from remote device
#[derive(Debug, Clone)]
pub struct FrameData {
    /// Frame data bytes (encoded frame)
    pub data: Vec<u8>,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Frame format identifier
    pub format: u32,
    /// Frame timestamp
    pub timestamp: u64,
}

impl FrameData {
    /// Create new frame data
    pub fn new(data: Vec<u8>, width: u32, height: u32, format: u32, timestamp: u64) -> Self {
        Self {
            data,
            width,
            height,
            format,
            timestamp,
        }
    }
    
    /// Encode frame with metadata for JNI transfer
    /// Format: [metadata_len: u32][metadata_bytes][frame_data]
    pub fn encode_for_jni(&self) -> Vec<u8> {
        let metadata = FrameMetadataV1 {
            frame_id: 0, // Will be set by frame source
            timestamp: self.timestamp,
            monitor_id: 0,
            width: self.width,
            height: self.height,
            format: self.format as i32,
            flags: 0,
            dirty_x: 0,
            dirty_y: 0,
            dirty_width: self.width,
            dirty_height: self.height,
            cursor_x: 0,
            cursor_y: 0,
            cursor_shape_id: 0,
        };
        
        let mut metadata_bytes = Vec::new();
        metadata.encode(&mut metadata_bytes).unwrap_or_default();
        
        let mut result = Vec::with_capacity(4 + metadata_bytes.len() + self.data.len());
        result.extend_from_slice(&(metadata_bytes.len() as u32).to_le_bytes());
        result.extend_from_slice(&metadata_bytes);
        result.extend_from_slice(&self.data);
        
        result
    }
    
    /// Decode frame from JNI format
    pub fn decode_from_jni(data: &[u8]) -> Result<Self, String> {
        if data.len() < 4 {
            return Err("Invalid frame data: too short".to_string());
        }
        
        let metadata_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + metadata_len {
            return Err("Invalid frame data: metadata length mismatch".to_string());
        }
        
        let metadata_bytes = &data[4..4 + metadata_len];
        let frame_data = &data[4 + metadata_len..];
        
        let metadata = FrameMetadataV1::decode(metadata_bytes)
            .map_err(|e| format!("Failed to decode metadata: {}", e))?;
        
        Ok(Self {
            data: frame_data.to_vec(),
            width: metadata.width,
            height: metadata.height,
            format: metadata.format as u32,
            timestamp: metadata.timestamp,
        })
    }
}
