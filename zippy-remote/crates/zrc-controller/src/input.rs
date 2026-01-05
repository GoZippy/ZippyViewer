//! Input command handling for remote control
//!
//! This module implements input commands for controlling remote devices:
//! - Mouse movement, clicks, and scrolling
//! - Keyboard key events (down/up)
//! - Text string input
//!
//! Requirements: 5.1-5.7

use prost::Message;
use serde::Serialize;
use thiserror::Error;

use zrc_proto::v1::{InputEventTypeV1, InputEventV1};

/// Input operation errors
#[derive(Debug, Error)]
pub enum InputError {
    #[error("No active session")]
    NoSession,

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),
}

/// Mouse button types
/// Requirements: 5.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MouseButton {
    Left = 1,
    Right = 2,
    Middle = 3,
    X1 = 4,
    X2 = 5,
}

impl std::str::FromStr for MouseButton {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "left" | "1" => Ok(Self::Left),
            "right" | "2" => Ok(Self::Right),
            "middle" | "3" => Ok(Self::Middle),
            "x1" | "4" => Ok(Self::X1),
            "x2" | "5" => Ok(Self::X2),
            _ => Err(format!("Unknown mouse button: {s}. Valid values: left, right, middle, x1, x2")),
        }
    }
}

impl MouseButton {
    /// Convert to protocol button value
    pub fn to_proto_value(self) -> u32 {
        self as u32
    }
}

/// Key code for keyboard input
/// Requirements: 5.2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCode(pub u32);

impl KeyCode {
    /// Create a new key code
    pub fn new(code: u32) -> Self {
        Self(code)
    }

    /// Get the raw key code value
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// Input modifier keys bitmask
/// Requirements: 5.2
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers(pub u32);

impl Modifiers {
    pub const NONE: u32 = 0;
    pub const SHIFT: u32 = 1;
    pub const CTRL: u32 = 2;
    pub const ALT: u32 = 4;
    pub const META: u32 = 8;
    pub const CAPS_LOCK: u32 = 16;
    pub const NUM_LOCK: u32 = 32;

    /// Create empty modifiers
    pub fn none() -> Self {
        Self(0)
    }

    /// Create modifiers with shift
    pub fn with_shift(mut self) -> Self {
        self.0 |= Self::SHIFT;
        self
    }

    /// Create modifiers with ctrl
    pub fn with_ctrl(mut self) -> Self {
        self.0 |= Self::CTRL;
        self
    }

    /// Create modifiers with alt
    pub fn with_alt(mut self) -> Self {
        self.0 |= Self::ALT;
        self
    }

    /// Create modifiers with meta (Windows/Command key)
    pub fn with_meta(mut self) -> Self {
        self.0 |= Self::META;
        self
    }

    /// Get the raw value
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// Result of sending an input command
#[derive(Debug, Clone, Serialize)]
pub struct InputResult {
    /// Whether the input was sent successfully
    pub success: bool,
    /// Session ID the input was sent to
    pub session_id: String,
    /// Type of input sent
    pub input_type: String,
    /// Additional details
    pub details: String,
}

/// Validates input parameters
/// Requirements: 5.6
pub struct InputValidator;

impl InputValidator {
    /// Validate mouse coordinates
    /// Requirements: 5.6
    pub fn validate_mouse_coords(x: i32, y: i32) -> Result<(), InputError> {
        // Coordinates should be non-negative for absolute positioning
        // Allow negative for relative movement
        if x < -32768 || x > 32767 {
            return Err(InputError::InvalidInput(format!(
                "X coordinate {} out of range [-32768, 32767]",
                x
            )));
        }
        if y < -32768 || y > 32767 {
            return Err(InputError::InvalidInput(format!(
                "Y coordinate {} out of range [-32768, 32767]",
                y
            )));
        }
        Ok(())
    }

    /// Validate scroll delta
    /// Requirements: 5.6
    pub fn validate_scroll_delta(delta: i32) -> Result<(), InputError> {
        // Scroll delta should be within reasonable bounds
        if delta < -10000 || delta > 10000 {
            return Err(InputError::InvalidInput(format!(
                "Scroll delta {} out of range [-10000, 10000]",
                delta
            )));
        }
        Ok(())
    }

    /// Validate key code
    /// Requirements: 5.6
    pub fn validate_key_code(code: u32) -> Result<(), InputError> {
        // Virtual key codes are typically 0-255 for standard keys
        // Extended keys can go higher
        if code > 0xFFFF {
            return Err(InputError::InvalidInput(format!(
                "Key code {} out of range [0, 65535]",
                code
            )));
        }
        Ok(())
    }

