#![cfg(target_os = "linux")]

use std::sync::Arc;
use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::Mutex;

use zrc_core::platform::{HostPlatform, InputEvent};

use crate::capturer::LinuxCapturer;
use crate::injector::LinuxInjector;

/// Thread-safe wrapper for LinuxCapturer
struct SendCapturer(LinuxCapturer);

// Safety: LinuxCapturer operations are performed synchronously within mutex locks
// and don't hold Linux handles across await points
unsafe impl Send for SendCapturer {}

/// Linux platform implementation
pub struct LinuxPlatform {
    capturer: Arc<Mutex<SendCapturer>>,
    injector: Arc<Mutex<LinuxInjector>>,
}

// Safety: LinuxPlatform uses Mutex for interior mutability and all operations
// are performed synchronously within the lock
unsafe impl Send for LinuxPlatform {}
unsafe impl Sync for LinuxPlatform {}

impl LinuxPlatform {
    /// Create new Linux platform instance
    pub fn new() -> anyhow::Result<Self> {
        let capturer = Arc::new(Mutex::new(SendCapturer(LinuxCapturer::new()
            .map_err(|e| anyhow::anyhow!("capturer init failed: {e}"))?)));
        let injector = Arc::new(Mutex::new(LinuxInjector::new()
            .map_err(|e| anyhow::anyhow!("injector init failed: {e}"))?));

        Ok(Self {
            capturer,
            injector,
        })
    }
}

#[async_trait]
impl HostPlatform for LinuxPlatform {
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
                injector.inject_mouse_button(button, down)
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
        let clipboard = crate::clipboard::X11Clipboard::new()
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
        let clipboard = crate::clipboard::X11Clipboard::new()
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
