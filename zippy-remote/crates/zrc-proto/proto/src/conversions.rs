//! Conversion traits between proto types and domain types.
//!
//! This module provides From/Into implementations for converting between
//! protocol buffer types and common Rust types used throughout the ZRC system.
//!
//! Requirements: 10.3

use crate::v1::*;

// ============================================================================
// Identity conversions
// ============================================================================

impl From<[u8; 32]> for DeviceIdV1 {
    fn from(id: [u8; 32]) -> Self {
        Self { id: id.to_vec() }
    }
}

impl From<&[u8; 32]> for DeviceIdV1 {
    fn from(id: &[u8; 32]) -> Self {
        Self { id: id.to_vec() }
    }
}

impl TryFrom<DeviceIdV1> for [u8; 32] {
    type Error = &'static str;

    fn try_from(value: DeviceIdV1) -> Result<Self, Self::Error> {
        value.id.try_into().map_err(|_| "DeviceIdV1.id must be exactly 32 bytes")
    }
}

impl TryFrom<&DeviceIdV1> for [u8; 32] {
    type Error = &'static str;

    fn try_from(value: &DeviceIdV1) -> Result<Self, Self::Error> {
        value.id.as_slice().try_into().map_err(|_| "DeviceIdV1.id must be exactly 32 bytes")
    }
}

impl From<[u8; 32]> for OperatorIdV1 {
    fn from(id: [u8; 32]) -> Self {
        Self { id: id.to_vec() }
    }
}

impl From<&[u8; 32]> for OperatorIdV1 {
    fn from(id: &[u8; 32]) -> Self {
        Self { id: id.to_vec() }
    }
}

impl TryFrom<OperatorIdV1> for [u8; 32] {
    type Error = &'static str;

    fn try_from(value: OperatorIdV1) -> Result<Self, Self::Error> {
        value.id.try_into().map_err(|_| "OperatorIdV1.id must be exactly 32 bytes")
    }
}

impl TryFrom<&OperatorIdV1> for [u8; 32] {
    type Error = &'static str;

    fn try_from(value: &OperatorIdV1) -> Result<Self, Self::Error> {
        value.id.as_slice().try_into().map_err(|_| "OperatorIdV1.id must be exactly 32 bytes")
    }
}

impl From<[u8; 32]> for SessionIdV1 {
    fn from(id: [u8; 32]) -> Self {
        Self { id: id.to_vec() }
    }
}

impl From<[u8; 16]> for SessionIdV1 {
    fn from(id: [u8; 16]) -> Self {
        Self { id: id.to_vec() }
    }
}

impl From<[u8; 16]> for TicketIdV1 {
    fn from(id: [u8; 16]) -> Self {
        Self { id: id.to_vec() }
    }
}

impl TryFrom<TicketIdV1> for [u8; 16] {
    type Error = &'static str;

    fn try_from(value: TicketIdV1) -> Result<Self, Self::Error> {
        value.id.try_into().map_err(|_| "TicketIdV1.id must be exactly 16 bytes")
    }
}

// ============================================================================
// Timestamp conversions
// ============================================================================

impl From<u64> for TimestampV1 {
    fn from(unix_seconds: u64) -> Self {
        Self { unix_seconds }
    }
}

impl From<TimestampV1> for u64 {
    fn from(ts: TimestampV1) -> Self {
        ts.unix_seconds
    }
}

impl From<&TimestampV1> for u64 {
    fn from(ts: &TimestampV1) -> Self {
        ts.unix_seconds
    }
}

impl From<std::time::SystemTime> for TimestampV1 {
    fn from(time: std::time::SystemTime) -> Self {
        let unix_seconds = time
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self { unix_seconds }
    }
}

impl TryFrom<TimestampV1> for std::time::SystemTime {
    type Error = &'static str;

    fn try_from(ts: TimestampV1) -> Result<Self, Self::Error> {
        std::time::UNIX_EPOCH
            .checked_add(std::time::Duration::from_secs(ts.unix_seconds))
            .ok_or("timestamp overflow")
    }
}

// ============================================================================
// Duration conversions
// ============================================================================

impl From<u64> for DurationV1 {
    fn from(seconds: u64) -> Self {
        Self { seconds }
    }
}

impl From<DurationV1> for u64 {
    fn from(d: DurationV1) -> Self {
        d.seconds
    }
}

impl From<std::time::Duration> for DurationV1 {
    fn from(d: std::time::Duration) -> Self {
        Self { seconds: d.as_secs() }
    }
}

