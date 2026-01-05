#![cfg(target_os = "linux")]

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SystemdError {
    #[error("Service not found")]
    NotFound,
    #[error("Failed to reload service: {0}")]
    ReloadFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("systemd error: {0}")]
    Systemd(String),
}

/// systemd service manager
pub struct SystemdService {
    unit_name: String,
    unit_file_path: PathBuf,
}

impl SystemdService {
    /// Create service with unit name
    pub fn new(unit_name: String) -> Self {
        let unit_file_path = PathBuf::from("/etc/systemd/system")
            .join(format!("{}.service", unit_name));

        Self {
            unit_name,
            unit_file_path,
        }
    }

    /// Generate unit file content
    pub fn generate_unit_file(
        &self,
        exec_path: &str,
        args: &[String],
        user: Option<&str>,
        restart_policy: RestartPolicy,
        watchdog_sec: Option<u32>,
    ) -> String {
        let restart = match restart_policy {
            RestartPolicy::Always => "always",
            RestartPolicy::OnFailure => "on-failure",
            RestartPolicy::Never => "no",
        };

        let watchdog_line = if let Some(sec) = watchdog_sec {
            format!("WatchdogSec={}\n", sec)
        } else {
            String::new()
        };

        let user_line = if let Some(u) = user {
            format!("User={}\n", u)
        } else {
            String::new()
        };

        format!(
            r#"[Unit]
Description=ZRC Agent Service
After=network.target graphical-session.target
Wants=graphical-session.target

[Service]
Type=notify
ExecStart={} {}
Restart={}
RestartSec=10
{}
{}
# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ReadWritePaths=/var/lib/zrc-agent

[Install]
WantedBy=default.target
"#,
            exec_path,
            args.join(" "),
            restart,
            watchdog_line,
            user_line
        )
    }

    /// Install service
    pub fn install(
        &self,
        exec_path: &str,
        args: &[String],
        user: Option<&str>,
        restart_policy: RestartPolicy,
        watchdog_sec: Option<u32>,
    ) -> Result<(), SystemdError> {
        // Write unit file
        let unit_content = self.generate_unit_file(exec_path, args, user, restart_policy, watchdog_sec);
        std::fs::write(&self.unit_file_path, unit_content)?;

        // Reload systemd
        std::process::Command::new("systemctl")
            .arg("daemon-reload")
            .output()
            .map_err(|e| SystemdError::ReloadFailed(e.to_string()))?;

        // Enable service
        std::process::Command::new("systemctl")
            .arg("enable")
            .arg(&self.unit_name)
            .output()
            .map_err(|e| SystemdError::ReloadFailed(e.to_string()))?;

        Ok(())
    }

    /// Start service
    pub fn start(&self) -> Result<(), SystemdError> {
        std::process::Command::new("systemctl")
            .arg("start")
            .arg(&self.unit_name)
            .output()
            .map_err(|e| SystemdError::ReloadFailed(e.to_string()))?;
        Ok(())
    }

    /// Stop service
    pub fn stop(&self) -> Result<(), SystemdError> {
        std::process::Command::new("systemctl")
            .arg("stop")
            .arg(&self.unit_name)
            .output()
            .map_err(|e| SystemdError::ReloadFailed(e.to_string()))?;
        Ok(())
    }

    /// Send sd_notify ready
    #[cfg(feature = "systemd")]
    pub fn notify_ready(&self) -> Result<(), SystemdError> {
        use libsystemd::daemon::{notify, NotifyState};

        notify(false, &[NotifyState::Ready])
            .map_err(|e| SystemdError::Systemd(format!("sd_notify failed: {}", e)))?;
        Ok(())
    }

    #[cfg(not(feature = "systemd"))]
    pub fn notify_ready(&self) -> Result<(), SystemdError> {
        // Fallback: use systemd-notify command
        std::process::Command::new("systemd-notify")
            .arg("--ready")
            .output()
            .map_err(|e| SystemdError::Systemd(format!("systemd-notify failed: {}", e)))?;
        Ok(())
    }

    /// Send sd_notify status
    #[cfg(feature = "systemd")]
    pub fn notify_status(&self, status: &str) -> Result<(), SystemdError> {
        use libsystemd::daemon::{notify, NotifyState};

        notify(false, &[NotifyState::Status(status)])
            .map_err(|e| SystemdError::Systemd(format!("sd_notify failed: {}", e)))?;
        Ok(())
    }

    #[cfg(not(feature = "systemd"))]
    pub fn notify_status(&self, status: &str) -> Result<(), SystemdError> {
        // Fallback: use systemd-notify command
        std::process::Command::new("systemd-notify")
            .arg(&format!("STATUS={}", status))
            .output()
            .map_err(|e| SystemdError::Systemd(format!("systemd-notify failed: {}", e)))?;
        Ok(())
    }

    /// Send sd_notify watchdog
    #[cfg(feature = "systemd")]
    pub fn notify_watchdog(&self) -> Result<(), SystemdError> {
        use libsystemd::daemon::{notify, NotifyState};

        notify(false, &[NotifyState::Watchdog])
            .map_err(|e| SystemdError::Systemd(format!("sd_notify failed: {}", e)))?;
        Ok(())
    }

    #[cfg(not(feature = "systemd"))]
    pub fn notify_watchdog(&self) -> Result<(), SystemdError> {
        // Fallback: use systemd-notify command
        std::process::Command::new("systemd-notify")
            .arg("WATCHDOG=1")
            .output()
            .map_err(|e| SystemdError::Systemd(format!("systemd-notify failed: {}", e)))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}
