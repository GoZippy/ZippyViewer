#![cfg(target_os = "linux")]

use std::collections::HashMap;
use x11rb::connection::Connection;
use x11rb::protocol::randr::{ConnectionExt as RandrConnectionExt, GetOutputInfo, GetCrtcInfo};
use x11rb::protocol::xproto::ConnectionExt as XprotoConnectionExt;
use x11rb::rust_connection::RustConnection;
use x11rb::xcb_ffi::XCBConnection;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub is_primary: bool,
    pub scale_factor: f64,
    pub refresh_rate: f64,
}

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("Failed to enumerate displays: {0}")]
    EnumerationFailed(String),
    #[error("Display not found: {0}")]
    DisplayNotFound(u32),
    #[error("X11 connection failed: {0}")]
    ConnectionFailed(String),
}

/// Monitor manager for Linux
pub struct MonitorManager {
    conn: Option<XCBConnection>,
    monitors: HashMap<u32, MonitorInfo>,
    primary_monitor_id: Option<u32>,
}

impl MonitorManager {
    /// Create new monitor manager
    pub fn new() -> Result<Self, MonitorError> {
        let mut manager = Self {
            conn: None,
            monitors: HashMap::new(),
            primary_monitor_id: None,
        };
        manager.refresh()?;
        Ok(manager)
    }

    /// Refresh monitor list
    pub fn refresh(&mut self) -> Result<(), MonitorError> {
        self.monitors.clear();
        self.primary_monitor_id = None;

        // Check if we're in X11 session
        if std::env::var("DISPLAY").is_err() {
            // Not in X11, return empty list or use placeholder
            return Ok(());
        }

        // Connect to X11 if not already connected
        if self.conn.is_none() {
            let (conn, _) = x11rb::connect(None)
                .map_err(|e| MonitorError::ConnectionFailed(format!("Failed to connect: {}", e)))?;
            self.conn = Some(conn);
        }

        let conn = self.conn.as_ref().unwrap();
        let screen_num = 0; // Use first screen
        let screen = &conn.setup().roots[screen_num];
        let root_window = screen.root;

        // Query XRandR version
        let _version = conn.randr_query_version(1, 5)
            .map_err(|e| MonitorError::EnumerationFailed(format!("RandR query failed: {}", e)))?
            .reply()
            .map_err(|e| MonitorError::EnumerationFailed(format!("RandR reply failed: {}", e)))?;

        // Get screen resources
        let resources = conn.randr_get_screen_resources(root_window)
            .map_err(|e| MonitorError::EnumerationFailed(format!("GetScreenResources failed: {}", e)))?
            .reply()
            .map_err(|e| MonitorError::EnumerationFailed(format!("GetScreenResources reply failed: {}", e)))?;

        // Get primary output
        let primary = conn.randr_get_output_primary(root_window)
            .map_err(|e| MonitorError::EnumerationFailed(format!("GetOutputPrimary failed: {}", e)))?
            .reply()
            .map_err(|e| MonitorError::EnumerationFailed(format!("GetOutputPrimary reply failed: {}", e)))?;

        // Enumerate outputs
        for &output in resources.outputs.iter() {
            let output_info = conn.randr_get_output_info(output, 0)
                .map_err(|e| MonitorError::EnumerationFailed(format!("GetOutputInfo failed: {}", e)))?
                .reply()
                .map_err(|e| MonitorError::EnumerationFailed(format!("GetOutputInfo reply failed: {}", e)))?;

            if output_info.connection != 0 {
                // Output is connected
                let name = String::from_utf8_lossy(&output_info.name).to_string();
                let is_primary = primary.output == output;

                // Get CRTC info to get position and size
                if let Some(crtc) = output_info.crtc {
                    let crtc_info = conn.randr_get_crtc_info(crtc, 0)
                        .map_err(|e| MonitorError::EnumerationFailed(format!("GetCrtcInfo failed: {}", e)))?
                        .reply()
                        .map_err(|e| MonitorError::EnumerationFailed(format!("GetCrtcInfo reply failed: {}", e)))?;

                    let monitor = MonitorInfo {
                        id: output,
                        name,
                        width: crtc_info.width,
                        height: crtc_info.height,
                        x: crtc_info.x,
                        y: crtc_info.y,
                        is_primary,
                        scale_factor: 1.0, // TODO: Get actual scale factor
                        refresh_rate: 60.0, // TODO: Get actual refresh rate
                    };

                    if is_primary {
                        self.primary_monitor_id = Some(output);
                    }

                    self.monitors.insert(output, monitor);
                }
            }
        }

        // If no monitors found, add a default one
        if self.monitors.is_empty() {
            let screen = &conn.setup().roots[screen_num];
            let default_id = 1u32;
            self.primary_monitor_id = Some(default_id);
            self.monitors.insert(default_id, MonitorInfo {
                id: default_id,
                name: "Default Display".to_string(),
                width: screen.width_in_pixels as u32,
                height: screen.height_in_pixels as u32,
                x: 0,
                y: 0,
                is_primary: true,
                scale_factor: 1.0,
                refresh_rate: 60.0,
            });
        }

        Ok(())
    }

    /// Get all monitors
    pub fn monitors(&self) -> Vec<&MonitorInfo> {
        self.monitors.values().collect()
    }

    /// Get monitor by ID
    pub fn get_monitor(&self, id: u32) -> Option<&MonitorInfo> {
        self.monitors.get(&id)
    }

    /// Get primary monitor
    pub fn primary_monitor(&self) -> Option<&MonitorInfo> {
        self.primary_monitor_id.and_then(|id| self.monitors.get(&id))
    }
}
