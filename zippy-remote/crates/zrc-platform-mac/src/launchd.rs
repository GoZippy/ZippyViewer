#![cfg(target_os = "macos")]

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LaunchdError {
    #[error("Service not found")]
    NotFound,
    #[error("Failed to load service: {0}")]
    LoadFailed(String),
    #[error("Failed to unload service: {0}")]
    UnloadFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// LaunchAgent/LaunchDaemon service manager
pub struct LaunchdService {
    label: String,
    plist_path: PathBuf,
}

impl LaunchdService {
    /// Create service with label
    pub fn new(label: String) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let plist_path = PathBuf::from(home)
            .join("Library/LaunchAgents")
            .join(format!("{}.plist", label));

        Self {
            label,
            plist_path,
        }
    }

    /// Generate plist content
    pub fn generate_plist(&self, program_path: &str, args: &[String]) -> String {
        format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
{}
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/{}.out.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/{}.err.log</string>
</dict>
</plist>"#,
            self.label,
            program_path,
            args.iter().map(|a| format!("        <string>{}</string>", a)).collect::<Vec<_>>().join("\n"),
            self.label,
            self.label
        )
    }

    /// Install service
    pub fn install(&self, program_path: &str, args: &[String]) -> Result<(), LaunchdError> {
        // Create directory if it doesn't exist
        if let Some(parent) = self.plist_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write plist
        let plist_content = self.generate_plist(program_path, args);
        std::fs::write(&self.plist_path, plist_content)?;

        // Load service
        self.load()
    }

    /// Load service
    pub fn load(&self) -> Result<(), LaunchdError> {
        std::process::Command::new("launchctl")
            .arg("load")
            .arg(&self.plist_path)
            .output()
            .map_err(|e| LaunchdError::LoadFailed(e.to_string()))?;
        Ok(())
    }

    /// Unload service
    pub fn unload(&self) -> Result<(), LaunchdError> {
        std::process::Command::new("launchctl")
            .arg("unload")
            .arg(&self.plist_path)
            .output()
            .map_err(|e| LaunchdError::UnloadFailed(e.to_string()))?;
        Ok(())
    }

    /// Start service
    pub fn start(&self) -> Result<(), LaunchdError> {
        std::process::Command::new("launchctl")
            .arg("start")
            .arg(&self.label)
            .output()
            .map_err(|e| LaunchdError::LoadFailed(e.to_string()))?;
        Ok(())
    }

    /// Stop service
    pub fn stop(&self) -> Result<(), LaunchdError> {
        std::process::Command::new("launchctl")
            .arg("stop")
            .arg(&self.label)
            .output()
            .map_err(|e| LaunchdError::UnloadFailed(e.to_string()))?;
        Ok(())
    }
}