impl From<DurationV1> for std::time::Duration {
    fn from(d: DurationV1) -> Self {
        std::time::Duration::from_secs(d.seconds)
    }
}

// ============================================================================
// Public key conversions
// ============================================================================

impl PublicKeyV1 {
    /// Create an Ed25519 public key from raw bytes.
    pub fn ed25519(key_bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: key_bytes.into(),
        }
    }

    /// Create an X25519 public key from raw bytes.
    pub fn x25519(key_bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: key_bytes.into(),
        }
    }

    /// Check if this is an Ed25519 key.
    pub fn is_ed25519(&self) -> bool {
        self.key_type == KeyTypeV1::Ed25519 as i32
    }

    /// Check if this is an X25519 key.
    pub fn is_x25519(&self) -> bool {
        self.key_type == KeyTypeV1::X25519 as i32
    }

    /// Get the key type as an enum.
    pub fn key_type_enum(&self) -> KeyTypeV1 {
        KeyTypeV1::try_from(self.key_type).unwrap_or(KeyTypeV1::Unspecified)
    }
}

impl PublicKeyBundleV1 {
    /// Create a new public key bundle from raw bytes.
    pub fn new(sign_pub: impl Into<Vec<u8>>, kex_pub: impl Into<Vec<u8>>) -> Self {
        Self {
            sign_pub: sign_pub.into(),
            kex_pub: kex_pub.into(),
        }
    }

    /// Create from fixed-size arrays.
    pub fn from_arrays(sign_pub: [u8; 32], kex_pub: [u8; 32]) -> Self {
        Self {
            sign_pub: sign_pub.to_vec(),
            kex_pub: kex_pub.to_vec(),
        }
    }
}

// ============================================================================
// Signature conversions
// ============================================================================

impl SignatureV1 {
    /// Create an Ed25519 signature from raw bytes.
    pub fn ed25519(sig_bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            sig_type: SigTypeV1::Ed25519 as i32,
            sig_bytes: sig_bytes.into(),
        }
    }

    /// Check if this is an Ed25519 signature.
    pub fn is_ed25519(&self) -> bool {
        self.sig_type == SigTypeV1::Ed25519 as i32
    }

    /// Get the signature type as an enum.
    pub fn sig_type_enum(&self) -> SigTypeV1 {
        SigTypeV1::try_from(self.sig_type).unwrap_or(SigTypeV1::Unspecified)
    }
}

// ============================================================================
// Permission conversions
// ============================================================================

/// Bitflags-style wrapper for permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Permissions(pub u32);

impl Permissions {
    pub const NONE: Self = Self(0);
    pub const VIEW: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const CLIPBOARD: Self = Self(1 << 2);
    pub const FILE_TRANSFER: Self = Self(1 << 3);
    pub const AUDIO: Self = Self(1 << 4);
    pub const UNATTENDED: Self = Self(1 << 5);

    /// Check if a permission is set.
    pub fn has(self, perm: Self) -> bool {
        (self.0 & perm.0) == perm.0
    }

    /// Add a permission.
    pub fn with(self, perm: Self) -> Self {
        Self(self.0 | perm.0)
    }

    /// Remove a permission.
    pub fn without(self, perm: Self) -> Self {
        Self(self.0 & !perm.0)
    }

    /// Create from a list of PermissionV1 values.
    pub fn from_permission_list(perms: &[i32]) -> Self {
        let mut result = 0u32;
        for &p in perms {
            if let Ok(perm) = PermissionV1::try_from(p) {
                result |= match perm {
                    PermissionV1::View => Self::VIEW.0,
                    PermissionV1::Control => Self::CONTROL.0,
                    PermissionV1::Clipboard => Self::CLIPBOARD.0,
                    PermissionV1::Files => Self::FILE_TRANSFER.0,
                    PermissionV1::Audio => Self::AUDIO.0,
                    PermissionV1::Recording => Self::UNATTENDED.0,
                    _ => 0,
                };
            }
        }
        Self(result)
    }

    /// Convert to a list of PermissionV1 values.
    pub fn to_permission_list(self) -> Vec<PermissionV1> {
        let mut result = Vec::new();
        if self.has(Self::VIEW) {
            result.push(PermissionV1::View);
        }
        if self.has(Self::CONTROL) {
            result.push(PermissionV1::Control);
        }
        if self.has(Self::CLIPBOARD) {
            result.push(PermissionV1::Clipboard);
        }
        if self.has(Self::FILE_TRANSFER) {
            result.push(PermissionV1::Files);
        }
        if self.has(Self::AUDIO) {
            result.push(PermissionV1::Audio);
        }
        if self.has(Self::UNATTENDED) {
            result.push(PermissionV1::Recording);
        }
        result
    }
}

