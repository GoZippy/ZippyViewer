#![cfg(target_os = "linux")]
#![allow(unsafe_code)] // X11 SHM requires unsafe for shared memory operations

use std::ptr;
use x11rb::connection::Connection;
use x11rb::protocol::shm::{ConnectionExt as ShmConnectionExt, Seg};
use x11rb::protocol::xproto::{ConnectionExt as XprotoConnectionExt, ImageFormat, Window};
use x11rb::rust_connection::RustConnection;
use x11rb::xcb_ffi::XCBConnection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum X11ShmError {
    #[error("X11 connection failed: {0}")]
    ConnectionFailed(String),
    #[error("MIT-SHM extension not available")]
    ShmNotAvailable,
    #[error("SHM segment creation failed: {0}")]
    ShmCreationFailed(String),
    #[error("Frame capture failed: {0}")]
    CaptureFailed(String),
    #[error("Resolution change detected")]
    ResolutionChanged,
}

/// X11 SHM-based capturer (fast, requires MIT-SHM extension)
pub struct X11ShmCapturer {
    conn: XCBConnection,
    screen_num: usize,
    root_window: Window,
    shm_seg: Seg,
    shm_id: i32,
    shm_addr: *mut u8,
    width: u16,
    height: u16,
    depth: u8,
    frame_buffer: Vec<u8>,
}

