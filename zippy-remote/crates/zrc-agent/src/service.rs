use async_trait::async_trait;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("service initialization failed: {0}")]
    InitFailed(String),
    #[error("service start failed: {0}")]
    StartFailed(String),
    #[error("service stop failed: {0}")]
    StopFailed(String),
    #[error("signal handling failed: {0}")]
    SignalFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

/// Service host trait - does NOT require Send + Sync since Windows service handles
/// contain raw pointers that are not thread-safe.
#[async_trait(?Send)]
pub trait ServiceHost {
    async fn start(&mut self) -> Result<(), ServiceError>;
    async fn stop(&mut self) -> Result<(), ServiceError>;
    async fn handle_signal(&mut self, signal: i32) -> Result<(), ServiceError>;
    fn status(&self) -> ServiceStatus;
}

pub struct ForegroundService {
    status: ServiceStatus,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl ForegroundService {
    pub fn new() -> (Self, tokio::sync::watch::Receiver<bool>) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        (
            Self {
                status: ServiceStatus::Stopped,
                shutdown: tx,
            },
            rx,
        )
    }
}

#[async_trait(?Send)]
impl ServiceHost for ForegroundService {
    async fn start(&mut self) -> Result<(), ServiceError> {
        info!("Starting zrc-agent in foreground mode");
        self.status = ServiceStatus::Starting;
        
        // Use cross-platform ctrl_c signal handler
        // Unix-specific signals are handled via cfg attributes in platform-specific code
        self.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ServiceError> {
        info!("Stopping zrc-agent");
        self.status = ServiceStatus::Stopping;
        let _ = self.shutdown.send(true);
        self.status = ServiceStatus::Stopped;
        Ok(())
    }

    async fn handle_signal(&mut self, _signal: i32) -> Result<(), ServiceError> {
        self.stop().await
    }

    fn status(&self) -> ServiceStatus {
        self.status
    }
}

#[cfg(windows)]
pub mod windows {
    use super::*;
    use std::sync::mpsc;
    use zrc_platform_win::service::{WinService, ServiceControl};

    pub struct WindowsServiceHost {
        service: Option<WinService>,
        control_rx: mpsc::Receiver<ServiceControl>,
        control_tx: mpsc::Sender<ServiceControl>,
        service_name: String,
        status: ServiceStatus,
    }

    impl WindowsServiceHost {
        pub fn new(service_name: String) -> Result<Self, ServiceError> {
            let (control_tx, control_rx) = mpsc::channel();
            Ok(Self {
                service: None,
                control_rx,
                control_tx,
                service_name,
                status: ServiceStatus::Stopped,
            })
        }

        /// Initialize the Windows service (must be called from service main)
        pub fn init_service(&mut self) -> Result<(), ServiceError> {
            let service = WinService::new(self.service_name.clone(), self.control_tx.clone())
                .map_err(|e| ServiceError::InitFailed(e.to_string()))?;
            self.service = Some(service);
            Ok(())
        }
    }

    #[async_trait(?Send)]
    impl ServiceHost for WindowsServiceHost {
        async fn start(&mut self) -> Result<(), ServiceError> {
            use zrc_platform_win::service::SERVICE_RUNNING;
            
            self.status = ServiceStatus::Starting;
            
            // Initialize service if not already done
            if self.service.is_none() {
                self.init_service()?;
            }
            
            // Set service status to running
            if let Some(ref service) = self.service {
                service.set_status(SERVICE_RUNNING)
                    .map_err(|e| ServiceError::StartFailed(e.to_string()))?;
            }
            
            self.status = ServiceStatus::Running;
            Ok(())
        }

        async fn stop(&mut self) -> Result<(), ServiceError> {
            self.status = ServiceStatus::Stopping;
            // Service will be stopped via control handler
            self.status = ServiceStatus::Stopped;
            Ok(())
        }

        async fn handle_signal(&mut self, signal: i32) -> Result<(), ServiceError> {
            // Windows service control signals
            match signal {
                1 => self.stop().await, // SERVICE_CONTROL_STOP
                _ => Ok(()),
            }
        }

        fn status(&self) -> ServiceStatus {
            self.status
        }
    }
}

#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;

    pub struct SystemdServiceHost {
        status: ServiceStatus,
    }

    impl SystemdServiceHost {
        pub fn new() -> Result<Self, ServiceError> {
            Ok(Self {
                status: ServiceStatus::Stopped,
            })
        }
    }

    #[async_trait]
    impl ServiceHost for SystemdServiceHost {
        async fn start(&mut self) -> Result<(), ServiceError> {
            info!("Starting zrc-agent as systemd service");
            self.status = ServiceStatus::Starting;
            // systemd integration would go here
            self.status = ServiceStatus::Running;
            Ok(())
        }

        async fn stop(&mut self) -> Result<(), ServiceError> {
            info!("Stopping zrc-agent systemd service");
            self.status = ServiceStatus::Stopped;
            Ok(())
        }

        async fn handle_signal(&mut self, _signal: i32) -> Result<(), ServiceError> {
            self.stop().await
        }

        fn status(&self) -> ServiceStatus {
            self.status
        }
    }
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;

    pub struct LaunchdServiceHost {
        status: ServiceStatus,
    }

    impl LaunchdServiceHost {
        pub fn new() -> Result<Self, ServiceError> {
            Ok(Self {
                status: ServiceStatus::Stopped,
            })
        }
    }

    #[async_trait]
    impl ServiceHost for LaunchdServiceHost {
        async fn start(&mut self) -> Result<(), ServiceError> {
            info!("Starting zrc-agent as launchd daemon");
            self.status = ServiceStatus::Starting;
            // launchd integration would go here
            self.status = ServiceStatus::Running;
            Ok(())
        }

        async fn stop(&mut self) -> Result<(), ServiceError> {
            info!("Stopping zrc-agent launchd daemon");
            self.status = ServiceStatus::Stopped;
            Ok(())
        }

        async fn handle_signal(&mut self, _signal: i32) -> Result<(), ServiceError> {
            self.stop().await
        }

        fn status(&self) -> ServiceStatus {
            self.status
        }
    }
}
