//! Input event structures for Android

use serde::{Deserialize, Serialize};

/// Input event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEventType {
    MouseMove,
    MouseDown,
    MouseUp,
    Scroll,
    KeyDown,
    KeyUp,
    KeyChar,
}

/// Input event for sending to remote device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEvent {
    /// Event type
    pub event_type: InputEventType,
    /// Mouse X coordinate
    pub mouse_x: i32,
    /// Mouse Y coordinate
    pub mouse_y: i32,
    /// Mouse button (0 = none, 1 = left, 2 = right, 3 = middle)
    pub button: u32,
    /// Key code
    pub key_code: u32,
    /// Modifier keys bitmask
    pub modifiers: u32,
    /// Text input (for KeyChar events)
    pub text: String,
    /// Scroll delta X
    pub scroll_delta_x: i32,
    /// Scroll delta Y
    pub scroll_delta_y: i32,
}

impl InputEvent {
    /// Create a mouse move event
    pub fn mouse_move(x: i32, y: i32) -> Self {
        Self {
            event_type: InputEventType::MouseMove,
            mouse_x: x,
            mouse_y: y,
            button: 0,
            key_code: 0,
            modifiers: 0,
            text: String::new(),
            scroll_delta_x: 0,
            scroll_delta_y: 0,
        }
    }

    /// Create a mouse click event
    pub fn mouse_click(x: i32, y: i32, button: u32) -> Self {
        Self {
            event_type: InputEventType::MouseDown,
            mouse_x: x,
            mouse_y: y,
            button,
            key_code: 0,
            modifiers: 0,
            text: String::new(),
            scroll_delta_x: 0,
            scroll_delta_y: 0,
        }
    }

    /// Create a scroll event
    pub fn scroll(delta_x: i32, delta_y: i32) -> Self {
        Self {
            event_type: InputEventType::Scroll,
            mouse_x: 0,
            mouse_y: 0,
            button: 0,
            key_code: 0,
            modifiers: 0,
            text: String::new(),
            scroll_delta_x: delta_x,
            scroll_delta_y: delta_y,
        }
    }

    /// Create a key event
    pub fn key(key_code: u32, down: bool) -> Self {
        Self {
            event_type: if down {
                InputEventType::KeyDown
            } else {
                InputEventType::KeyUp
            },
            mouse_x: 0,
            mouse_y: 0,
            button: 0,
            key_code,
            modifiers: 0,
            text: String::new(),
            scroll_delta_x: 0,
            scroll_delta_y: 0,
        }
    }

    /// Create a text input event
    pub fn text_input(text: String) -> Self {
        Self {
            event_type: InputEventType::KeyChar,
            mouse_x: 0,
            mouse_y: 0,
            button: 0,
            key_code: 0,
            modifiers: 0,
            text,
            scroll_delta_x: 0,
            scroll_delta_y: 0,
        }
    }
    
    /// Create from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
    
    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}