impl X11ShmCapturer {
    /// Check if MIT-SHM extension is available
    pub fn is_available() -> bool {
        if let Ok((conn, _)) = x11rb::connect(None) {
            if let Ok(ext_info) = conn.shm_query_version() {
                ext_info.is_some()
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Create new X11 SHM capturer
    pub fn new() -> Result<Self, X11ShmError> {
        let (conn, screen_num) = x11rb::connect(None)
            .map_err(|e| X11ShmError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

        // Query SHM extension
        let shm_info = conn.shm_query_version()
            .map_err(|e| X11ShmError::ShmNotAvailable)?
            .ok_or(X11ShmError::ShmNotAvailable)?;

        let screen = &conn.setup().roots[screen_num];
        let root_window = screen.root;
        let width = screen.width_in_pixels;
        let height = screen.height_in_pixels;
        let depth = screen.root_depth;

        // Calculate buffer size (BGRA format)
        let stride = (width as usize) * 4;
        let buffer_size = stride * (height as usize);

        // Create shared memory segment
        let shm_id = unsafe {
            libc::shmget(
                libc::IPC_PRIVATE,
                buffer_size,
                libc::IPC_CREAT | libc::IPC_EXCL | 0o600,
            )
        };

        if shm_id < 0 {
            return Err(X11ShmError::ShmCreationFailed(
                "Failed to create shared memory segment".to_string(),
            ));
        }

        // Attach shared memory
        let shm_addr = unsafe {
            libc::shmat(shm_id, ptr::null(), 0) as *mut u8
        };

        if shm_addr == ptr::null_mut() {
            unsafe {
                libc::shmctl(shm_id, libc::IPC_RMID, ptr::null_mut());
            }
            return Err(X11ShmError::ShmCreationFailed(
                "Failed to attach shared memory".to_string(),
            ));
        }

        // Generate segment ID for X server
        let shm_seg = conn.generate_id()
            .map_err(|e| {
                unsafe {
                    libc::shmdt(shm_addr);
                    libc::shmctl(shm_id, libc::IPC_RMID, ptr::null_mut());
                }
                X11ShmError::ShmCreationFailed(format!("Failed to generate ID: {}", e))
            })?;

        // Attach segment to X server
        conn.shm_attach(shm_seg, shm_id as u32, false)
            .map_err(|e| {
                unsafe {
                    libc::shmdt(shm_addr);
                    libc::shmctl(shm_id, libc::IPC_RMID, ptr::null_mut());
                }
                X11ShmError::ShmCreationFailed(format!("Failed to attach to X server: {}", e))
            })?;

        conn.flush()
            .map_err(|e| {
                unsafe {
                    libc::shmdt(shm_addr);
                    libc::shmctl(shm_id, libc::IPC_RMID, ptr::null_mut());
                }
                X11ShmError::ShmCreationFailed(format!("Failed to flush: {}", e))
            })?;

        let frame_buffer = vec![0u8; buffer_size];

        Ok(Self {
            conn,
            screen_num,
            root_window,
            shm_seg,
            shm_id,
            shm_addr,
            width,
            height,
            depth,
            frame_buffer,
        })
    }

    /// Capture a frame using ShmGetImage
    pub fn capture_frame(&mut self) -> Result<Vec<u8>, X11ShmError> {
        // Check if resolution changed
        let screen = &self.conn.setup().roots[self.screen_num];
        let new_width = screen.width_in_pixels;
        let new_height = screen.height_in_pixels;

        if new_width != self.width || new_height != self.height {
            // Resolution changed, need to recreate SHM segment
            self.handle_resolution_change()?;
        }

        // Use ShmGetImage to capture root window
        let reply = self.conn
            .shm_get_image(
                self.root_window,
                0,
                0,
                self.width,
                self.height,
                !0, // plane_mask
                self.shm_seg,
                0, // offset
            )
            .map_err(|e| X11ShmError::CaptureFailed(format!("ShmGetImage failed: {}", e)))?
            .reply()
            .map_err(|e| X11ShmError::CaptureFailed(format!("ShmGetImage reply failed: {}", e)))?;

        // Copy from shared memory to buffer
        // The data is in the shared memory segment at shm_addr
        let stride = (self.width as usize) * 4;
        let buffer_size = stride * (self.height as usize);

        unsafe {
            ptr::copy_nonoverlapping(
                self.shm_addr,
                self.frame_buffer.as_mut_ptr(),
                buffer_size,
            );
        }

        Ok(self.frame_buffer.clone())
    }

    /// Handle resolution changes
    pub fn handle_resolution_change(&mut self) -> Result<(), X11ShmError> {
        let screen = &self.conn.setup().roots[self.screen_num];
        let new_width = screen.width_in_pixels;
        let new_height = screen.height_in_pixels;

        if new_width == self.width && new_height == self.height {
            return Ok(());
        }

        // Detach old segment
        let _ = self.conn.shm_detach(self.shm_seg);
        unsafe {
            libc::shmdt(self.shm_addr);
            libc::shmctl(self.shm_id, libc::IPC_RMID, ptr::null_mut());
        }

        // Update dimensions
        self.width = new_width;
        self.height = new_height;

        // Recreate SHM segment
        let stride = (self.width as usize) * 4;
        let buffer_size = stride * (self.height as usize);

        let shm_id = unsafe {
            libc::shmget(
                libc::IPC_PRIVATE,
                buffer_size,
                libc::IPC_CREAT | libc::IPC_EXCL | 0o600,
            )
        };

        if shm_id < 0 {
            return Err(X11ShmError::ShmCreationFailed(
                "Failed to recreate shared memory segment".to_string(),
            ));
        }

        let shm_addr = unsafe {
            libc::shmat(shm_id, ptr::null(), 0) as *mut u8
        };

        if shm_addr == ptr::null_mut() {
            unsafe {
                libc::shmctl(shm_id, libc::IPC_RMID, ptr::null_mut());
            }
            return Err(X11ShmError::ShmCreationFailed(
                "Failed to attach new shared memory".to_string(),
            ));
        }

        let shm_seg = self.conn.generate_id()
            .map_err(|e| {
                unsafe {
                    libc::shmdt(shm_addr);
                    libc::shmctl(shm_id, libc::IPC_RMID, ptr::null_mut());
                }
                X11ShmError::ShmCreationFailed(format!("Failed to generate new ID: {}", e))
            })?;

        self.conn.shm_attach(shm_seg, shm_id as u32, false)
            .map_err(|e| {
                unsafe {
                    libc::shmdt(shm_addr);
                    libc::shmctl(shm_id, libc::IPC_RMID, ptr::null_mut());
                }
                X11ShmError::ShmCreationFailed(format!("Failed to attach new segment: {}", e))
            })?;

        self.conn.flush()
            .map_err(|e| {
                unsafe {
                    libc::shmdt(shm_addr);
                    libc::shmctl(shm_id, libc::IPC_RMID, ptr::null_mut());
                }
                X11ShmError::ShmCreationFailed(format!("Failed to flush: {}", e))
            })?;

        self.shm_seg = shm_seg;
        self.shm_id = shm_id;
        self.shm_addr = shm_addr;
        self.frame_buffer = vec![0u8; buffer_size];

        Ok(())
    }
}

impl Drop for X11ShmCapturer {
    fn drop(&mut self) {
        // Detach from X server
        let _ = self.conn.shm_detach(self.shm_seg);
        
        // Detach from process
        unsafe {
            libc::shmdt(self.shm_addr);
            libc::shmctl(self.shm_id, libc::IPC_RMID, ptr::null_mut());
        }
    }
}
