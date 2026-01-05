#![cfg(target_os = "linux")]
#![cfg(feature = "uinput")]

use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UinputError {
    #[error("uinput device not available")]
    NotAvailable,
    #[error("Failed to create device: {0}")]
    DeviceCreationFailed(String),
    #[error("Failed to inject event: {0}")]
    InjectionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(feature = "uinput")]
use mouse_keyboard_input::*;

/// uinput-based input injection (requires privileges)
pub struct UinputInjector {
    #[cfg(feature = "uinput")]
    mouse_device: Mouse,
    #[cfg(feature = "uinput")]
    keyboard_device: Keyboard,
    held_keys: HashSet<u32>,
}

impl UinputInjector {
    /// Check if uinput is available
    pub fn is_available() -> bool {
        std::path::Path::new("/dev/uinput").exists()
    }

    /// Create new uinput injector
    pub fn new() -> Result<Self, UinputError> {
        if !Self::is_available() {
            return Err(UinputError::NotAvailable);
        }

        #[cfg(feature = "uinput")]
        {
            let mouse_device = Mouse::new()
                .map_err(|e| UinputError::DeviceCreationFailed(format!("Mouse device creation failed: {}", e)))?;

            let keyboard_device = Keyboard::new()
                .map_err(|e| UinputError::DeviceCreationFailed(format!("Keyboard device creation failed: {}", e)))?;

            Ok(Self {
                mouse_device,
                keyboard_device,
                held_keys: HashSet::new(),
            })
        }

        #[cfg(not(feature = "uinput"))]
        {
            Err(UinputError::NotAvailable)
        }
    }

    /// Inject mouse move (relative)
    pub fn inject_mouse_move(&self, dx: i32, dy: i32) -> Result<(), UinputError> {
        #[cfg(feature = "uinput")]
        {
            self.mouse_device
                .move_relative(dx, dy)
                .map_err(|e| UinputError::InjectionFailed(format!("Mouse move failed: {}", e)))?;
        }
        Ok(())
    }

    /// Inject mouse button
    pub fn inject_mouse_button(&self, button: u8, down: bool) -> Result<(), UinputError> {
        #[cfg(feature = "uinput")]
        {
            let button_code = match button {
                0 => Button::Left,
                1 => Button::Right,
                2 => Button::Middle,
                _ => return Err(UinputError::InjectionFailed(format!("Invalid button: {}", button))),
            };

            if down {
                self.mouse_device
                    .press(&button_code)
                    .map_err(|e| UinputError::InjectionFailed(format!("Mouse button press failed: {}", e)))?;
            } else {
                self.mouse_device
                    .release(&button_code)
                    .map_err(|e| UinputError::InjectionFailed(format!("Mouse button release failed: {}", e)))?;
            }
        }
        Ok(())
    }

    /// Inject mouse scroll
    pub fn inject_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<(), UinputError> {
        #[cfg(feature = "uinput")]
        {
            if delta_y != 0 {
                self.mouse_device
                    .scroll(delta_y)
                    .map_err(|e| UinputError::InjectionFailed(format!("Mouse scroll failed: {}", e)))?;
            }
            // Horizontal scroll not directly supported by mouse-keyboard-input
            // Would need to use button events or a different approach
        }
        Ok(())
    }

    /// Inject key
    pub fn inject_key(&mut self, keycode: u32, down: bool) -> Result<(), UinputError> {
        #[cfg(feature = "uinput")]
        {
            // Convert keycode to Key enum
            // This is a simplified mapping - in practice, you'd need a proper keycode to Key mapping
            let key = self.keycode_to_key(keycode)?;

            if down {
                self.keyboard_device
                    .press(&key)
                    .map_err(|e| UinputError::InjectionFailed(format!("Key press failed: {}", e)))?;
                self.held_keys.insert(keycode);
            } else {
                self.keyboard_device
                    .release(&key)
                    .map_err(|e| UinputError::InjectionFailed(format!("Key release failed: {}", e)))?;
                self.held_keys.remove(&keycode);
            }
        }
        Ok(())
    }

    #[cfg(feature = "uinput")]
    fn keycode_to_key(&self, keycode: u32) -> Result<Key, UinputError> {
        // This is a simplified mapping - you'd need a proper keycode table
        // For now, we'll use a basic mapping
        use Key::*;
        match keycode {
            1 => Ok(Escape),
            2 => Ok(Number1),
            3 => Ok(Number2),
            4 => Ok(Number3),
            5 => Ok(Number4),
            6 => Ok(Number5),
            7 => Ok(Number6),
            8 => Ok(Number7),
            9 => Ok(Number8),
            10 => Ok(Number9),
            11 => Ok(Number0),
            // Add more mappings as needed
            _ => Err(UinputError::InjectionFailed(format!("Unsupported keycode: {}", keycode))),
        }
    }

    /// Release all held keys
    pub fn release_all_keys(&mut self) -> Result<(), UinputError> {
        let keys: Vec<u32> = self.held_keys.iter().copied().collect();
        for keycode in keys {
            self.inject_key(keycode, false)?;
        }
        Ok(())
    }
}

impl Drop for UinputInjector {
    fn drop(&mut self) {
        let _ = self.release_all_keys();
        // Devices are automatically cleaned up when dropped
    }
}
