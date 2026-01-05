//! Clipboard synchronization
use std::sync::{Arc, Mutex};
use std::time::Duration;
use arboard::{Clipboard, ImageData};
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use zrc_proto::v1::{ControlMsgV1, ControlMsgTypeV1, ClipboardMsgV1, ClipboardFormatV1, ClipboardDirectionV1};
use std::time::Instant;

/// Managers local and remote clipboard synchronization
pub struct ClipboardManager {
    clipboard: Arc<Mutex<Clipboard>>,
    enabled: Arc<AtomicBool>,
    control_tx: mpsc::Sender<ControlMsgV1>,
    // Last hash or content to detect changes
    last_text: Arc<Mutex<Option<String>>>,
    // last_image_hash: Arc<Mutex<Option<u64>>>, // For optimization later
}

impl ClipboardManager {
    pub fn new(control_tx: mpsc::Sender<ControlMsgV1>) -> Result<Self, anyhow::Error> {
        let clipboard = Clipboard::new()?;
        
        Ok(Self {
            clipboard: Arc::new(Mutex::new(clipboard)),
            enabled: Arc::new(AtomicBool::new(true)),
            control_tx,
            last_text: Arc::new(Mutex::new(None)),
        })
    }
    
    /// Enable or disable sync
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if sync is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    
    /// Start monitoring local clipboard (spawns a thread)
    pub fn start_monitoring(&self) {
        let clipboard = self.clipboard.clone();
        let enabled = self.enabled.clone();
        let control_tx = self.control_tx.clone();
        let last_text = self.last_text.clone();
        
        // Use std::thread because arboard might be blocking or need OS thread affinity
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(500));
                
                if !enabled.load(Ordering::Relaxed) {
                    continue;
                }
                
                // Check text
                let current_text = {
                    if let Ok(mut lock) = clipboard.lock() {
                        lock.get_text().ok()
                    } else {
                        None
                    }
                };

                if let Some(text) = current_text {
                    // Check size limit
                    const MAX_TEXT_SIZE: usize = 10 * 1024 * 1024; // 10MB
                    if text.len() > MAX_TEXT_SIZE {
                        tracing::warn!("Clipboard text too large ({} bytes), skipping", text.len());
                        continue;
                    }
                    
                    let mut last = last_text.lock().unwrap();
                    if last.as_deref() != Some(&text) {
                        // Change detected
                        *last = Some(text.clone());
                        drop(last); // Release lock before sending
                        
                        // Send to remote
                        let msg = ControlMsgV1 {
                            msg_type: ControlMsgTypeV1::Clipboard as i32,
                            payload: Some(zrc_proto::v1::control_msg_v1::Payload::Clipboard(ClipboardMsgV1 {
                                direction: ClipboardDirectionV1::ToDevice as i32,
                                format: ClipboardFormatV1::Text as i32,
                                data: text.into_bytes(),
                                sequence_id: 0, // TODO: sequence
                            })),
                            ..Default::default()
                        };
                        
                        let _ = control_tx.blocking_send(msg);
                    }
                }
                
                // Check image
                let current_image = {
                    if let Ok(mut lock) = clipboard.lock() {
                        lock.get_image().ok()
                    } else {
                        None
                    }
                };

                if let Some(image) = current_image {
                    // Check size limit (default 10MB)
                    const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024;
                    let image_size = image.width * image.height * 4; // RGBA
                    
                    if image_size <= MAX_IMAGE_SIZE {
                        // Convert image to bytes (RGBA format)
                        let mut image_data = Vec::with_capacity(image_size);
                        image_data.extend_from_slice(&image.bytes);
                        
                        // Send to remote
                        let msg = ControlMsgV1 {
                            msg_type: ControlMsgTypeV1::Clipboard as i32,
                            payload: Some(zrc_proto::v1::control_msg_v1::Payload::Clipboard(ClipboardMsgV1 {
                                direction: ClipboardDirectionV1::ToDevice as i32,
                                format: ClipboardFormatV1::ImagePng as i32,
                                data: image_data,
                                sequence_id: 0,
                            })),
                            ..Default::default()
                        };
                        
                        let _ = control_tx.blocking_send(msg);
                    } else {
                        // Image too large - skip
                        tracing::warn!("Clipboard image too large ({} bytes), skipping", image_size);
                    }
                }
            }
        });
    }
    
    /// Apply remote update to local clipboard
    pub fn apply_remote_update(&self, msg: ClipboardMsgV1) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        // Check size limits
        const MAX_TEXT_SIZE: usize = 10 * 1024 * 1024; // 10MB
        const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB
        
        if msg.data.len() > MAX_TEXT_SIZE.max(MAX_IMAGE_SIZE) {
            tracing::warn!("Clipboard data too large ({} bytes), rejecting", msg.data.len());
            return;
        }

        if let Ok(mut clipboard) = self.clipboard.lock() {
            match ClipboardFormatV1::try_from(msg.format) {
                Ok(ClipboardFormatV1::Text) => {
                    if msg.data.len() > MAX_TEXT_SIZE {
                        tracing::warn!("Text clipboard data too large, rejecting");
                        return;
                    }
                    if let Ok(text) = String::from_utf8(msg.data) {
                        // Update last_text to avoid loop
                        if let Ok(mut last) = self.last_text.lock() {
                            *last = Some(text.clone());
                        }
                        
                        let _ = clipboard.set_text(text);
                    }
                },
                Ok(ClipboardFormatV1::ImagePng) | Ok(ClipboardFormatV1::ImageBmp) => {
                    if msg.data.len() > MAX_IMAGE_SIZE {
                        tracing::warn!("Image clipboard data too large, rejecting");
                        return;
                    }
                    // Convert bytes to ImageData
                    // Assume RGBA format - we'd need width/height from protocol
                    // For now, this is a placeholder - the protocol may need to include dimensions
                    // ImageData { width, height, bytes: msg.data }
                    // For MVP, we'll skip image paste until protocol supports dimensions
                    tracing::debug!("Image clipboard update received ({} bytes), but dimensions not available in protocol", msg.data.len());
                },
                _ => {
                    tracing::debug!("Unsupported clipboard format: {}", msg.format);
                }
            }
        }
    }
}
