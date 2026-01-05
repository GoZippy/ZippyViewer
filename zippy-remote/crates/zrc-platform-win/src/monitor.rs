#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use std::collections::HashMap;
use thiserror::Error;
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
};

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("enumeration failed")]
    EnumerationFailed,
    #[error("monitor not found")]
    NotFound,
}

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub handle: isize, // Store as isize to avoid Hash issues with HMONITOR
    pub device_name: String,
    pub friendly_name: String,
    pub bounds: RECT,
    pub work_area: RECT,
    pub is_primary: bool,
    pub dpi: u32,
    pub refresh_rate: u32,
}

/// Monitor enumeration and management
pub struct MonitorManager {
    pub(crate) monitors: Vec<MonitorInfo>,
    pub(crate) monitor_map: HashMap<isize, usize>,
}

impl MonitorManager {
    /// Create monitor manager and enumerate monitors
    pub fn new() -> Result<Self, MonitorError> {
        let mut manager = Self {
            monitors: Vec::new(),
            monitor_map: HashMap::new(),
        };
        manager.refresh()?;
        Ok(manager)
    }

    /// Create empty monitor manager (fallback)
    pub fn empty() -> Self {
        Self {
            monitors: Vec::new(),
            monitor_map: HashMap::new(),
        }
    }

    /// Refresh monitor list
    pub fn refresh(&mut self) -> Result<(), MonitorError> {
        self.monitors.clear();
        self.monitor_map.clear();

        unsafe {
            let mut monitors: Vec<MonitorInfo> = Vec::new();

            let result = EnumDisplayMonitors(
                None,
                None,
                Some(Self::enum_proc),
                LPARAM(&mut monitors as *mut _ as isize),
            );

            if !result.as_bool() {
                return Err(MonitorError::EnumerationFailed);
            }

            self.monitors = monitors;

            // Build index map
            for (idx, monitor) in self.monitors.iter().enumerate() {
                self.monitor_map.insert(monitor.handle, idx);
            }

            Ok(())
        }
    }

    unsafe extern "system" fn enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprect: *mut RECT,
        lparam: LPARAM,
    ) -> windows::core::BOOL {
        let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

        let mut info = MONITORINFOEXW {
            monitorInfo: MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            szDevice: [0u16; 32],
        };

        if !GetMonitorInfoW(hmonitor, &mut info.monitorInfo).as_bool() {
            return windows::core::BOOL::from(true); // Continue enumeration
        }

        let device_name = String::from_utf16_lossy(&info.szDevice);
        let friendly_name = Self::get_friendly_name(&device_name);

        // Get DPI
        let dpi = Self::get_dpi_for_monitor(hmonitor);

        // Get refresh rate (simplified - would need to query display mode)
        let refresh_rate = 60; // Placeholder

        let monitor_info = MonitorInfo {
            handle: hmonitor.0 as isize,
            device_name: device_name.clone(),
            friendly_name,
            bounds: info.monitorInfo.rcMonitor,
            work_area: info.monitorInfo.rcWork,
            is_primary: (info.monitorInfo.dwFlags & 1) != 0, // MONITORINFOF_PRIMARY = 1
            dpi,
            refresh_rate,
        };

        monitors.push(monitor_info);

        windows::core::BOOL::from(true) // Continue enumeration
    }

    fn get_friendly_name(device_name: &str) -> String {
        // Try to get friendly name from registry or use device name
        // Simplified - in production, query registry
        device_name.to_string()
    }

    fn get_dpi_for_monitor(_hmonitor: HMONITOR) -> u32 {
        unsafe {
            // Use GetDpiForMonitor if available (Windows 8.1+)
            // Fallback to system DPI
            let hdc = GetDC(None);
            let dpi_x = if !hdc.is_invalid() {
                let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSX) as u32;
                let _ = ReleaseDC(None, hdc);
                dpi
            } else {
                96
            };
            
            dpi_x.max(96) // Minimum 96 DPI
        }
    }

    /// Get all monitors
    pub fn monitors(&self) -> &[MonitorInfo] {
        &self.monitors
    }

    /// Get primary monitor
    pub fn primary_monitor(&self) -> Option<&MonitorInfo> {
        self.monitors.iter().find(|m| m.is_primary)
    }

    /// Get monitor by index
    pub fn get_monitor(&self, index: usize) -> Option<&MonitorInfo> {
        self.monitors.get(index)
    }

    /// Get monitor by handle
    pub fn get_monitor_by_handle(&self, handle: isize) -> Option<&MonitorInfo> {
        self.monitor_map.get(&handle).and_then(|&idx| self.monitors.get(idx))
    }

    /// Handle display change notification
    pub fn handle_display_change(&mut self) -> Result<(), MonitorError> {
        self.refresh()
    }
}
