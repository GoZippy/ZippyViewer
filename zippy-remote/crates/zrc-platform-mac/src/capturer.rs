#![cfg(target_os = "macos")]

use crate::capture_sck::{SckCapturer, SckError};
use crate::capture_cg::{CgCapturer, CgError};
use crate::monitor::{MonitorManager, MonitorInfo};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("ScreenCaptureKit error: {0}")]
    Sck(#[from] SckError),
    #[error("CGDisplayStream error: {0}")]
    Cg(#[from] CgError),
    #[error("No capture backend available")]
    NoBackend,
    #[error("Permission denied")]
    PermissionDenied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    ScreenCaptureKit,
    CGDisplayStream,
}

enum CaptureBackend {
    ScreenCaptureKit(SckCapturer),
    CGDisplayStream(CgCapturer),
}

/// Unified macOS capturer with automatic backend selection
pub struct MacCapturer {
    backend: CaptureBackend,
    backend_type: BackendType,
    monitor_manager: MonitorManager,
    current_monitor: Option<u32>,
    target_fps: u32,
}

impl MacCapturer {
    /// Create capturer with best available backend
    pub fn new() -> Result<Self, CaptureError> {
        let monitor_manager = MonitorManager::new()
            .map_err(|e| CaptureError::Cg(CgError::StreamCreationFailed(e.to_string())))?;

        let (backend, backend_type) = if SckCapturer::is_available() {
            let capturer = SckCapturer::new()?;
            (CaptureBackend::ScreenCaptureKit(capturer), BackendType::ScreenCaptureKit)
        } else {
            let main_display = monitor_manager.main_monitor()
                .ok_or(CaptureError::NoBackend)?;
            let capturer = CgCapturer::new(Some(main_display.id))?;
            (CaptureBackend::CGDisplayStream(capturer), BackendType::CGDisplayStream)
        };

        Ok(Self {
            backend,
            backend_type,
            monitor_manager,
            current_monitor: None,
            target_fps: 60,
        })
    }

    /// Capture a frame
    pub fn capture_frame(&mut self) -> Result<Vec<u8>, CaptureError> {
        match &mut self.backend {
            CaptureBackend::ScreenCaptureKit(capturer) => {
                capturer.capture_frame().map_err(CaptureError::Sck)
            }
            CaptureBackend::CGDisplayStream(capturer) => {
                capturer.capture_frame().map_err(CaptureError::Cg)
            }
        }
    }

    /// Get backend type
    pub fn backend_type(&self) -> BackendType {
        self.backend_type
    }

    /// List available monitors
    pub fn list_monitors(&self) -> Vec<&MonitorInfo> {
        self.monitor_manager.monitors()
    }

    /// Select monitor to capture
    pub fn select_monitor(&mut self, monitor_id: u32) -> Result<(), CaptureError> {
        if self.monitor_manager.get_monitor(monitor_id).is_none() {
            return Err(CaptureError::Cg(CgError::DisplayNotFound));
        }

        self.current_monitor = Some(monitor_id);
        Ok(())
    }

    /// Set target FPS
    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps;
        match &mut self.backend {
            CaptureBackend::ScreenCaptureKit(capturer) => {
                capturer.set_target_fps(fps);
            }
            CaptureBackend::CGDisplayStream(_) => {
                // CGDisplayStream doesn't support FPS control directly
            }
        }
    }
}
