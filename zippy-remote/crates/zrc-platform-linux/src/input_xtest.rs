#![cfg(target_os = "linux")]

use std::collections::HashSet;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as XprotoConnectionExt, Window};
use x11rb::protocol::xtest::{ConnectionExt as XTestConnectionExt, FAKE_INPUT_TYPE_MOTION_NOTIFY, FAKE_INPUT_TYPE_BUTTON_PRESS, FAKE_INPUT_TYPE_BUTTON_RELEASE, FAKE_INPUT_TYPE_KEY_PRESS, FAKE_INPUT_TYPE_KEY_RELEASE};
use x11rb::rust_connection::RustConnection;
use x11rb::xcb_ffi::XCBConnection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XTestError {
    #[error("X11 connection failed: {0}")]
    ConnectionFailed(String),
    #[error("XTest extension not available")]
    XTestNotAvailable,
    #[error("Failed to inject event: {0}")]
    InjectionFailed(String),
    #[error("Invalid keycode: {0}")]
    InvalidKeycode(u32),
}

/// XTest-based input injection (X11)
pub struct XTestInjector {
    conn: XCBConnection,
    screen_num: usize,
    root_window: Window,
    held_keys: HashSet<u32>,
}

impl XTestInjector {
    /// Check if XTest extension is available
    pub fn is_available() -> bool {
        if let Ok((conn, _)) = x11rb::connect(None) {
            if let Ok(ext_info) = conn.xtest_get_version(2, 2) {
                ext_info.is_ok()
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Create new XTest injector
    pub fn new() -> Result<Self, XTestError> {
        if !Self::is_available() {
            return Err(XTestError::XTestNotAvailable);
        }

        let (conn, screen_num) = x11rb::connect(None)
            .map_err(|e| XTestError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

        let screen = &conn.setup().roots[screen_num];
        let root_window = screen.root;

        Ok(Self {
            conn,
            screen_num,
            root_window,
            held_keys: HashSet::new(),
        })
    }

    /// Inject mouse move
    pub fn inject_mouse_move(&self, x: i32, y: i32) -> Result<(), XTestError> {
        self.conn
            .xtest_fake_input(
                FAKE_INPUT_TYPE_MOTION_NOTIFY,
                0,
                0,
                self.root_window,
                x as i16,
                y as i16,
                0,
            )
            .map_err(|e| XTestError::InjectionFailed(format!("Mouse move failed: {}", e)))?;

        self.conn
            .flush()
            .map_err(|e| XTestError::InjectionFailed(format!("Flush failed: {}", e)))?;

        Ok(())
    }

    /// Inject mouse button
    pub fn inject_mouse_button(&self, button: u8, down: bool) -> Result<(), XTestError> {
        let event_type = if down {
            FAKE_INPUT_TYPE_BUTTON_PRESS
        } else {
            FAKE_INPUT_TYPE_BUTTON_RELEASE
        };

        // X11 button numbers: 1=left, 2=middle, 3=right, 4=scroll up, 5=scroll down
        let button_code = match button {
            0 => 1, // Left
            1 => 3, // Right
            2 => 2, // Middle
            _ => button,
        };

        self.conn
            .xtest_fake_input(
                event_type,
                button_code as u8,
                0,
                self.root_window,
                0,
                0,
                0,
            )
            .map_err(|e| XTestError::InjectionFailed(format!("Mouse button failed: {}", e)))?;

        self.conn
            .flush()
            .map_err(|e| XTestError::InjectionFailed(format!("Flush failed: {}", e)))?;

        Ok(())
    }

    /// Inject mouse scroll
    pub fn inject_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<(), XTestError> {
        // X11 uses button 4 (scroll up) and 5 (scroll down) for scrolling
        // For smooth scrolling, we can send multiple button events
        if delta_y > 0 {
            // Scroll up
            for _ in 0..delta_y.min(10) {
                self.inject_mouse_button(4, true)?;
                self.inject_mouse_button(4, false)?;
            }
        } else if delta_y < 0 {
            // Scroll down
            for _ in 0..(-delta_y).min(10) {
                self.inject_mouse_button(5, true)?;
                self.inject_mouse_button(5, false)?;
            }
        }

        // Horizontal scroll (if supported)
        if delta_x != 0 {
            // Some systems use button 6/7 for horizontal scroll
            // For now, we'll skip horizontal scroll
        }

        Ok(())
    }

    /// Inject key
    pub fn inject_key(&mut self, keycode: u32, down: bool) -> Result<(), XTestError> {
        // X11 keycodes are 8-bit (0-255)
        if keycode > 255 {
            return Err(XTestError::InvalidKeycode(keycode));
        }

        let event_type = if down {
            FAKE_INPUT_TYPE_KEY_PRESS
        } else {
            FAKE_INPUT_TYPE_KEY_RELEASE
        };

        self.conn
            .xtest_fake_input(
                event_type,
                keycode as u8,
                0,
                self.root_window,
                0,
                0,
                0,
            )
            .map_err(|e| XTestError::InjectionFailed(format!("Key injection failed: {}", e)))?;

        self.conn
            .flush()
            .map_err(|e| XTestError::InjectionFailed(format!("Flush failed: {}", e)))?;

        if down {
            self.held_keys.insert(keycode);
        } else {
            self.held_keys.remove(&keycode);
        }

        Ok(())
    }

    /// Release all held keys
    pub fn release_all_keys(&mut self) -> Result<(), XTestError> {
        let keys: Vec<u32> = self.held_keys.iter().copied().collect();
        for keycode in keys {
            self.inject_key(keycode, false)?;
        }
        Ok(())
    }
}

impl Drop for XTestInjector {
    fn drop(&mut self) {
        let _ = self.release_all_keys();
    }
}