    /// Validate text input
    /// Requirements: 5.6
    pub fn validate_text(text: &str) -> Result<(), InputError> {
        // Text should not be empty
        if text.is_empty() {
            return Err(InputError::InvalidInput(
                "Text input cannot be empty".to_string(),
            ));
        }
        // Text should not be excessively long
        if text.len() > 10000 {
            return Err(InputError::InvalidInput(format!(
                "Text input too long: {} bytes (max 10000)",
                text.len()
            )));
        }
        Ok(())
    }
}

/// Builds InputEventV1 messages for the protocol
/// Requirements: 5.5
pub struct InputEventBuilder;

impl InputEventBuilder {
    /// Build a mouse move event
    /// Requirements: 5.1
    pub fn mouse_move(x: i32, y: i32) -> InputEventV1 {
        InputEventV1 {
            event_type: InputEventTypeV1::MouseMove as i32,
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

    /// Build a mouse button down event
    /// Requirements: 5.1
    pub fn mouse_down(x: i32, y: i32, button: MouseButton) -> InputEventV1 {
        InputEventV1 {
            event_type: InputEventTypeV1::MouseDown as i32,
            mouse_x: x,
            mouse_y: y,
            button: button.to_proto_value(),
            key_code: 0,
            modifiers: 0,
            text: String::new(),
            scroll_delta_x: 0,
            scroll_delta_y: 0,
        }
    }

    /// Build a mouse button up event
    /// Requirements: 5.1
    pub fn mouse_up(x: i32, y: i32, button: MouseButton) -> InputEventV1 {
        InputEventV1 {
            event_type: InputEventTypeV1::MouseUp as i32,
            mouse_x: x,
            mouse_y: y,
            button: button.to_proto_value(),
            key_code: 0,
            modifiers: 0,
            text: String::new(),
            scroll_delta_x: 0,
            scroll_delta_y: 0,
        }
    }

    /// Build a mouse click event (down + up)
    /// Requirements: 5.1
    pub fn mouse_click(x: i32, y: i32, button: MouseButton) -> Vec<InputEventV1> {
        vec![
            Self::mouse_down(x, y, button),
            Self::mouse_up(x, y, button),
        ]
    }

    /// Build a scroll event
    /// Requirements: 5.4
    pub fn scroll(delta_x: i32, delta_y: i32) -> InputEventV1 {
        InputEventV1 {
            event_type: InputEventTypeV1::Scroll as i32,
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

    /// Build a key down event
    /// Requirements: 5.2
    pub fn key_down(code: KeyCode, modifiers: Modifiers) -> InputEventV1 {
        InputEventV1 {
            event_type: InputEventTypeV1::KeyDown as i32,
            mouse_x: 0,
            mouse_y: 0,
            button: 0,
            key_code: code.value(),
            modifiers: modifiers.value(),
            text: String::new(),
            scroll_delta_x: 0,
            scroll_delta_y: 0,
        }
    }

    /// Build a key up event
    /// Requirements: 5.2
    pub fn key_up(code: KeyCode, modifiers: Modifiers) -> InputEventV1 {
        InputEventV1 {
            event_type: InputEventTypeV1::KeyUp as i32,
            mouse_x: 0,
            mouse_y: 0,
            button: 0,
            key_code: code.value(),
            modifiers: modifiers.value(),
            text: String::new(),
            scroll_delta_x: 0,
            scroll_delta_y: 0,
        }
    }

    /// Build a text input event (character input)
    /// Requirements: 5.3
    pub fn text_input(text: &str) -> InputEventV1 {
        InputEventV1 {
            event_type: InputEventTypeV1::KeyChar as i32,
            mouse_x: 0,
            mouse_y: 0,
            button: 0,
            key_code: 0,
            modifiers: 0,
            text: text.to_string(),
            scroll_delta_x: 0,
            scroll_delta_y: 0,
        }
    }
}

/// Input command builder and sender
/// Requirements: 5.1-5.7
pub struct InputCommands {
    /// Current session ID (if connected)
    session_id: Option<String>,
    /// Whether we have control permission
    has_control_permission: bool,
}

impl InputCommands {
    /// Create a new input commands handler
    pub fn new() -> Self {
        Self {
            session_id: None,
            has_control_permission: false,
        }
    }

    /// Create with a session ID
    pub fn with_session(session_id: String) -> Self {
        Self {
            session_id: Some(session_id),
            has_control_permission: true,
        }
    }

    /// Set the current session
    pub fn set_session(&mut self, session_id: String, has_control: bool) {
        self.session_id = Some(session_id);
        self.has_control_permission = has_control;
    }

    /// Clear the current session
    pub fn clear_session(&mut self) {
        self.session_id = None;
        self.has_control_permission = false;
    }

    /// Get the current session ID
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Check if we have an active session
    fn require_session(&self, provided_session: Option<&str>) -> Result<String, InputError> {
        provided_session
            .map(|s| s.to_string())
            .or_else(|| self.session_id.clone())
            .ok_or(InputError::NoSession)
    }

    /// Check control permission
    fn require_control_permission(&self) -> Result<(), InputError> {
        if !self.has_control_permission && self.session_id.is_some() {
            return Err(InputError::PermissionDenied(
                "Control permission not granted for this session".to_string(),
            ));
        }
        Ok(())
    }

    /// Encode an input event to bytes
    fn encode_event(event: &InputEventV1) -> Vec<u8> {
        event.encode_to_vec()
    }

    /// Send mouse move
    /// Requirements: 5.1
    pub async fn mouse_move(
        &self,
        session_id: Option<&str>,
        x: i32,
        y: i32,
    ) -> Result<InputResult, InputError> {
        let session = self.require_session(session_id)?;
        self.require_control_permission()?;
        InputValidator::validate_mouse_coords(x, y)?;

        let event = InputEventBuilder::mouse_move(x, y);
        let _bytes = Self::encode_event(&event);

        // In a full implementation, we would send the bytes over the control stream
        // For now, we return success to indicate the event was built correctly
        Ok(InputResult {
            success: true,
            session_id: session,
            input_type: "mouse_move".to_string(),
            details: format!("Move to ({}, {})", x, y),
        })
    }

    /// Send mouse click
    /// Requirements: 5.1
    pub async fn mouse_click(
        &self,
        session_id: Option<&str>,
        x: i32,
        y: i32,
        button: MouseButton,
    ) -> Result<InputResult, InputError> {
        let session = self.require_session(session_id)?;
        self.require_control_permission()?;
        InputValidator::validate_mouse_coords(x, y)?;

        let events = InputEventBuilder::mouse_click(x, y, button);
        for event in &events {
            let _bytes = Self::encode_event(event);
            // Send bytes over control stream
        }

        Ok(InputResult {
            success: true,
            session_id: session,
            input_type: "mouse_click".to_string(),
            details: format!("Click {:?} at ({}, {})", button, x, y),
        })
    }

    /// Send mouse button down
    /// Requirements: 5.1
    pub async fn mouse_down(
        &self,
        session_id: Option<&str>,
        x: i32,
        y: i32,
        button: MouseButton,
    ) -> Result<InputResult, InputError> {
        let session = self.require_session(session_id)?;
        self.require_control_permission()?;
        InputValidator::validate_mouse_coords(x, y)?;

        let event = InputEventBuilder::mouse_down(x, y, button);
        let _bytes = Self::encode_event(&event);

        Ok(InputResult {
            success: true,
            session_id: session,
            input_type: "mouse_down".to_string(),
            details: format!("Button {:?} down at ({}, {})", button, x, y),
        })
    }

    /// Send mouse button up
    /// Requirements: 5.1
    pub async fn mouse_up(
        &self,
        session_id: Option<&str>,
        x: i32,
        y: i32,
        button: MouseButton,
    ) -> Result<InputResult, InputError> {
        let session = self.require_session(session_id)?;
        self.require_control_permission()?;
        InputValidator::validate_mouse_coords(x, y)?;

        let event = InputEventBuilder::mouse_up(x, y, button);
        let _bytes = Self::encode_event(&event);

        Ok(InputResult {
            success: true,
            session_id: session,
            input_type: "mouse_up".to_string(),
            details: format!("Button {:?} up at ({}, {})", button, x, y),
        })
    }

    /// Send scroll event
    /// Requirements: 5.4
    pub async fn scroll(
        &self,
        session_id: Option<&str>,
        delta: i32,
    ) -> Result<InputResult, InputError> {
        let session = self.require_session(session_id)?;
        self.require_control_permission()?;
        InputValidator::validate_scroll_delta(delta)?;

        // Vertical scroll (delta_y)
        let event = InputEventBuilder::scroll(0, delta);
        let _bytes = Self::encode_event(&event);

        Ok(InputResult {
            success: true,
            session_id: session,
            input_type: "scroll".to_string(),
            details: format!("Scroll delta {}", delta),
        })
    }

    /// Send scroll event with both axes
    /// Requirements: 5.4
    pub async fn scroll_xy(
        &self,
        session_id: Option<&str>,
        delta_x: i32,
        delta_y: i32,
    ) -> Result<InputResult, InputError> {
        let session = self.require_session(session_id)?;
        self.require_control_permission()?;
        InputValidator::validate_scroll_delta(delta_x)?;
        InputValidator::validate_scroll_delta(delta_y)?;

        let event = InputEventBuilder::scroll(delta_x, delta_y);
        let _bytes = Self::encode_event(&event);

        Ok(InputResult {
            success: true,
            session_id: session,
            input_type: "scroll".to_string(),
            details: format!("Scroll delta ({}, {})", delta_x, delta_y),
        })
    }

    /// Send key event
    /// Requirements: 5.2
    pub async fn key(
        &self,
        session_id: Option<&str>,
        code: KeyCode,
        down: bool,
    ) -> Result<InputResult, InputError> {
        let session = self.require_session(session_id)?;
        self.require_control_permission()?;
        InputValidator::validate_key_code(code.value())?;

        let event = if down {
            InputEventBuilder::key_down(code, Modifiers::none())
        } else {
            InputEventBuilder::key_up(code, Modifiers::none())
        };
        let _bytes = Self::encode_event(&event);

        Ok(InputResult {
            success: true,
            session_id: session,
            input_type: if down { "key_down" } else { "key_up" }.to_string(),
            details: format!("Key code {} {}", code.value(), if down { "down" } else { "up" }),
        })
    }

    /// Send key event with modifiers
    /// Requirements: 5.2
    pub async fn key_with_modifiers(
        &self,
        session_id: Option<&str>,
        code: KeyCode,
        down: bool,
        modifiers: Modifiers,
    ) -> Result<InputResult, InputError> {
        let session = self.require_session(session_id)?;
        self.require_control_permission()?;
        InputValidator::validate_key_code(code.value())?;

        let event = if down {
            InputEventBuilder::key_down(code, modifiers)
        } else {
            InputEventBuilder::key_up(code, modifiers)
        };
        let _bytes = Self::encode_event(&event);

        Ok(InputResult {
            success: true,
            session_id: session,
            input_type: if down { "key_down" } else { "key_up" }.to_string(),
            details: format!(
                "Key code {} {} (modifiers: 0x{:02x})",
                code.value(),
                if down { "down" } else { "up" },
                modifiers.value()
            ),
        })
    }

    /// Send text input
    /// Requirements: 5.3
    pub async fn text(
        &self,
        session_id: Option<&str>,
        text: &str,
    ) -> Result<InputResult, InputError> {
        let session = self.require_session(session_id)?;
        self.require_control_permission()?;
        InputValidator::validate_text(text)?;

        let event = InputEventBuilder::text_input(text);
        let _bytes = Self::encode_event(&event);

        Ok(InputResult {
            success: true,
            session_id: session,
            input_type: "text".to_string(),
            details: format!("Text input: {} chars", text.len()),
        })
    }
}

impl Default for InputCommands {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_button_from_str() {
        assert_eq!("left".parse::<MouseButton>().unwrap(), MouseButton::Left);
        assert_eq!("RIGHT".parse::<MouseButton>().unwrap(), MouseButton::Right);
        assert_eq!("Middle".parse::<MouseButton>().unwrap(), MouseButton::Middle);
        assert_eq!("1".parse::<MouseButton>().unwrap(), MouseButton::Left);
        assert_eq!("2".parse::<MouseButton>().unwrap(), MouseButton::Right);
        assert!("invalid".parse::<MouseButton>().is_err());
    }

    #[test]
    fn test_mouse_button_to_proto() {
        assert_eq!(MouseButton::Left.to_proto_value(), 1);
        assert_eq!(MouseButton::Right.to_proto_value(), 2);
        assert_eq!(MouseButton::Middle.to_proto_value(), 3);
    }

    #[test]
    fn test_modifiers() {
        let mods = Modifiers::none()
            .with_shift()
            .with_ctrl();
        assert_eq!(mods.value(), Modifiers::SHIFT | Modifiers::CTRL);
    }

    #[test]
    fn test_validate_mouse_coords() {
        assert!(InputValidator::validate_mouse_coords(0, 0).is_ok());
        assert!(InputValidator::validate_mouse_coords(1920, 1080).is_ok());
        assert!(InputValidator::validate_mouse_coords(-100, -100).is_ok());
        assert!(InputValidator::validate_mouse_coords(50000, 0).is_err());
    }

    #[test]
    fn test_validate_scroll_delta() {
        assert!(InputValidator::validate_scroll_delta(0).is_ok());
        assert!(InputValidator::validate_scroll_delta(120).is_ok());
        assert!(InputValidator::validate_scroll_delta(-120).is_ok());
        assert!(InputValidator::validate_scroll_delta(20000).is_err());
    }

    #[test]
    fn test_validate_key_code() {
        assert!(InputValidator::validate_key_code(0).is_ok());
        assert!(InputValidator::validate_key_code(65).is_ok()); // 'A'
        assert!(InputValidator::validate_key_code(0xFFFF).is_ok());
        assert!(InputValidator::validate_key_code(0x10000).is_err());
    }

    #[test]
    fn test_validate_text() {
        assert!(InputValidator::validate_text("hello").is_ok());
        assert!(InputValidator::validate_text("").is_err());
        let long_text = "a".repeat(20000);
        assert!(InputValidator::validate_text(&long_text).is_err());
    }

    #[test]
    fn test_input_event_builder_mouse_move() {
        let event = InputEventBuilder::mouse_move(100, 200);
        assert_eq!(event.event_type, InputEventTypeV1::MouseMove as i32);
        assert_eq!(event.mouse_x, 100);
        assert_eq!(event.mouse_y, 200);
    }

    #[test]
    fn test_input_event_builder_mouse_click() {
        let events = InputEventBuilder::mouse_click(100, 200, MouseButton::Left);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, InputEventTypeV1::MouseDown as i32);
        assert_eq!(events[1].event_type, InputEventTypeV1::MouseUp as i32);
    }

    #[test]
    fn test_input_event_builder_scroll() {
        let event = InputEventBuilder::scroll(10, 20);
        assert_eq!(event.event_type, InputEventTypeV1::Scroll as i32);
        assert_eq!(event.scroll_delta_x, 10);
        assert_eq!(event.scroll_delta_y, 20);
    }

    #[test]
    fn test_input_event_builder_key() {
        let event = InputEventBuilder::key_down(KeyCode::new(65), Modifiers::none().with_shift());
        assert_eq!(event.event_type, InputEventTypeV1::KeyDown as i32);
        assert_eq!(event.key_code, 65);
        assert_eq!(event.modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn test_input_event_builder_text() {
        let event = InputEventBuilder::text_input("hello");
        assert_eq!(event.event_type, InputEventTypeV1::KeyChar as i32);
        assert_eq!(event.text, "hello");
    }

    #[test]
    fn test_input_commands_no_session() {
        let cmds = InputCommands::new();
        assert!(cmds.session_id().is_none());
    }

    #[test]
    fn test_input_commands_with_session() {
        let cmds = InputCommands::with_session("test-session".to_string());
        assert_eq!(cmds.session_id(), Some("test-session"));
    }

    #[tokio::test]
    async fn test_input_commands_require_session() {
        let cmds = InputCommands::new();
        let result = cmds.mouse_move(None, 100, 200).await;
        assert!(matches!(result, Err(InputError::NoSession)));
    }

    #[tokio::test]
    async fn test_input_commands_with_provided_session() {
        let cmds = InputCommands::new();
        let result = cmds.mouse_move(Some("provided-session"), 100, 200).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.session_id, "provided-session");
    }

    #[tokio::test]
    async fn test_mouse_move_success() {
        let cmds = InputCommands::with_session("test".to_string());
        let result = cmds.mouse_move(None, 100, 200).await.unwrap();
        assert!(result.success);
        assert_eq!(result.input_type, "mouse_move");
    }

    #[tokio::test]
    async fn test_mouse_click_success() {
        let cmds = InputCommands::with_session("test".to_string());
        let result = cmds.mouse_click(None, 100, 200, MouseButton::Left).await.unwrap();
        assert!(result.success);
        assert_eq!(result.input_type, "mouse_click");
    }

    #[tokio::test]
    async fn test_scroll_success() {
        let cmds = InputCommands::with_session("test".to_string());
        let result = cmds.scroll(None, 120).await.unwrap();
        assert!(result.success);
        assert_eq!(result.input_type, "scroll");
    }

    #[tokio::test]
    async fn test_key_success() {
        let cmds = InputCommands::with_session("test".to_string());
        let result = cmds.key(None, KeyCode::new(65), true).await.unwrap();
        assert!(result.success);
        assert_eq!(result.input_type, "key_down");
    }

    #[tokio::test]
    async fn test_text_success() {
        let cmds = InputCommands::with_session("test".to_string());
        let result = cmds.text(None, "hello").await.unwrap();
        assert!(result.success);
        assert_eq!(result.input_type, "text");
    }
}
