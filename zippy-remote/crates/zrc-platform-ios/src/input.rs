//! Input event types for iOS

use uniffi::Enum;

/// Mouse button types
#[derive(Debug, Clone, Copy, Enum, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Input event types exposed to Swift
#[derive(Debug, Clone, Enum)]
pub enum InputEvent {
    /// Mouse movement
    MouseMove { x: i32, y: i32 },
    /// Mouse button click
    MouseClick {
        x: i32,
        y: i32,
        button: MouseButton,
    },
    /// Keyboard key press
    KeyPress { code: u32, down: bool },
    /// Scroll event
    Scroll { delta_x: i32, delta_y: i32 },
}

impl InputEvent {
    /// Convert to zrc-core InputEvent
    pub fn to_core_event(&self) -> zrc_core::platform::InputEvent {
        match self {
            InputEvent::MouseMove { x, y } => zrc_core::platform::InputEvent::MouseMove { x: *x, y: *y },
            InputEvent::MouseClick { x, y, button } => {
                let button_code = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                };
                zrc_core::platform::InputEvent::MouseButton {
                    button: button_code,
                    down: true,
                }
            }
            InputEvent::KeyPress { code, down } => {
                zrc_core::platform::InputEvent::Key {
                    keycode: *code,
                    down: *down,
                }
            }
            InputEvent::Scroll { delta_x, delta_y } => {
                // Note: zrc-core doesn't have scroll events in InputEvent enum
                // This would need to be handled differently or added to zrc-core
                // For now, we'll use a workaround or extend the core
                zrc_core::platform::InputEvent::MouseMove { x: *delta_x, y: *delta_y }
            }
        }
    }
}
