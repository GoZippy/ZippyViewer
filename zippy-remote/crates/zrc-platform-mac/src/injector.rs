#![cfg(target_os = "macos")]

use crate::mouse::{MacMouse, MouseError};
use crate::keyboard::{MacKeyboard, KeyboardError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InjectorError {
    #[error("Mouse error: {0}")]
    Mouse(#[from] MouseError),
    #[error("Keyboard error: {0}")]
    Keyboard(#[from] KeyboardError),
}

/// Unified input injector
pub struct MacInjector {
    mouse: MacMouse,
    keyboard: MacKeyboard,
}

impl MacInjector {
    pub fn new() -> Result<Self, InjectorError> {
        Ok(Self {
            mouse: MacMouse::new()?,
            keyboard: MacKeyboard::new()?,
        })
    }

    /// Inject mouse move
    pub fn inject_mouse_move(&self, x: i32, y: i32) -> Result<(), InjectorError> {
        // Convert to macOS coordinate system (bottom-left origin)
        // TODO: Get screen height and convert
        self.mouse.inject_mouse_move(x as f64, y as f64)?;
        Ok(())
    }

    /// Inject mouse button
    pub fn inject_mouse_button(&self, button: u8, down: bool, x: i32, y: i32) -> Result<(), InjectorError> {
        self.mouse.inject_mouse_button(button, down, x as f64, y as f64)?;
        Ok(())
    }

    /// Inject mouse scroll
    pub fn inject_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<(), InjectorError> {
        self.mouse.inject_mouse_scroll(delta_x, delta_y)?;
        Ok(())
    }

    /// Inject key
    pub fn inject_key(&mut self, keycode: u32, down: bool) -> Result<(), InjectorError> {
        self.keyboard.inject_key(keycode, down)?;
        Ok(())
    }

    /// Inject text
    pub fn inject_text(&self, text: &str) -> Result<(), InjectorError> {
        self.keyboard.inject_text(text)?;
        Ok(())
    }
}
