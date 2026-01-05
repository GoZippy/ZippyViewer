use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

#[cfg(windows)]
use zrc_platform_win::injector::WinInjector;

#[derive(Debug, Error)]
pub enum InputError {
    #[error("input injection failed: {0}")]
    InjectionFailed(String),
    #[error("coordinate out of bounds")]
    CoordinateOutOfBounds,
    #[error("key not found")]
    KeyNotFound,
}

#[async_trait]
pub trait PlatformInjector: Send + Sync {
    async fn inject_mouse_move(&mut self, x: i32, y: i32) -> Result<(), InputError>;
    async fn inject_mouse_button(&mut self, button: MouseButton, pressed: bool) -> Result<(), InputError>;
    async fn inject_mouse_scroll(&mut self, delta_x: i32, delta_y: i32) -> Result<(), InputError>;
    async fn inject_key(&mut self, key: u32, pressed: bool) -> Result<(), InputError>;
    async fn inject_text(&mut self, text: &str) -> Result<(), InputError>;
    async fn release_all_keys(&mut self) -> Result<(), InputError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[cfg(windows)]
pub struct WindowsInjector {
    injector: WinInjector,
    held_keys: std::collections::HashSet<u32>,
}

#[cfg(windows)]
impl WindowsInjector {
    pub fn new() -> Result<Self, InputError> {
        let injector = WinInjector::new();
        
        Ok(Self {
            injector,
            held_keys: std::collections::HashSet::new(),
        })
    }
}

#[cfg(windows)]
#[async_trait]
impl PlatformInjector for WindowsInjector {
    async fn inject_mouse_move(&mut self, x: i32, y: i32) -> Result<(), InputError> {
        self.injector.inject_mouse_move(x, y)
            .map_err(|e| InputError::InjectionFailed(e.to_string()))
    }

    async fn inject_mouse_button(&mut self, button: MouseButton, pressed: bool) -> Result<(), InputError> {
        // WinInjector uses u32 for button: 1=Left, 2=Right, 3=Middle, 4=X1, 5=X2
        let win_button = match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            MouseButton::Middle => 3,
            MouseButton::X1 => 4,
            MouseButton::X2 => 5,
        };
        
        self.injector.inject_mouse_button(win_button, pressed)
            .map_err(|e| InputError::InjectionFailed(e.to_string()))
    }

    async fn inject_mouse_scroll(&mut self, delta_x: i32, delta_y: i32) -> Result<(), InputError> {
        // WinInjector uses (delta, horizontal) signature
        // Handle vertical scroll (delta_y) first, then horizontal (delta_x)
        if delta_y != 0 {
            self.injector.inject_mouse_scroll(delta_y, false)
                .map_err(|e| InputError::InjectionFailed(e.to_string()))?;
        }
        if delta_x != 0 {
            self.injector.inject_mouse_scroll(delta_x, true)
                .map_err(|e| InputError::InjectionFailed(e.to_string()))?;
        }
        Ok(())
    }

    async fn inject_key(&mut self, key: u32, pressed: bool) -> Result<(), InputError> {
        self.injector.inject_key(key, pressed)
            .map_err(|e| InputError::InjectionFailed(e.to_string()))?;
        
        if pressed {
            self.held_keys.insert(key);
        } else {
            self.held_keys.remove(&key);
        }
        Ok(())
    }

    async fn inject_text(&mut self, text: &str) -> Result<(), InputError> {
        self.injector.inject_text(text)
            .map_err(|e| InputError::InjectionFailed(e.to_string()))
    }

    async fn release_all_keys(&mut self) -> Result<(), InputError> {
        // Release all currently held keys
        let keys_to_release: Vec<u32> = self.held_keys.iter().copied().collect();
        for key in keys_to_release {
            let _ = self.inject_key(key, false).await;
        }
        self.held_keys.clear();
        info!("Released all held keys");
        Ok(())
    }
}
