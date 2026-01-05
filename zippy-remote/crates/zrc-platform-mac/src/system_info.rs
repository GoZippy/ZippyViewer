#![cfg(target_os = "macos")]

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
    pub os_version: String,
    pub computer_name: String,
    pub hardware_model: String,
    pub is_apple_silicon: bool,
}

impl SystemInfo {
    /// Get system information
    pub fn get() -> Result<Self, SystemInfoError> {
        let os_version = Command::new("sw_vers")
            .arg("-productVersion")
            .output()?
            .stdout
            .iter()
            .take_while(|&&b| b != b'\n')
            .copied()
            .collect::<Vec<_>>();
        let os_version = String::from_utf8_lossy(&os_version).to_string();

        let computer_name = Command::new("scutil")
            .arg("--get")
            .arg("ComputerName")
            .output()?
            .stdout
            .iter()
            .take_while(|&&b| b != b'\n')
            .copied()
            .collect::<Vec<_>>();
        let computer_name = String::from_utf8_lossy(&computer_name).to_string();

        let hardware_model = Command::new("sysctl")
            .arg("-n")
            .arg("hw.model")
            .output()?
            .stdout
            .iter()
            .take_while(|&&b| b != b'\n')
            .copied()
            .collect::<Vec<_>>();
        let hardware_model = String::from_utf8_lossy(&hardware_model).to_string();

        // Detect Apple Silicon
        let is_apple_silicon = Command::new("sysctl")
            .arg("-n")
            .arg("machdep.cpu.brand_string")
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout).ok()
            })
            .map(|brand| brand.contains("Apple"))
            .unwrap_or(false);

        Ok(Self {
            os_version,
            computer_name,
            hardware_model,
            is_apple_silicon,
        })
    }
}
