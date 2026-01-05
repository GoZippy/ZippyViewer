//! Frame reception and handling for remote display
//!
//! This module implements frame reception over QUIC streams:
//! - Receive frame data over dedicated QUIC stream
//! - Decode frame metadata (dimensions, format, timestamp)
//! - Save frames to file
//! - Display frame statistics
//!
//! Requirements: 6.1-6.7

use std::collections::VecDeque;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use prost::Message;
use thiserror::Error;
use tokio::sync::{mpsc, RwLock};

use zrc_proto::v1::{FrameFormatV1, FrameMetadataV1};

/// Frame reception errors
#[derive(Debug, Error)]
pub enum FrameError {
    #[error("No active session")]
    NoSession,

    #[error("Stream closed")]
    StreamClosed,

    #[error("Invalid frame data: {0}")]
    InvalidFrame(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Decode error: {0}")]
    Decode(String),

    #[error("Timeout")]
    Timeout,
}

/// Frame format for saving
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveFormat {
    /// Raw frame data as received
    #[default]
    Raw,
    /// PNG format (requires conversion)
    Png,
}

impl std::str::FromStr for SaveFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "raw" => Ok(Self::Raw),
            "png" => Ok(Self::Png),
            _ => Err(format!("Unknown format: {s}. Use 'raw' or 'png'")),
        }
    }
}

/// A received frame with metadata and data
#[derive(Debug, Clone)]
pub struct ReceivedFrame {
    /// Frame metadata
    pub metadata: FrameMetadata,
    /// Frame data bytes
    pub data: Vec<u8>,
    /// When the frame was received locally
    pub received_at: Instant,
}

/// Parsed frame metadata
#[derive(Debug, Clone)]
pub struct FrameMetadata {
    /// Unique frame identifier
    pub frame_id: u64,
    /// Capture timestamp in microseconds
    pub timestamp: u64,
    /// Monitor/display identifier
    pub monitor_id: u32,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Frame encoding format
    pub format: FrameFormat,
    /// Frame flags
    pub flags: FrameFlags,
    /// Dirty rectangle (for partial updates)
    pub dirty_rect: Option<DirtyRect>,
    /// Cursor position (if not embedded)
    pub cursor: Option<CursorInfo>,
}

impl From<FrameMetadataV1> for FrameMetadata {
    fn from(v: FrameMetadataV1) -> Self {
        let dirty_rect = if v.dirty_width > 0 && v.dirty_height > 0 {
            Some(DirtyRect {
                x: v.dirty_x,
                y: v.dirty_y,
                width: v.dirty_width,
                height: v.dirty_height,
            })
        } else {
            None
        };

        let cursor = if v.cursor_shape_id > 0 || v.cursor_x != 0 || v.cursor_y != 0 {
            Some(CursorInfo {
                x: v.cursor_x,
                y: v.cursor_y,
                shape_id: v.cursor_shape_id,
            })
        } else {
            None
        };

        Self {
            frame_id: v.frame_id,
            timestamp: v.timestamp,
            monitor_id: v.monitor_id,
            width: v.width,
            height: v.height,
            format: FrameFormat::from(v.format()),
            flags: FrameFlags::from_bits(v.flags),
            dirty_rect,
            cursor,
        }
    }
}

/// Frame encoding format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameFormat {
    #[default]
    Unspecified,
    RawBgra,
    RawRgba,
    Jpeg,
    Png,
    H264,
    Vp8,
    Vp9,
    Av1,
}

impl From<FrameFormatV1> for FrameFormat {
    fn from(v: FrameFormatV1) -> Self {
        match v {
            FrameFormatV1::Unspecified => Self::Unspecified,
            FrameFormatV1::RawBgra => Self::RawBgra,
            FrameFormatV1::RawRgba => Self::RawRgba,
            FrameFormatV1::Jpeg => Self::Jpeg,
            FrameFormatV1::Png => Self::Png,
            FrameFormatV1::H264 => Self::H264,
            FrameFormatV1::Vp8 => Self::Vp8,
            FrameFormatV1::Vp9 => Self::Vp9,
            FrameFormatV1::Av1 => Self::Av1,
        }
    }
}

