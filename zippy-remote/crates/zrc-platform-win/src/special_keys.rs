#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use std::sync::Arc;
use thiserror::Error;
use windows::Win32::{
    System::Shutdown::*,
    UI::Input::KeyboardAndMouse::*,
};

use crate::injector::{WinInjector, InputError};

#[derive(Debug, Error)]
pub enum SpecialKeyError {
    #[error("elevation required")]
    ElevationRequired,
    #[error("injection failed: {0}")]
    InjectionFailed(#[from] InputError),
    #[error("SAS library not available")]
    SasNotAvailable,
}

/// Special key sequence handler
pub struct SpecialKeyHandler {
    injector: Arc<WinInjector>,
    is_service: bool,
    is_elevated: bool,
}

impl SpecialKeyHandler {
    /// Create special key handler
    pub fn new(injector: Arc<WinInjector>) -> Self {
        let is_service = Self::is_service_context();
        let is_elevated = injector.is_elevated();
        
        Self {
            injector,
            is_service,
            is_elevated,
        }
    }

    /// Check if running in service context
    fn is_service_context() -> bool {
        // Check if running as SYSTEM or in session 0
        // Simplified check - in production, check process token
        false // Placeholder
    }

    /// Send Ctrl+Alt+Del (requires SYSTEM context)
    pub fn send_ctrl_alt_del(&self) -> Result<(), SpecialKeyError> {
        if !self.is_service && !self.is_elevated {
            return Err(SpecialKeyError::ElevationRequired);
        }

        // Try to use SendSAS if available (requires sas.dll)
        // For now, we'll use a workaround: inject the keys individually
        // Note: This won't work for secure attention sequence, but is a placeholder
        
        // In production, you'd use:
        // let sas = windows::core::load_library("sas.dll")?;
        // let send_sas: extern "system" fn(BOOL) -> HRESULT = ...;
        // send_sas(FALSE)?;
        
        // For now, return error indicating SAS is required
        Err(SpecialKeyError::SasNotAvailable)
    }

    /// Send Alt+Tab
    pub fn send_alt_tab(&self) -> Result<(), SpecialKeyError> {
        // Create a new injector for mutation
        let mut injector = WinInjector::new();
        
        // Hold Alt
        injector.inject_key(VK_MENU.0 as u32, true)?;
        
        // Press Tab
        injector.inject_key(VK_TAB.0 as u32, true)?;
        injector.inject_key(VK_TAB.0 as u32, false)?;
        
        // Release Alt
        injector.inject_key(VK_MENU.0 as u32, false)?;
        
        Ok(())
    }

    /// Send Win+L (lock workstation)
    pub fn send_lock_workstation(&self) -> Result<(), SpecialKeyError> {
        unsafe {
            // Use LockWorkStation API
            LockWorkStation()
                .map_err(|_| SpecialKeyError::InjectionFailed(InputError::SendFailed))
        }
    }

    /// Send Ctrl+Shift+Esc (Task Manager)
    pub fn send_task_manager(&self) -> Result<(), SpecialKeyError> {
        // Create a new injector for mutation
        let mut injector = WinInjector::new();
        
        // Hold Ctrl
        injector.inject_key(VK_CONTROL.0 as u32, true)?;
        
        // Hold Shift
        injector.inject_key(VK_SHIFT.0 as u32, true)?;
        
        // Press Esc
        injector.inject_key(VK_ESCAPE.0 as u32, true)?;
        injector.inject_key(VK_ESCAPE.0 as u32, false)?;
        
        // Release Shift
        injector.inject_key(VK_SHIFT.0 as u32, false)?;
        
        // Release Ctrl
        injector.inject_key(VK_CONTROL.0 as u32, false)?;
        
        Ok(())
    }

    /// Log special key sequence attempt (for audit)
    pub fn log_special_key(&self, sequence: &str) {
        // In production, this would log to Event Log or audit system
        eprintln!("[AUDIT] Special key sequence: {}", sequence);
    }
}
