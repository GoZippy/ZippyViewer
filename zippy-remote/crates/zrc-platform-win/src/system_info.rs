#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use windows::Win32::{
    Foundation::*,
    System::SystemInformation::*,
    UI::WindowsAndMessaging::*,
};

use crate::monitor::MonitorManager;

/// System information
pub struct SystemInfo {
    pub windows_version: String,
    pub build_number: u32,
    pub computer_name: String,
    pub domain: String,
    pub user_name: String,
    pub is_rdp_session: bool,
    pub is_vm: bool,
    pub uptime_seconds: u64,
    pub monitor_manager: MonitorManager,
}

impl SystemInfo {
    /// Collect system information
    pub fn collect() -> Self {
        unsafe {
            let mut version_info = OSVERSIONINFOW {
                dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
                ..Default::default()
            };
            let _ = GetVersionExW(&mut version_info);

            let windows_version = format!(
                "{}.{}.{}",
                version_info.dwMajorVersion, version_info.dwMinorVersion, version_info.dwBuildNumber
            );

            let build_number = version_info.dwBuildNumber;

            // Get computer name
            let mut computer_name = [0u16; 256];
            let mut size = computer_name.len() as u32;
            let _ = GetComputerNameExW(
                ComputerNamePhysicalNetBIOS,
                Some(windows::core::PWSTR(computer_name.as_mut_ptr())),
                &mut size,
            );
            let computer_name = String::from_utf16_lossy(&computer_name[..size as usize]);

            // Get domain
            let mut domain = [0u16; 256];
            let mut size = domain.len() as u32;
            let _ = GetComputerNameExW(
                ComputerNamePhysicalDnsDomain,
                Some(windows::core::PWSTR(domain.as_mut_ptr())),
                &mut size,
            );
            let domain = String::from_utf16_lossy(&domain[..size as usize]);

            // Get user name - simplified, just use environment variable
            let user_name = std::env::var("USERNAME").unwrap_or_default();

            // Check if RDP session
            let is_rdp_session = GetSystemMetrics(SM_REMOTESESSION) != 0;

            // Check if VM (simplified - check for common VM indicators)
            let is_vm = Self::detect_vm();

            // Get uptime
            let uptime_seconds = Self::get_uptime();

            // Get monitor manager
            let monitor_manager = MonitorManager::new().unwrap_or_else(|_| {
                // Fallback to empty manager if enumeration fails
                MonitorManager::empty()
            });

            Self {
                windows_version,
                build_number,
                computer_name,
                domain,
                user_name,
                is_rdp_session,
                is_vm,
                uptime_seconds,
                monitor_manager,
            }
        }
    }

    /// Get display configuration
    pub fn display_config(&self) -> DisplayConfig {
        DisplayConfig {
            monitor_count: self.monitor_manager.monitors().len(),
            primary_monitor: self.monitor_manager.primary_monitor().cloned(),
            all_monitors: self.monitor_manager.monitors().to_vec(),
        }
    }

    /// Get network adapters
    pub fn network_adapters(&self) -> Vec<NetworkAdapter> {
        Self::enumerate_network_adapters()
    }

    fn enumerate_network_adapters() -> Vec<NetworkAdapter> {
        // Simplified implementation - in production, use GetAdaptersAddresses
        // For now, return empty list to avoid complex API issues
        Vec::new()
    }

    fn detect_vm() -> bool {
        unsafe {
            // Check for common VM indicators using system firmware
            // Simplified - production would check multiple indicators
            let mut buffer = [0u8; 256];
            let size = GetSystemFirmwareTable(
                FIRMWARE_TABLE_PROVIDER(0x52534D42), // 'RSMB'
                0,
                Some(&mut buffer),
            );
            
            if size > 0 {
                // Clamp size to buffer length to avoid out-of-bounds access
                let actual_size = (size as usize).min(buffer.len());
                // Check for VM strings in firmware table
                let firmware = String::from_utf8_lossy(&buffer[..actual_size]);
                firmware.contains("VMware") || firmware.contains("VirtualBox") || firmware.contains("QEMU")
            } else {
                false
            }
        }
    }

    fn get_uptime() -> u64 {
        unsafe {
            let tick_count = GetTickCount64();
            tick_count / 1000 // Convert to seconds
        }
    }
}

#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub monitor_count: usize,
    pub primary_monitor: Option<crate::monitor::MonitorInfo>,
    pub all_monitors: Vec<crate::monitor::MonitorInfo>,
}

#[derive(Debug, Clone)]
pub struct NetworkAdapter {
    pub friendly_name: String,
    pub description: String,
    pub adapter_type: String,
    pub operational_status: String,
}
