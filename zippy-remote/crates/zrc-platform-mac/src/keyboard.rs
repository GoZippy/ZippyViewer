#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeyboardError {
    #[error("Failed to create event: {0}")]
    EventCreationFailed(String),
    #[error("Failed to post event: {0}")]
    EventPostFailed(String),
    #[error("Accessibility permission required")]
    PermissionDenied,
    #[error("Invalid keycode: {0}")]
    InvalidKeycode(u32),
}

/// Keyboard input injection
pub struct MacKeyboard {
    held_keys: HashSet<u32>,
    _private: (),
}

impl MacKeyboard {
    pub fn new() -> Result<Self, KeyboardError> {
        // TODO: Check accessibility permission
        Ok(Self {
            held_keys: HashSet::new(),
            _private: (),
        })
    }

    /// Inject key event
    pub fn inject_key(&mut self, keycode: u32, down: bool) -> Result<(), KeyboardError> {
        // TODO: Implement using CGEventCreateKeyboardEvent
        if down {
            self.held_keys.insert(keycode);
        } else {
            self.held_keys.remove(&keycode);
        }
        Ok(())
    }

    /// Inject text (Unicode)
    pub fn inject_text(&self, text: &str) -> Result<(), KeyboardError> {
        // TODO: Implement using CGEventKeyboardSetUnicodeString
        Ok(())
    }

    /// Release all held keys
    pub fn release_all_keys(&mut self) -> Result<(), KeyboardError> {
        let keys: Vec<u32> = self.held_keys.iter().copied().collect();
        for keycode in keys {
            self.inject_key(keycode, false)?;
        }
        Ok(())
    }
}

impl Drop for MacKeyboard {
    fn drop(&mut self) {
        let _ = self.release_all_keys();
    }
}
