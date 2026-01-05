#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use std::sync::Arc;
use tokio::sync::Mutex;
use thiserror::Error;

use crate::capture_gdi::{BgraFrame, GdiCapturer, CaptureError as GdiError};
use crate::capture_dxgi::{DxgiCapturer, DxgiError};
use crate::capture_wgc::{WgcCapturer, WgcError};
use crate::monitor::MonitorManager;

#[derive(Debug, Error)]
pub enum CapturerError {
    #[error("GDI error: {0}")]
    Gdi(#[from] GdiError),
    #[error("DXGI error: {0}")]
    Dxgi(#[from] DxgiError),
    #[error("WGC error: {0}")]
    Wgc(#[from] WgcError),
    #[error("no capture backend available")]
    NoBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Wgc,
    Dxgi,
    Gdi,
}

enum CaptureBackend {
    Wgc(WgcCapturer),
    Dxgi(DxgiCapturer),
    Gdi(GdiCapturer),
}

/// Windows screen capture with automatic backend selection
pub struct WinCapturer {
    backend: CaptureBackend,
    backend_type: BackendType,
    monitor_manager: MonitorManager,
    current_monitor: Option<usize>,
}

impl WinCapturer {
    /// Create capturer with best available backend
    pub fn new() -> Result<Self, CapturerError> {
        let (backend, backend_type) = if WgcCapturer::is_available() {
            match WgcCapturer::new() {
                Ok(wgc) => (CaptureBackend::Wgc(wgc), BackendType::Wgc),
                Err(_) => {
                    // Fall back to DXGI
                    if DxgiCapturer::is_available() {
                        match DxgiCapturer::new() {
                            Ok(dxgi) => (CaptureBackend::Dxgi(dxgi), BackendType::Dxgi),
                            Err(_) => {
                                // Fall back to GDI
                                let gdi = GdiCapturer::new()?;
                                (CaptureBackend::Gdi(gdi), BackendType::Gdi)
                            }
                        }
                    } else {
                        // Fall back to GDI
                        let gdi = GdiCapturer::new()?;
                        (CaptureBackend::Gdi(gdi), BackendType::Gdi)
                    }
                }
            }
        } else if DxgiCapturer::is_available() {
            match DxgiCapturer::new() {
                Ok(dxgi) => (CaptureBackend::Dxgi(dxgi), BackendType::Dxgi),
                Err(_) => {
                    // Fall back to GDI
                    let gdi = GdiCapturer::new()?;
                    (CaptureBackend::Gdi(gdi), BackendType::Gdi)
                }
            }
        } else {
            // Use GDI as last resort
            let gdi = GdiCapturer::new()?;
            (CaptureBackend::Gdi(gdi), BackendType::Gdi)
        };

        let monitor_manager = MonitorManager::new().unwrap_or_else(|_| {
            MonitorManager {
                monitors: Vec::new(),
                monitor_map: std::collections::HashMap::new(),
            }
        });

        Ok(Self {
            backend,
            backend_type,
            monitor_manager,
            current_monitor: None,
        })
    }

    /// Force specific backend
    pub fn with_backend(backend_type: BackendType) -> Result<Self, CapturerError> {
        let backend = match backend_type {
            BackendType::Wgc => {
                if !WgcCapturer::is_available() {
                    return Err(CapturerError::NoBackend);
                }
                CaptureBackend::Wgc(WgcCapturer::new()?)
            }
            BackendType::Dxgi => {
                if !DxgiCapturer::is_available() {
                    return Err(CapturerError::NoBackend);
                }
                CaptureBackend::Dxgi(DxgiCapturer::new()?)
            }
            BackendType::Gdi => CaptureBackend::Gdi(GdiCapturer::new()?),
        };

        let monitor_manager = MonitorManager::new().unwrap_or_else(|_| {
            MonitorManager {
                monitors: Vec::new(),
                monitor_map: std::collections::HashMap::new(),
            }
        });

        Ok(Self {
            backend,
            backend_type,
            monitor_manager,
            current_monitor: None,
        })
    }

    /// Capture frame
    pub fn capture_frame(&mut self) -> Result<BgraFrame, CapturerError> {
        match &mut self.backend {
            CaptureBackend::Wgc(wgc) => wgc.capture_frame().map_err(CapturerError::Wgc),
            CaptureBackend::Dxgi(dxgi) => {
                // Use 100ms timeout for DXGI
                dxgi.capture_frame(100).map_err(CapturerError::Dxgi)
            }
            CaptureBackend::Gdi(gdi) => gdi.capture_frame().map_err(CapturerError::Gdi),
        }
    }

    /// Get current backend type
    pub fn backend_type(&self) -> BackendType {
        self.backend_type
    }

    /// List available monitors
    pub fn list_monitors(&self) -> &[crate::monitor::MonitorInfo] {
        self.monitor_manager.monitors()
    }

    /// Select monitor by index
    pub fn select_monitor(&mut self, index: usize) -> Result<(), CapturerError> {
        if index >= self.monitor_manager.monitors().len() {
            return Err(CapturerError::NoBackend);
        }
        self.current_monitor = Some(index);
        Ok(())
    }

    /// Handle display change (monitor hotplug)
    pub fn handle_display_change(&mut self) -> Result<(), CapturerError> {
        self.monitor_manager.handle_display_change()
            .map_err(|_| CapturerError::NoBackend)?;
        Ok(())
    }
}
