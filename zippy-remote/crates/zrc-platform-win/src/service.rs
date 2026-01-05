#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use std::sync::{mpsc, Arc, Mutex};
use thiserror::Error;
use windows::Win32::{
    Foundation::*,
    System::Services::*,
    System::RemoteDesktop::WTSSESSION_NOTIFICATION,
};

// Re-export SERVICE_RUNNING for use by zrc-agent
pub use windows::Win32::System::Services::SERVICE_RUNNING;

// Session change event types (from WM_WTSSESSION_CHANGE)
const WTS_CONSOLE_CONNECT_VAL: u32 = 0x1;
const WTS_CONSOLE_DISCONNECT_VAL: u32 = 0x2;
const WTS_REMOTE_CONNECT_VAL: u32 = 0x3;
const WTS_REMOTE_DISCONNECT_VAL: u32 = 0x4;
const WTS_SESSION_LOGON_VAL: u32 = 0x5;
const WTS_SESSION_LOGOFF_VAL: u32 = 0x6;
const WTS_SESSION_LOCK_VAL: u32 = 0x7;
const WTS_SESSION_UNLOCK_VAL: u32 = 0x8;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("service control manager error")]
    ScmError,
    #[error("service registration failed")]
    RegistrationFailed,
    #[error("status reporting failed")]
    StatusFailed,
    #[error("service handler registration failed")]
    HandlerRegistrationFailed,
}

pub enum ServiceControl {
    Stop,
    Pause,
    Continue,
    Interrogate,
    Shutdown,
    SessionChange(SessionChangeEvent),
}

#[derive(Debug, Clone)]
pub enum SessionChangeEvent {
    SessionLogon(u32),
    SessionLogoff(u32),
    SessionLock(u32),
    SessionUnlock(u32),
    ConsoleConnect(u32),
    ConsoleDisconnect(u32),
    RemoteConnect(u32),
    RemoteDisconnect(u32),
}

/// Windows Service wrapper
pub struct WinService {
    service_name: String,
    status_handle: SERVICE_STATUS_HANDLE,
    current_status: Arc<Mutex<SERVICE_STATUS>>,
    #[allow(dead_code)]
    control_tx: mpsc::Sender<ServiceControl>,
}

impl WinService {
    /// Create service instance
    pub fn new(service_name: String, control_tx: mpsc::Sender<ServiceControl>) -> Result<Self, ServiceError> {
        unsafe {
            // Register service control handler
            let status_handle = RegisterServiceCtrlHandlerExW(
                &windows::core::HSTRING::from(&service_name),
                Some(Self::control_handler),
                None,
            ).map_err(|_| ServiceError::HandlerRegistrationFailed)?;

            let current_status = Arc::new(Mutex::new(SERVICE_STATUS {
                dwServiceType: SERVICE_WIN32_OWN_PROCESS,
                dwCurrentState: SERVICE_STOPPED,
                dwControlsAccepted: SERVICE_ACCEPT_STOP,
                dwWin32ExitCode: 0,
                dwServiceSpecificExitCode: 0,
                dwCheckPoint: 0,
                dwWaitHint: 0,
            }));

            Ok(Self {
                service_name,
                status_handle,
                current_status,
                control_tx,
            })
        }
    }

    /// Report status to SCM
    pub fn set_status(&self, state: SERVICE_STATUS_CURRENT_STATE) -> Result<(), ServiceError> {
        unsafe {
            let mut status = self.current_status.lock().unwrap();
            status.dwCurrentState = state;
            
            SetServiceStatus(self.status_handle, &*status)
                .map_err(|_| ServiceError::StatusFailed)
        }
    }

    /// Set service type and accepted controls
    pub fn configure(&self, service_type: u32, controls: u32) {
        let mut status = self.current_status.lock().unwrap();
        status.dwServiceType.0 = service_type;
        status.dwControlsAccepted = controls;
    }

    unsafe extern "system" fn control_handler(
        control: u32,
        event_type: u32,
        event_data: *mut std::ffi::c_void,
        _context: *mut std::ffi::c_void,
    ) -> u32 {
        match control {
            SERVICE_CONTROL_STOP => {
                // Send stop signal
            }
            SERVICE_CONTROL_PAUSE => {
                // Send pause signal
            }
            SERVICE_CONTROL_CONTINUE => {
                // Send continue signal
            }
            SERVICE_CONTROL_INTERROGATE => {
                // Report current status
            }
            SERVICE_CONTROL_SESSIONCHANGE => {
                // Handle session change
                if !event_data.is_null() {
                    let session_notification = &*(event_data as *const WTSSESSION_NOTIFICATION);
                    let session_id = session_notification.dwSessionId;
                    
                    let _event = match event_type {
                        WTS_SESSION_LOGON_VAL => SessionChangeEvent::SessionLogon(session_id),
                        WTS_SESSION_LOGOFF_VAL => SessionChangeEvent::SessionLogoff(session_id),
                        WTS_SESSION_LOCK_VAL => SessionChangeEvent::SessionLock(session_id),
                        WTS_SESSION_UNLOCK_VAL => SessionChangeEvent::SessionUnlock(session_id),
                        WTS_CONSOLE_CONNECT_VAL => SessionChangeEvent::ConsoleConnect(session_id),
                        WTS_CONSOLE_DISCONNECT_VAL => SessionChangeEvent::ConsoleDisconnect(session_id),
                        WTS_REMOTE_CONNECT_VAL => SessionChangeEvent::RemoteConnect(session_id),
                        WTS_REMOTE_DISCONNECT_VAL => SessionChangeEvent::RemoteDisconnect(session_id),
                        _ => return NO_ERROR.0,
                    };
                    
                    // Send event (would need service instance)
                }
            }
            _ => {}
        }
        
        NO_ERROR.0
    }

    /// Log to Event Log
    pub fn log_event(&self, event_type: u16, event_id: u32, message: &str) -> Result<(), ServiceError> {
        eprintln!("[EVENT LOG] Type: {}, ID: {}, Message: {}", event_type, event_id, message);
        Ok(())
    }
}