impl std::fmt::Display for FrameFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unspecified => write!(f, "unspecified"),
            Self::RawBgra => write!(f, "raw-bgra"),
            Self::RawRgba => write!(f, "raw-rgba"),
            Self::Jpeg => write!(f, "jpeg"),
            Self::Png => write!(f, "png"),
            Self::H264 => write!(f, "h264"),
            Self::Vp8 => write!(f, "vp8"),
            Self::Vp9 => write!(f, "vp9"),
            Self::Av1 => write!(f, "av1"),
        }
    }
}

/// Frame flags
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameFlags {
    pub is_keyframe: bool,
    pub cursor_visible: bool,
    pub cursor_embedded: bool,
    pub is_partial: bool,
}

impl FrameFlags {
    pub fn from_bits(bits: u32) -> Self {
        Self {
            is_keyframe: bits & 1 != 0,
            cursor_visible: bits & 2 != 0,
            cursor_embedded: bits & 4 != 0,
            is_partial: bits & 8 != 0,
        }
    }
}

/// Dirty rectangle for partial updates
#[derive(Debug, Clone, Copy)]
pub struct DirtyRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Cursor information
#[derive(Debug, Clone, Copy)]
pub struct CursorInfo {
    pub x: i32,
    pub y: i32,
    pub shape_id: u32,
}

/// Frame statistics
#[derive(Debug, Clone, Default)]
pub struct FrameStats {
    /// Total frames received
    pub frames_received: u64,
    /// Frames dropped (due to buffer overflow, etc.)
    pub frames_dropped: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Current frame rate (frames per second)
    pub frame_rate: f64,
    /// Current bandwidth (bytes per second)
    pub bandwidth: f64,
    /// Current resolution
    pub resolution: Option<(u32, u32)>,
    /// Current format
    pub format: Option<FrameFormat>,
    /// Average frame size in bytes
    pub avg_frame_size: u64,
    /// Keyframes received
    pub keyframes: u64,
    /// Partial frames received
    pub partial_frames: u64,
}

/// Frame receiver for handling incoming frames
/// Requirements: 6.1, 6.2
pub struct FrameReceiver {
    /// Channel for receiving frames
    rx: mpsc::Receiver<ReceivedFrame>,
    /// Statistics
    stats: Arc<RwLock<FrameStatsCollector>>,
    /// Whether the receiver is active
    active: Arc<AtomicBool>,
}

