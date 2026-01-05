#![cfg(target_os = "linux")]

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as XprotoConnectionExt, ImageFormat, Window};
use x11rb::rust_connection::RustConnection;
use x11rb::xcb_ffi::XCBConnection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum X11BasicError {
    #[error("X11 connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Frame capture failed: {0}")]
    CaptureFailed(String),
}

/// X11 basic capturer (fallback, uses GetImage)
pub struct X11BasicCapturer {
    conn: XCBConnection,
    screen_num: usize,
    root_window: Window,
    width: u16,
    height: u16,
    depth: u8,
    frame_buffer: Vec<u8>,
}

impl X11BasicCapturer {
    /// Create new X11 basic capturer
    pub fn new() -> Result<Self, X11BasicError> {
        let (conn, screen_num) = x11rb::connect(None)
            .map_err(|e| X11BasicError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

        let screen = &conn.setup().roots[screen_num];
        let root_window = screen.root;
        let width = screen.width_in_pixels;
        let height = screen.height_in_pixels;
        let depth = screen.root_depth;

        // Pre-allocate buffer (BGRA format)
        let stride = (width as usize) * 4;
        let buffer_size = stride * (height as usize);
        let frame_buffer = vec![0u8; buffer_size];

        Ok(Self {
            conn,
            screen_num,
            root_window,
            width,
            height,
            depth,
            frame_buffer,
        })
    }

    /// Capture a frame using GetImage
    pub fn capture_frame(&mut self) -> Result<Vec<u8>, X11BasicError> {
        // Check if resolution changed
        let screen = &self.conn.setup().roots[self.screen_num];
        let new_width = screen.width_in_pixels;
        let new_height = screen.height_in_pixels;

        if new_width != self.width || new_height != self.height {
            self.width = new_width;
            self.height = new_height;
            let stride = (self.width as usize) * 4;
            let buffer_size = stride * (self.height as usize);
            self.frame_buffer = vec![0u8; buffer_size];
        }

        // Get image from root window
        let reply = self.conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root_window,
                0,
                0,
                self.width,
                self.height,
                !0, // plane_mask
            )
            .map_err(|e| X11BasicError::CaptureFailed(format!("GetImage failed: {}", e)))?
            .reply()
            .map_err(|e| X11BasicError::CaptureFailed(format!("GetImage reply failed: {}", e)))?;

        // Convert X11 image data to BGRA format
        // X11 GetImage returns data in the format specified by the visual
        // We need to convert it to BGRA (32-bit, 8 bits per channel)
        let bpp = (reply.depth as usize + 7) / 8; // bytes per pixel
        let src_stride = (reply.width as usize * bpp + 3) / 4 * 4; // X11 pads to 4-byte boundary
        let dst_stride = (self.width as usize) * 4;

        // Simple conversion: if depth is 24 or 32, we can extract RGB
        // For other depths, we'd need proper color conversion
        if reply.depth == 24 || reply.depth == 32 {
            for y in 0..(reply.height as usize).min(self.height as usize) {
                for x in 0..(reply.width as usize).min(self.width as usize) {
                    let src_idx = y * src_stride + x * bpp;
                    let dst_idx = y * dst_stride + x * 4;

                    if src_idx + bpp <= reply.data.len() && dst_idx + 4 <= self.frame_buffer.len() {
                        // X11 uses BGR order, convert to BGRA
                        if bpp >= 3 {
                            self.frame_buffer[dst_idx] = reply.data[src_idx];     // B
                            self.frame_buffer[dst_idx + 1] = reply.data[src_idx + 1]; // G
                            self.frame_buffer[dst_idx + 2] = reply.data[src_idx + 2]; // R
                            self.frame_buffer[dst_idx + 3] = if bpp >= 4 { reply.data[src_idx + 3] } else { 255 }; // A
                        }
                    }
                }
            }
        } else {
            // For other depths, we'd need proper color conversion
            // For now, return error or use a simpler approach
            return Err(X11BasicError::CaptureFailed(
                format!("Unsupported depth: {}", reply.depth)
            ));
        }

        Ok(self.frame_buffer.clone())
    }
}

impl Drop for X11BasicCapturer {
    fn drop(&mut self) {
        // Connection is automatically closed when dropped
    }
}
