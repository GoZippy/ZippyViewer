#![cfg(target_os = "linux")]

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WaylandInputError {
    #[error("Wayland session not detected")]
    NotWayland,
    #[error("XWayland not available")]
    XWaylandNotAvailable,
    #[error("libei not available")]
    LibeiNotAvailable,
    #[error("Input injection not supported on Wayland")]
    NotSupported,
}

/// Wayland input status and capabilities
pub struct WaylandInputStatus {
    is_wayland: bool,
    has_xwayland: bool,
    has_libei: bool,
}

impl WaylandInputStatus {
    /// Detect Wayland input capabilities
    pub fn detect() -> Self {
        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
        let has_xwayland = std::env::var("DISPLAY").is_ok() && is_wayland;
        let has_libei = false; // TODO: Check for libei availability

        Self {
            is_wayland,
            has_xwayland,
            has_libei,
        }
    }

    /// Check if XWayland fallback is available
    pub fn can_use_xwayland(&self) -> bool {
        self.has_xwayland
    }

    /// Check if libei is available
    pub fn has_libei(&self) -> bool {
        self.has_libei
    }

    /// Get limitation message
    pub fn limitation_message(&self) -> Option<&'static str> {
        if self.is_wayland && !self.has_xwayland && !self.has_libei {
            Some("Input injection on Wayland requires XWayland or libei support")
        } else {
            None
        }
    }
}
