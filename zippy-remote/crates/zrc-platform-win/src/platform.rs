#![cfg(windows)]

use std::sync::Arc;
use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::Mutex;

use zrc_core::platform::{HostPlatform, InputEvent};

use crate::capturer::WinCapturer;
use crate::injector::WinInjector;

/// Thread-safe wrapper for WinCapturer
struct SendCapturer(WinCapturer);

// Safety: WinCapturer operations are performed synchronously within mutex locks
// and don't hold Windows handles across await points
unsafe impl Send for SendCapturer {}

/// Windows platform implementation
pub struct WinPlatform {
    capturer: Arc<Mutex<SendCapturer>>,
    injector: Arc<Mutex<WinInjector>>,
}

// Safety: WinPlatform uses Mutex for interior mutability and all operations
// are performed synchronously within the lock
unsafe impl Send for WinPlatform {}
unsafe impl Sync for WinPlatform {}

impl WinPlatform {
    /// Create new Windows platform instance
    pub fn new() -> anyhow::Result<Self> {
        let capturer = Arc::new(Mutex::new(SendCapturer(WinCapturer::new()?)));
        let injector = Arc::new(Mutex::new(WinInjector::new()));

        Ok(Self {
            capturer,
            injector,
        })
    }
}

#[async_trait]
impl HostPlatform for WinPlatform {
    async fn capture_frame(&self) -> anyhow::Result<Bytes> {
        let mut capturer = self.capturer.lock().await;
        let frame = capturer.0.capture_frame()
            .map_err(|e| anyhow::anyhow!("capture failed: {e}"))?;
        
        // Convert BGRA frame to bytes
        Ok(Bytes::from(frame.bgra))
    }

    async fn apply_input(&self, evt: InputEvent) -> anyhow::Result<()> {
        let mut injector = self.injector.lock().await;
        
        match evt {
            InputEvent::MouseMove { x, y } => {
                injector.inject_mouse_move(x, y)
                    .map_err(|e| anyhow::anyhow!("mouse move failed: {e}"))?;
            }
            InputEvent::MouseButton { button, down } => {
                injector.inject_mouse_button(button as u32, down)
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
        // Create clipboard on-demand to avoid Send issues
        let clipboard = crate::clipboard::WinClipboard::new()
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
        // Create clipboard on-demand to avoid Send issues
        let clipboard = crate::clipboard::WinClipboard::new()
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
