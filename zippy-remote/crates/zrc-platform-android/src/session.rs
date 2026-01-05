//! Session management for Android

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::error::ZrcError;

/// Session handle
pub struct Session {
    session_id: u64,
    device_id: Vec<u8>,
}

impl Session {
    /// Create a new session
    pub fn new(session_id: u64, device_id: Vec<u8>) -> Self {
        Self {
            session_id,
            device_id,
        }
    }

    /// Get session ID
    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    /// Get device ID
    pub fn device_id(&self) -> &[u8] {
        &self.device_id
    }
}
