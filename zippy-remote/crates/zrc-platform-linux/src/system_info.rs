#![cfg(target_os = "linux")]

use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SystemInfoError {
    #[error("Failed to get system info: {0}")]
    CommandFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub distribution: String,
    pub kernel_version: String,
    pub hostname: String,
    pub architecture: String,
    pub is_vm: bool,
}

impl SystemInfo {
    /// Get system information
    pub fn get() -> Result<Self, SystemInfoError> {
        // Get distribution
        let distribution = std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content.lines()
                    .find(|line| line.starts_with("PRETTY_NAME="))
                    .and_then(|line| {
                        line.strip_prefix("PRETTY_NAME=")
                            .map(|s| s.trim_matches('"').to_string())
                    })
            })
            .unwrap_or_else(|| "Unknown".to_string());

        // Get kernel version
        let kernel_version = Command::new("uname")
            .arg("-r")
            .output()?
            .stdout
            .iter()
            .take_while(|&&b| b != b'\n')
            .copied()
            .collect::<Vec<_>>();
        let kernel_version = String::from_utf8_lossy(&kernel_version).to_string();

        // Get hostname
        let hostname = std::fs::read_to_string("/etc/hostname")
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Get architecture
        let architecture = Command::new("uname")
            .arg("-m")
            .output()?
            .stdout
            .iter()
            .take_while(|&&b| b != b'\n')
            .copied()
            .collect::<Vec<_>>();
        let architecture = String::from_utf8_lossy(&architecture).to_string();

        // Detect VM
        let is_vm = std::fs::read_to_string("/sys/class/dmi/id/product_name")
            .ok()
            .map(|s| {
                let s_lower = s.to_lowercase();
                s_lower.contains("vmware") || s_lower.contains("virtualbox") || 
                s_lower.contains("qemu") || s_lower.contains("kvm") ||
                s_lower.contains("xen") || s_lower.contains("hyperv")
            })
            .unwrap_or(false);

        Ok(Self {
            distribution,
            kernel_version,
            hostname,
            architecture,
            is_vm,
        })
    }
}