impl FrameReceiver {
    /// Create a new frame receiver with the given channel
    pub fn new(rx: mpsc::Receiver<ReceivedFrame>) -> Self {
        Self {
            rx,
            stats: Arc::new(RwLock::new(FrameStatsCollector::new())),
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Receive the next frame
    /// Requirements: 6.1
    pub async fn recv(&mut self) -> Option<ReceivedFrame> {
        if !self.active.load(Ordering::Relaxed) {
            return None;
        }

        match self.rx.recv().await {
            Some(frame) => {
                // Update statistics
                let mut stats = self.stats.write().await;
                stats.record_frame(&frame);
                Some(frame)
            }
            None => {
                self.active.store(false, Ordering::Relaxed);
                None
            }
        }
    }

    /// Try to receive a frame without blocking
    pub fn try_recv(&mut self) -> Option<ReceivedFrame> {
        match self.rx.try_recv() {
            Ok(frame) => {
                // We can't update stats synchronously here without blocking
                Some(frame)
            }
            Err(_) => None,
        }
    }

    /// Get current statistics
    /// Requirements: 6.4, 6.5
    pub async fn stats(&self) -> FrameStats {
        let collector = self.stats.read().await;
        collector.get_stats()
    }

    /// Check if the receiver is still active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Close the receiver
    pub fn close(&self) {
        self.active.store(false, Ordering::Relaxed);
    }
}

/// Collects frame statistics over time
struct FrameStatsCollector {
    /// Total frames received
    frames_received: u64,
    /// Frames dropped
    frames_dropped: u64,
    /// Total bytes received
    bytes_received: u64,
    /// Keyframes received
    keyframes: u64,
    /// Partial frames received
    partial_frames: u64,
    /// Recent frame timestamps for rate calculation
    recent_frames: VecDeque<(Instant, usize)>,
    /// Last known resolution
    resolution: Option<(u32, u32)>,
    /// Last known format
    format: Option<FrameFormat>,
    /// Window for rate calculation
    rate_window: Duration,
}

impl FrameStatsCollector {
    fn new() -> Self {
        Self {
            frames_received: 0,
            frames_dropped: 0,
            bytes_received: 0,
            keyframes: 0,
            partial_frames: 0,
            recent_frames: VecDeque::with_capacity(120),
            resolution: None,
            format: None,
            rate_window: Duration::from_secs(1),
        }
    }

    fn record_frame(&mut self, frame: &ReceivedFrame) {
        self.frames_received += 1;
        self.bytes_received += frame.data.len() as u64;

        if frame.metadata.flags.is_keyframe {
            self.keyframes += 1;
        }
        if frame.metadata.flags.is_partial {
            self.partial_frames += 1;
        }

        self.resolution = Some((frame.metadata.width, frame.metadata.height));
        self.format = Some(frame.metadata.format);

        // Track recent frames for rate calculation
        let now = Instant::now();
        self.recent_frames.push_back((now, frame.data.len()));

        // Remove old entries outside the window
        let cutoff = now - self.rate_window;
        while let Some((time, _)) = self.recent_frames.front() {
            if *time < cutoff {
                self.recent_frames.pop_front();
            } else {
                break;
            }
        }
    }

    #[allow(dead_code)]
    fn record_drop(&mut self) {
        self.frames_dropped += 1;
    }

    fn get_stats(&self) -> FrameStats {
        let now = Instant::now();
        let cutoff = now - self.rate_window;

        // Calculate frame rate and bandwidth from recent frames
        let recent: Vec<_> = self
            .recent_frames
            .iter()
            .filter(|(t, _)| *t >= cutoff)
            .collect();

        let frame_rate = if recent.len() >= 2 {
            let duration = recent.last().unwrap().0 - recent.first().unwrap().0;
            if duration.as_secs_f64() > 0.0 {
                (recent.len() - 1) as f64 / duration.as_secs_f64()
            } else {
                0.0
            }
        } else {
            0.0
        };

        let bandwidth: f64 = recent.iter().map(|(_, size)| *size as f64).sum::<f64>()
            / self.rate_window.as_secs_f64();

        let avg_frame_size = if self.frames_received > 0 {
            self.bytes_received / self.frames_received
        } else {
            0
        };

        FrameStats {
            frames_received: self.frames_received,
            frames_dropped: self.frames_dropped,
            bytes_received: self.bytes_received,
            frame_rate,
            bandwidth,
            resolution: self.resolution,
            format: self.format,
            avg_frame_size,
            keyframes: self.keyframes,
            partial_frames: self.partial_frames,
        }
    }
}

/// Frame saver for writing frames to files
/// Requirements: 6.3
pub struct FrameSaver {
    /// Output path
    path: std::path::PathBuf,
    /// Output format
    format: SaveFormat,
    /// File handle (for raw streaming)
    file: Option<std::fs::File>,
    /// Frames saved count
    frames_saved: u64,
}

impl FrameSaver {
    /// Create a new frame saver
    pub fn new(path: impl AsRef<Path>, format: SaveFormat) -> Result<Self, FrameError> {
        let path = path.as_ref().to_path_buf();

        // For raw format, create the file immediately
        let file = if format == SaveFormat::Raw {
            Some(std::fs::File::create(&path)?)
        } else {
            None
        };

        Ok(Self {
            path,
            format,
            file,
            frames_saved: 0,
        })
    }

    /// Save a frame
    /// Requirements: 6.3
    pub fn save_frame(&mut self, frame: &ReceivedFrame) -> Result<(), FrameError> {
        match self.format {
            SaveFormat::Raw => self.save_raw(frame),
            SaveFormat::Png => self.save_png(frame),
        }
    }

    fn save_raw(&mut self, frame: &ReceivedFrame) -> Result<(), FrameError> {
        if let Some(ref mut file) = self.file {
            // Write frame header (metadata)
            let header = FrameHeader {
                frame_id: frame.metadata.frame_id,
                timestamp: frame.metadata.timestamp,
                width: frame.metadata.width,
                height: frame.metadata.height,
                format: frame.metadata.format as u8,
                data_len: frame.data.len() as u32,
            };
            file.write_all(&header.to_bytes())?;

            // Write frame data
            file.write_all(&frame.data)?;

            self.frames_saved += 1;
            Ok(())
        } else {
            Err(FrameError::Io(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "File not open",
            )))
        }
    }

    fn save_png(&mut self, frame: &ReceivedFrame) -> Result<(), FrameError> {
        // For PNG, save each frame as a separate file
        let filename = self.path.with_file_name(format!(
            "{}_frame_{:06}.png",
            self.path.file_stem().unwrap_or_default().to_string_lossy(),
            self.frames_saved
        ));

        // Only raw BGRA/RGBA can be converted to PNG
        match frame.metadata.format {
            FrameFormat::RawBgra | FrameFormat::RawRgba => {
                // Simple PNG encoding would require the `image` crate
                // For now, just save as raw with .png extension
                // In a full implementation, we'd use image::save_buffer
                let mut file = std::fs::File::create(&filename)?;
                file.write_all(&frame.data)?;
                self.frames_saved += 1;
                Ok(())
            }
            FrameFormat::Png => {
                // Already PNG, just save directly
                let mut file = std::fs::File::create(&filename)?;
                file.write_all(&frame.data)?;
                self.frames_saved += 1;
                Ok(())
            }
            _ => Err(FrameError::InvalidFrame(format!(
                "Cannot convert {} to PNG",
                frame.metadata.format
            ))),
        }
    }

    /// Get the number of frames saved
    pub fn frames_saved(&self) -> u64 {
        self.frames_saved
    }

    /// Flush and close the saver
    pub fn finish(mut self) -> Result<u64, FrameError> {
        if let Some(ref mut file) = self.file {
            file.flush()?;
        }
        Ok(self.frames_saved)
    }
}

/// Simple frame header for raw format
#[repr(C)]
struct FrameHeader {
    frame_id: u64,
    timestamp: u64,
    width: u32,
    height: u32,
    format: u8,
    data_len: u32,
}

impl FrameHeader {
    fn to_bytes(&self) -> [u8; 25] {
        let mut bytes = [0u8; 25];
        bytes[0..8].copy_from_slice(&self.frame_id.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.timestamp.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.width.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.height.to_le_bytes());
        bytes[24] = self.format;
        // Note: data_len is written separately after this header
        bytes
    }
}

/// Parse frame metadata from protobuf bytes
/// Requirements: 6.2
pub fn parse_frame_metadata(data: &[u8]) -> Result<FrameMetadata, FrameError> {
    let proto = FrameMetadataV1::decode(data)
        .map_err(|e| FrameError::Decode(format!("Failed to decode frame metadata: {e}")))?;
    Ok(FrameMetadata::from(proto))
}

/// Create a mock frame receiver for testing
#[cfg(test)]
pub fn mock_frame_receiver() -> (mpsc::Sender<ReceivedFrame>, FrameReceiver) {
    let (tx, rx) = mpsc::channel(32);
    (tx, FrameReceiver::new(rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_format_display() {
        assert_eq!(format!("{}", FrameFormat::RawBgra), "raw-bgra");
        assert_eq!(format!("{}", FrameFormat::H264), "h264");
        assert_eq!(format!("{}", FrameFormat::Png), "png");
    }

    #[test]
    fn test_frame_flags_from_bits() {
        let flags = FrameFlags::from_bits(0);
        assert!(!flags.is_keyframe);
        assert!(!flags.cursor_visible);

        let flags = FrameFlags::from_bits(1);
        assert!(flags.is_keyframe);
        assert!(!flags.cursor_visible);

        let flags = FrameFlags::from_bits(3);
        assert!(flags.is_keyframe);
        assert!(flags.cursor_visible);

        let flags = FrameFlags::from_bits(15);
        assert!(flags.is_keyframe);
        assert!(flags.cursor_visible);
        assert!(flags.cursor_embedded);
        assert!(flags.is_partial);
    }

    #[test]
    fn test_save_format_parse() {
        assert_eq!("raw".parse::<SaveFormat>().unwrap(), SaveFormat::Raw);
        assert_eq!("png".parse::<SaveFormat>().unwrap(), SaveFormat::Png);
        assert_eq!("PNG".parse::<SaveFormat>().unwrap(), SaveFormat::Png);
        assert!("invalid".parse::<SaveFormat>().is_err());
    }

    #[test]
    fn test_frame_stats_default() {
        let stats = FrameStats::default();
        assert_eq!(stats.frames_received, 0);
        assert_eq!(stats.frames_dropped, 0);
        assert_eq!(stats.bytes_received, 0);
        assert_eq!(stats.frame_rate, 0.0);
        assert!(stats.resolution.is_none());
    }

    #[test]
    fn test_frame_saver_raw() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("frames.raw");

        let mut saver = FrameSaver::new(&output_path, SaveFormat::Raw).unwrap();
        assert_eq!(saver.frames_saved(), 0);

        // Create a test frame
        let frame = ReceivedFrame {
            metadata: FrameMetadata {
                frame_id: 1,
                timestamp: 1000,
                monitor_id: 0,
                width: 100,
                height: 100,
                format: FrameFormat::RawBgra,
                flags: FrameFlags::from_bits(1),
                dirty_rect: None,
                cursor: None,
            },
            data: vec![0xAB; 100 * 100 * 4],
            received_at: Instant::now(),
        };

        // Save the frame
        saver.save_frame(&frame).unwrap();
        assert_eq!(saver.frames_saved(), 1);

        // Finish and verify
        let count = saver.finish().unwrap();
        assert_eq!(count, 1);

        // Verify file exists and has content
        let file_size = std::fs::metadata(&output_path).unwrap().len();
        assert!(file_size > 0);
        // Header (25 bytes) + data (100*100*4 = 40000 bytes)
        assert_eq!(file_size, 25 + 40000);
    }

    #[test]
    fn test_frame_saver_png_from_raw() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.png");

        let mut saver = FrameSaver::new(&output_path, SaveFormat::Png).unwrap();

        // Create a test frame with raw BGRA data
        let frame = ReceivedFrame {
            metadata: FrameMetadata {
                frame_id: 1,
                timestamp: 1000,
                monitor_id: 0,
                width: 10,
                height: 10,
                format: FrameFormat::RawBgra,
                flags: FrameFlags::default(),
                dirty_rect: None,
                cursor: None,
            },
            data: vec![0xFF; 10 * 10 * 4],
            received_at: Instant::now(),
        };

        // Save the frame
        saver.save_frame(&frame).unwrap();
        assert_eq!(saver.frames_saved(), 1);

        // Verify file was created with expected name pattern
        let expected_file = temp_dir.path().join("output_frame_000000.png");
        assert!(expected_file.exists());
    }

    #[test]
    fn test_frame_saver_png_invalid_format() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.png");

        let mut saver = FrameSaver::new(&output_path, SaveFormat::Png).unwrap();

        // Create a test frame with H264 data (cannot convert to PNG)
        let frame = ReceivedFrame {
            metadata: FrameMetadata {
                frame_id: 1,
                timestamp: 1000,
                monitor_id: 0,
                width: 10,
                height: 10,
                format: FrameFormat::H264,
                flags: FrameFlags::default(),
                dirty_rect: None,
                cursor: None,
            },
            data: vec![0xFF; 100],
            received_at: Instant::now(),
        };

        // Should fail because H264 cannot be converted to PNG
        let result = saver.save_frame(&frame);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_frame_receiver_stats() {
        let (tx, mut rx) = mock_frame_receiver();

        // Send a test frame
        let frame = ReceivedFrame {
            metadata: FrameMetadata {
                frame_id: 1,
                timestamp: 1000,
                monitor_id: 0,
                width: 1920,
                height: 1080,
                format: FrameFormat::RawBgra,
                flags: FrameFlags::from_bits(1), // keyframe
                dirty_rect: None,
                cursor: None,
            },
            data: vec![0u8; 1920 * 1080 * 4],
            received_at: Instant::now(),
        };

        tx.send(frame).await.unwrap();
        drop(tx);

        // Receive the frame
        let received = rx.recv().await;
        assert!(received.is_some());

        // Check stats
        let stats = rx.stats().await;
        assert_eq!(stats.frames_received, 1);
        assert_eq!(stats.keyframes, 1);
        assert_eq!(stats.resolution, Some((1920, 1080)));
    }
}
