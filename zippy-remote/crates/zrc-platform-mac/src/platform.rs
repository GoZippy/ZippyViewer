#![cfg(target_os = "macos")]

use std::sync::Arc;
use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::Mutex;

use zrc_core::platform::{HostPlatform, InputEvent};

use crate::capturer::MacCapturer;
use crate::injector::MacInjector;

/// Thread-safe wrapper for MacCapturer
struct SendCapturer(MacCapturer);

// Safety: MacCapturer operations are performed synchronously within mutex locks
// and don't hold macOS handles across await points
unsafe impl Send for SendCapturer {}

/// macOS platform implementation
pub struct MacPlatform {
    capturer: Arc<Mutex<SendCapturer>>,
    injector: Arc<Mutex<MacInjector>>,
}

// Safety: MacPlatform uses Mutex for interior mutability and all operations
// are performed synchronously within the lock
unsafe impl Send for MacPlatform {}
unsafe impl Sync for MacPlatform {}

impl MacPlatform {
    /// Create new macOS platform instance
    pub fn new() -> anyhow::Result<Self> {
        let capturer = Arc::new(Mutex::new(SendCapturer(MacCapturer::new()
            .map_err(|e| anyhow::anyhow!("capturer init failed: {e}"))?)));
        let injector = Arc::new(Mutex::new(MacInjector::new()
            .map_err(|e| anyhow::anyhow!("injector init failed: {e}"))?));

        Ok(Self {
            capturer,
            injector,
        })
    }
}

#[async_trait]
impl HostPlatform for MacPlatform {
    async fn capture_frame(&self) -> anyhow::Result<Bytes> {
        let mut capturer = self.capturer.lock().await;
        let frame = capturer.0.capture_frame()
            .map_err(|e| anyhow::anyhow!("capture failed: {e}"))?;
        
        Ok(Bytes::from(frame))
    }

    async fn apply_input(&self, evt: InputEvent) -> anyhow::Result<()> {
        let mut injector = self.injector.lock().await;
        
        match evt {
            InputEvent::MouseMove { x, y } => {
                injector.inject_mouse_move(x, y)
                    .map_err(|e| anyhow::anyhow!("mouse move failed: {e}"))?;
            }
            InputEvent::MouseButton { button, down } => {
                injector.inject_mouse_button(button, down, 0, 0) // TODO: Get current mouse position
                    .map_err(|e| anyhow::anyhow!("mouse button failed: {e}"))?;
            }
            InputEvent::Key { keycode, down } => {
                injector.inject_key(keycode, down)
                    .map_err(|e| anyhow::anyhow!("key injection failed: {e}"))?;
            }
            InputEvent::Text(text) => {
                injector.inject_text(&text)
                    .map_err(|e| anyhow::anyhow!("text injection failed: {e}"))?;
            }
        }
        
        Ok(())
    }

    async fn set_clipboard(&self, data: Bytes) -> anyhow::Result<()> {
        let clipboard = crate::clipboard::MacClipboard::new()
            .map_err(|e| anyhow::anyhow!("clipboard init failed: {e}"))?;
        
        // Try to decode as text first, then as image
        if let Ok(text) = String::from_utf8(data.to_vec()) {
            clipboard.write_text(&text)
                .map_err(|e| anyhow::anyhow!("clipboard write failed: {e}"))?;
        } else {
            // Assume it's image data
            clipboard.write_image(&data)
                .map_err(|e| anyhow::anyhow!("clipboard write failed: {e}"))?;
        }
        
        Ok(())
    }

    async fn get_clipboard(&self) -> anyhow::Result<Bytes> {
        let clipboard = crate::clipboard::MacClipboard::new()
            .map_err(|e| anyhow::anyhow!("clipboard init failed: {e}"))?;
        
        // Try text first
        if let Some(text) = clipboard.read_text()
            .map_err(|e| anyhow::anyhow!("clipboard read failed: {e}"))? {
            return Ok(Bytes::from(text));
        }
        
        // Try image
        if let Some(image) = clipboard.read_image()
            .map_err(|e| anyhow::anyhow!("clipboard read failed: {e}"))? {
            return Ok(image);
        }
        
        Ok(Bytes::new())
    }
}
