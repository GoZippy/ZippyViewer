#![cfg(target_os = "linux")]

use crate::input_xtest::{XTestInjector, XTestError};
#[cfg(feature = "uinput")]
use crate::input_uinput::{UinputInjector, UinputError};
use crate::wayland_input::WaylandInputStatus;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InjectorError {
    #[error("XTest error: {0}")]
    XTest(#[from] XTestError),
    #[cfg(feature = "uinput")]
    #[error("uinput error: {0}")]
    Uinput(#[from] UinputError),
    #[error("No input backend available")]
    NoBackend,
}

enum InputBackend {
    XTest(XTestInjector),
    #[cfg(feature = "uinput")]
    Uinput(UinputInjector),
}

/// Unified input injector
pub struct LinuxInjector {
    backend: InputBackend,
    wayland_status: WaylandInputStatus,
}

impl LinuxInjector {
    pub fn new() -> Result<Self, InjectorError> {
        let wayland_status = WaylandInputStatus::detect();

        let backend = if wayland_status.is_wayland {
            // On Wayland, try XWayland fallback first
            if wayland_status.can_use_xwayland() && XTestInjector::is_available() {
                InputBackend::XTest(XTestInjector::new()?)
            } else {
                #[cfg(feature = "uinput")]
                {
                    if UinputInjector::is_available() {
                        InputBackend::Uinput(UinputInjector::new()?)
                    } else {
                        return Err(InjectorError::NoBackend);
                    }
                }
                #[cfg(not(feature = "uinput"))]
                {
                    return Err(InjectorError::NoBackend);
                }
            }
        } else {
            // On X11, prefer XTest
            if XTestInjector::is_available() {
                InputBackend::XTest(XTestInjector::new()?)
            } else {
                #[cfg(feature = "uinput")]
                {
                    if UinputInjector::is_available() {
                        InputBackend::Uinput(UinputInjector::new()?)
                    } else {
                        return Err(InjectorError::NoBackend);
                    }
                }
                #[cfg(not(feature = "uinput"))]
                {
                    return Err(InjectorError::NoBackend);
                }
            }
        };

        Ok(Self {
            backend,
            wayland_status,
        })
    }

    /// Inject mouse move
    pub fn inject_mouse_move(&self, x: i32, y: i32) -> Result<(), InjectorError> {
        match &self.backend {
            InputBackend::XTest(injector) => {
                injector.inject_mouse_move(x, y)?;
            }
            #[cfg(feature = "uinput")]
            InputBackend::Uinput(injector) => {
                // uinput uses relative movement, so we need to calculate delta
                // For now, we'll use the absolute position as relative
                injector.inject_mouse_move(x, y)?;
            }
        }
        Ok(())
    }

    /// Inject mouse button
    pub fn inject_mouse_button(&self, button: u8, down: bool) -> Result<(), InjectorError> {
        match &self.backend {
            InputBackend::XTest(injector) => {
                injector.inject_mouse_button(button, down)?;
            }
            #[cfg(feature = "uinput")]
            InputBackend::Uinput(injector) => {
                injector.inject_mouse_button(button, down)?;
            }
        }
        Ok(())
    }

    /// Inject mouse scroll
    pub fn inject_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<(), InjectorError> {
        match &self.backend {
            InputBackend::XTest(injector) => {
                injector.inject_mouse_scroll(delta_x, delta_y)?;
            }
            #[cfg(feature = "uinput")]
            InputBackend::Uinput(injector) => {
                injector.inject_mouse_scroll(delta_x, delta_y)?;
            }
        }
        Ok(())
    }

    /// Inject key
    pub fn inject_key(&mut self, keycode: u32, down: bool) -> Result<(), InjectorError> {
        match &mut self.backend {
            InputBackend::XTest(injector) => {
                injector.inject_key(keycode, down)?;
            }
            #[cfg(feature = "uinput")]
            InputBackend::Uinput(injector) => {
                injector.inject_key(keycode, down)?;
            }
        }
        Ok(())
    }

    /// Inject text
    pub fn inject_text(&self, text: &str) -> Result<(), InjectorError> {
        // TODO: Implement text injection (convert to key events)
        // For now, this is a placeholder
        // In a real implementation, you'd convert each character to key events
        Ok(())
    }
}
