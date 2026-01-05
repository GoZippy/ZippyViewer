#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use thiserror::Error;
use windows::Win32::{
    Foundation::*,
    System::Threading::GetCurrentThreadId,
    System::StationsAndDesktops::*,
};

#[derive(Debug, Error)]
pub enum UacError {
    #[error("desktop access denied")]
    AccessDenied,
    #[error("desktop switch failed")]
    SwitchFailed,
    #[error("elevation required")]
    ElevationRequired,
}

/// UAC and secure desktop handling
pub struct UacHandler {
    is_system: bool,
}

impl UacHandler {
    /// Create UAC handler
    pub fn new() -> Self {
        let is_system = Self::is_system_context();
        Self { is_system }
    }

    /// Check if running as SYSTEM
    fn is_system_context() -> bool {
        // Simplified check - in production, check process token
        false // Placeholder
    }

    /// Detect if on secure desktop
    pub fn is_secure_desktop(&self) -> bool {
        unsafe {
            let desktop = GetThreadDesktop(GetCurrentThreadId());
            let desktop = match desktop {
                Ok(d) => d,
                Err(_) => return false,
            };

            if desktop.is_invalid() {
                return false;
            }

            // Get desktop name
            let mut name = [0u16; 256];
            let mut needed = 0u32;
            let _ = GetUserObjectInformationW(
                HANDLE(desktop.0),
                UOI_NAME,
                Some(name.as_mut_ptr() as *mut _),
                (name.len() * 2) as u32,
                Some(&mut needed),
            );

            let name_str = String::from_utf16_lossy(&name[..needed as usize / 2]);
            name_str.contains("Winlogon") || name_str.contains("SAS")
        }
    }

    /// Switch to secure desktop (requires SYSTEM)
    pub fn switch_to_secure_desktop(&self) -> Result<(), UacError> {
        if !self.is_system {
            return Err(UacError::ElevationRequired);
        }

        unsafe {
            let desktop = OpenDesktopW(
                &windows::core::HSTRING::from("Winlogon"),
                DESKTOP_CONTROL_FLAGS::default(),
                false,
                0x1FF, // DESKTOP_ALL_ACCESS
            )
            .map_err(|_| UacError::AccessDenied)?;

            SetThreadDesktop(desktop).map_err(|_| UacError::SwitchFailed)?;
            Ok(())
        }
    }

    /// Switch back to default desktop
    pub fn switch_to_default_desktop(&self) -> Result<(), UacError> {
        unsafe {
            let desktop = OpenDesktopW(
                &windows::core::HSTRING::from("Default"),
                DESKTOP_CONTROL_FLAGS::default(),
                false,
                0x1FF, // DESKTOP_ALL_ACCESS
            )
            .map_err(|_| UacError::AccessDenied)?;

            SetThreadDesktop(desktop).map_err(|_| UacError::SwitchFailed)?;
            Ok(())
        }
    }

    /// Get current desktop name
    pub fn current_desktop_name(&self) -> String {
        unsafe {
            let desktop = GetThreadDesktop(GetCurrentThreadId());
            let desktop = match desktop {
                Ok(d) => d,
                Err(_) => return String::new(),
            };

            if desktop.is_invalid() {
                return String::new();
            }

            let mut name = [0u16; 256];
            let mut needed = 0u32;
            let _ = GetUserObjectInformationW(
                HANDLE(desktop.0),
                UOI_NAME,
                Some(name.as_mut_ptr() as *mut _),
                (name.len() * 2) as u32,
                Some(&mut needed),
            );

            String::from_utf16_lossy(&name[..needed as usize / 2])
        }
    }
}
