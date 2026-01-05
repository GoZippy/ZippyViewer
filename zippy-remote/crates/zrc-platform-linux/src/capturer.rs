#![cfg(target_os = "linux")]

use crate::capture_x11_shm::{X11ShmCapturer, X11ShmError};
use crate::capture_x11_basic::{X11BasicCapturer, X11BasicError};
#[cfg(feature = "pipewire")]
use crate::capture_pipewire::{PipeWireCapturer, PipeWireError};
use crate::monitor::{MonitorManager, MonitorInfo};
use crate::desktop_env::SessionType;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("X11 SHM error: {0}")]
    X11Shm(#[from] X11ShmError),
    #[error("X11 basic error: {0}")]
    X11Basic(#[from] X11BasicError),
    #[cfg(feature = "pipewire")]
    #[error("PipeWire error: {0}")]
    PipeWire(#[from] PipeWireError),
    #[error("No capture backend available")]
    NoBackend,
    #[error("Monitor error: {0}")]
    Monitor(#[from] crate::monitor::MonitorError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    #[cfg(feature = "pipewire")]
    PipeWire,
    X11Shm,
    X11Basic,
}

enum CaptureBackend {
    #[cfg(feature = "pipewire")]
    PipeWire(PipeWireCapturer),
    X11Shm(X11ShmCapturer),
    X11Basic(X11BasicCapturer),
}

/// Unified Linux capturer with automatic backend selection
pub struct LinuxCapturer {
    backend: CaptureBackend,
    backend_type: BackendType,
    monitor_manager: MonitorManager,
    current_monitor: Option<u32>,
    target_fps: u32,
}

impl LinuxCapturer {
    /// Detect session type (X11 or Wayland)
    pub fn detect_session_type() -> SessionType {
        crate::desktop_env::DesktopEnvironmentInfo::detect().session_type
    }

    /// Create capturer with best available backend
    pub fn new() -> Result<Self, CaptureError> {
        let monitor_manager = MonitorManager::new()?;

        let session_type = Self::detect_session_type();

        let (backend, backend_type) = match session_type {
            SessionType::Wayland => {
                #[cfg(feature = "pipewire")]
                {
                    if PipeWireCapturer::is_available() {
                        let capturer = PipeWireCapturer::new()?;
                        (CaptureBackend::PipeWire(capturer), BackendType::PipeWire)
                    } else {
                        return Err(CaptureError::NoBackend);
                    }
                }
                #[cfg(not(feature = "pipewire"))]
                {
                    // Wayland requires PipeWire feature
                    return Err(CaptureError::NoBackend);
                }
            }
            SessionType::X11 | SessionType::XWayland => {
                if X11ShmCapturer::is_available() {
                    let capturer = X11ShmCapturer::new()?;
                    (CaptureBackend::X11Shm(capturer), BackendType::X11Shm)
                } else {
                    let capturer = X11BasicCapturer::new()?;
                    (CaptureBackend::X11Basic(capturer), BackendType::X11Basic)
                }
            }
            SessionType::Headless => {
                return Err(CaptureError::NoBackend);
            }
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
            #[cfg(feature = "pipewire")]
            CaptureBackend::PipeWire(capturer) => {
                capturer.capture_frame().map_err(CaptureError::PipeWire)
            }
            CaptureBackend::X11Shm(capturer) => {
                capturer.capture_frame().map_err(CaptureError::X11Shm)
            }
            CaptureBackend::X11Basic(capturer) => {
                capturer.capture_frame().map_err(CaptureError::X11Basic)
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
            return Err(CaptureError::Monitor(crate::monitor::MonitorError::DisplayNotFound(monitor_id)));
        }

        self.current_monitor = Some(monitor_id);
        Ok(())
    }

    /// Set target FPS
    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps;
    }
}