impl From<u32> for Permissions {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Permissions> for u32 {
    fn from(value: Permissions) -> Self {
        value.0
    }
}

// ============================================================================
// Error conversions
// ============================================================================

impl ErrorV1 {
    /// Create a new error with the given code and message.
    pub fn new(code: ErrorCodeV1, message: impl Into<String>) -> Self {
        Self {
            error_code: code as i32,
            error_message: message.into(),
            details: Default::default(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    /// Create an authentication failed error.
    pub fn auth_failed(message: impl Into<String>) -> Self {
        Self::new(ErrorCodeV1::AuthFailed, message)
    }

    /// Create a permission denied error.
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(ErrorCodeV1::PermissionDenied, message)
    }

    /// Create a ticket expired error.
    pub fn ticket_expired(message: impl Into<String>) -> Self {
        Self::new(ErrorCodeV1::TicketExpired, message)
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCodeV1::InternalError, message)
    }

    /// Create an invalid message error.
    pub fn invalid_message(message: impl Into<String>) -> Self {
        Self::new(ErrorCodeV1::InvalidMessage, message)
    }

    /// Add a detail to the error.
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    /// Get the error code as an enum.
    pub fn error_code_enum(&self) -> ErrorCodeV1 {
        ErrorCodeV1::try_from(self.error_code).unwrap_or(ErrorCodeV1::Unspecified)
    }
}

// ============================================================================
// Input event conversions
// ============================================================================

impl InputEventV1 {
    /// Create a mouse move event.
    pub fn mouse_move(x: i32, y: i32) -> Self {
        Self {
            event_type: InputEventTypeV1::MouseMove as i32,
            mouse_x: x,
            mouse_y: y,
            ..Default::default()
        }
    }

    /// Create a mouse button down event.
    pub fn mouse_down(x: i32, y: i32, button: u32) -> Self {
        Self {
            event_type: InputEventTypeV1::MouseDown as i32,
            mouse_x: x,
            mouse_y: y,
            button,
            ..Default::default()
        }
    }

    /// Create a mouse button up event.
    pub fn mouse_up(x: i32, y: i32, button: u32) -> Self {
        Self {
            event_type: InputEventTypeV1::MouseUp as i32,
            mouse_x: x,
            mouse_y: y,
            button,
            ..Default::default()
        }
    }

    /// Create a key down event.
    pub fn key_down(key_code: u32, modifiers: u32) -> Self {
        Self {
            event_type: InputEventTypeV1::KeyDown as i32,
            key_code,
            modifiers,
            ..Default::default()
        }
    }

    /// Create a key up event.
    pub fn key_up(key_code: u32, modifiers: u32) -> Self {
        Self {
            event_type: InputEventTypeV1::KeyUp as i32,
            key_code,
            modifiers,
            ..Default::default()
        }
    }

    /// Create a character input event.
    pub fn key_char(text: impl Into<String>) -> Self {
        Self {
            event_type: InputEventTypeV1::KeyChar as i32,
            text: text.into(),
            ..Default::default()
        }
    }

    /// Create a scroll event.
    pub fn scroll(delta_x: i32, delta_y: i32) -> Self {
        Self {
            event_type: InputEventTypeV1::Scroll as i32,
            scroll_delta_x: delta_x,
            scroll_delta_y: delta_y,
            ..Default::default()
        }
    }

    /// Get the event type as an enum.
    pub fn event_type_enum(&self) -> InputEventTypeV1 {
        InputEventTypeV1::try_from(self.event_type).unwrap_or(InputEventTypeV1::Unspecified)
    }
}

// ============================================================================
// Control message conversions
// ============================================================================

impl ControlMsgV1 {
    /// Create a new control message with the given payload.
    pub fn new(sequence_number: u64, payload: control_msg_v1::Payload) -> Self {
        let msg_type = match &payload {
            control_msg_v1::Payload::Input(_) => ControlMsgTypeV1::Input,
            control_msg_v1::Payload::Clipboard(_) => ControlMsgTypeV1::Clipboard,
            control_msg_v1::Payload::FrameMeta(_) => ControlMsgTypeV1::FrameMeta,
            control_msg_v1::Payload::FileControl(_) => ControlMsgTypeV1::FileControl,
            control_msg_v1::Payload::SessionControl(_) => ControlMsgTypeV1::SessionControl,
            control_msg_v1::Payload::Ping(_) => ControlMsgTypeV1::Ping,
            control_msg_v1::Payload::Pong(_) => ControlMsgTypeV1::Pong,
        };

        Self {
            msg_type: msg_type as i32,
            sequence_number,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0),
            payload: Some(payload),
        }
    }

