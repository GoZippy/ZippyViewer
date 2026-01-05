use async_trait::async_trait;
use std::sync::Arc;
use bytes::Bytes;
use thiserror::Error;
use tracing::{debug, info, warn};

#[cfg(windows)]
use zrc_platform_win::capturer::WinCapturer;
#[cfg(windows)]
use zrc_platform_win::monitor::MonitorManager;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("capture failed: {0}")]
    CaptureFailed(String),
    #[error("monitor not found")]
    MonitorNotFound,
    #[error("format not supported")]
    FormatNotSupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureFormat {
    Bgra8888,
    Rgba8888,
    Nv12,
}

/// Platform capturer trait - does NOT require Send + Sync since Windows capture
/// backends contain raw pointers that are not thread-safe.
#[async_trait(?Send)]
pub trait PlatformCapturer {
    async fn capture_frame(&mut self, monitor_id: Option<u32>) -> Result<CaptureFrame, CaptureError>;
    fn supported_formats(&self) -> Vec<CaptureFormat>;
    fn set_target_fps(&mut self, fps: u32);
    fn current_fps(&self) -> f32;
}

#[derive(Debug, Clone)]
pub struct CaptureFrame {
    pub data: Bytes,
    pub width: u32,
    pub height: u32,
    pub format: CaptureFormat,
    pub timestamp: std::time::Instant,
}

#[cfg(windows)]
pub struct WindowsCapturer {
    capturer: WinCapturer,
    monitor_manager: MonitorManager,
    target_fps: u32,
    last_frame_time: Option<std::time::Instant>,
}

#[cfg(windows)]
impl WindowsCapturer {
    pub fn new() -> Result<Self, CaptureError> {
        let capturer = WinCapturer::new()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;
        let monitor_manager = MonitorManager::new()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;
        
        Ok(Self {
            capturer,
            monitor_manager,
            target_fps: 30,
            last_frame_time: None,
        })
    }

    pub fn list_monitors(&self) -> Vec<MonitorInfo> {
        self.monitor_manager.monitors()
            .iter()
            .map(|m| MonitorInfo {
                id: m.handle as u32,
                name: m.device_name.clone(),
                is_primary: m.is_primary,
                width: (m.bounds.right - m.bounds.left) as u32,
                height: (m.bounds.bottom - m.bounds.top) as u32,
            })
            .collect()
    }
}

#[cfg(windows)]
#[async_trait(?Send)]
impl PlatformCapturer for WindowsCapturer {
    async fn capture_frame(&mut self, monitor_id: Option<u32>) -> Result<CaptureFrame, CaptureError> {
        // Frame rate limiting
        if let Some(last_time) = self.last_frame_time {
            let elapsed = last_time.elapsed();
            let min_interval = std::time::Duration::from_secs(1) / self.target_fps.max(1);
            if elapsed < min_interval {
                tokio::time::sleep(min_interval - elapsed).await;
            }
        }

        let frame = self.capturer.capture_frame()
            .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

        self.last_frame_time = Some(std::time::Instant::now());

        Ok(CaptureFrame {
            data: Bytes::from(frame.bgra.clone()),
            width: frame.width,
            height: frame.height,
            format: CaptureFormat::Bgra8888,
            timestamp: std::time::Instant::now(),
        })
    }

    fn supported_formats(&self) -> Vec<CaptureFormat> {
        vec![CaptureFormat::Bgra8888]
    }

    fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps.min(60); // Cap at 60 FPS
        info!("Set target FPS to {}", self.target_fps);
    }

    fn current_fps(&self) -> f32 {
        // TODO: Calculate actual FPS from frame timing
        self.target_fps as f32
    }
}

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub is_primary: bool,
    pub width: u32,
    pub height: u32,
}