    /// Create an input control message.
    pub fn input(sequence_number: u64, event: InputEventV1) -> Self {
        Self::new(sequence_number, control_msg_v1::Payload::Input(event))
    }

    /// Create a ping control message.
    pub fn ping(sequence_number: u64) -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        Self::new(sequence_number, control_msg_v1::Payload::Ping(PingV1 { t }))
    }

    /// Create a pong control message.
    pub fn pong(sequence_number: u64, ping_timestamp: u64) -> Self {
        Self::new(sequence_number, control_msg_v1::Payload::Pong(PongV1 { t: ping_timestamp }))
    }

    /// Get the message type as an enum.
    pub fn msg_type_enum(&self) -> ControlMsgTypeV1 {
        ControlMsgTypeV1::try_from(self.msg_type).unwrap_or(ControlMsgTypeV1::Unspecified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_conversion() {
        let id: [u8; 32] = [1u8; 32];
        let device_id: DeviceIdV1 = id.into();
        assert_eq!(device_id.id.len(), 32);

        let back: [u8; 32] = device_id.try_into().unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn test_timestamp_conversion() {
        let ts = TimestampV1::from(1234567890u64);
        assert_eq!(ts.unix_seconds, 1234567890);

        let back: u64 = ts.into();
        assert_eq!(back, 1234567890);
    }

    #[test]
    fn test_system_time_conversion() {
        let now = std::time::SystemTime::now();
        let ts = TimestampV1::from(now);
        let back: std::time::SystemTime = ts.try_into().unwrap();
        
        // Should be within 1 second
        let diff = now.duration_since(back).unwrap_or_default();
        assert!(diff.as_secs() < 1);
    }

    #[test]
    fn test_permissions() {
        let perms = Permissions::NONE
            .with(Permissions::VIEW)
            .with(Permissions::CONTROL);
        
        assert!(perms.has(Permissions::VIEW));
        assert!(perms.has(Permissions::CONTROL));
        assert!(!perms.has(Permissions::CLIPBOARD));

        let without = perms.without(Permissions::CONTROL);
        assert!(without.has(Permissions::VIEW));
        assert!(!without.has(Permissions::CONTROL));
    }

    #[test]
    fn test_public_key_helpers() {
        let key = PublicKeyV1::ed25519(vec![0u8; 32]);
        assert!(key.is_ed25519());
        assert!(!key.is_x25519());

        let key = PublicKeyV1::x25519(vec![0u8; 32]);
        assert!(!key.is_ed25519());
        assert!(key.is_x25519());
    }

    #[test]
    fn test_error_helpers() {
        let err = ErrorV1::auth_failed("bad credentials")
            .with_detail("user", "test@example.com");
        
        assert_eq!(err.error_code_enum(), ErrorCodeV1::AuthFailed);
        assert_eq!(err.error_message, "bad credentials");
        assert_eq!(err.details.get("user"), Some(&"test@example.com".to_string()));
    }

    #[test]
    fn test_input_event_helpers() {
        let move_event = InputEventV1::mouse_move(100, 200);
        assert_eq!(move_event.event_type_enum(), InputEventTypeV1::MouseMove);
        assert_eq!(move_event.mouse_x, 100);
        assert_eq!(move_event.mouse_y, 200);

        let key_event = InputEventV1::key_down(65, 0);
        assert_eq!(key_event.event_type_enum(), InputEventTypeV1::KeyDown);
        assert_eq!(key_event.key_code, 65);
    }

    #[test]
    fn test_control_msg_helpers() {
        let msg = ControlMsgV1::input(1, InputEventV1::mouse_move(0, 0));
        assert_eq!(msg.msg_type_enum(), ControlMsgTypeV1::Input);
        assert_eq!(msg.sequence_number, 1);

        let ping = ControlMsgV1::ping(2);
        assert_eq!(ping.msg_type_enum(), ControlMsgTypeV1::Ping);
    }
}
