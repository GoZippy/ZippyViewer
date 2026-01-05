#![cfg(target_os = "linux")]

use bytes::Bytes;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as XprotoConnectionExt, Atom, Window};
use x11rb::rust_connection::RustConnection;
use x11rb::xcb_ffi::XCBConnection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Failed to access clipboard: {0}")]
    AccessFailed(String),
    #[error("Format not available")]
    FormatNotAvailable,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("X11 error: {0}")]
    X11(String),
}

/// X11 clipboard access
pub struct X11Clipboard {
    conn: XCBConnection,
    window: Window,
    clipboard_atom: Atom,
    primary_atom: Atom,
    utf8_string_atom: Atom,
    string_atom: Atom,
    targets_atom: Atom,
    selection_atom: Atom,
    image_png_atom: Atom,
    last_owner: Window,
}

impl X11Clipboard {
    /// Create clipboard handler
    pub fn new() -> Result<Self, ClipboardError> {
        let (conn, screen_num) = x11rb::connect(None)
            .map_err(|e| ClipboardError::AccessFailed(format!("Failed to connect: {}", e)))?;

        let screen = &conn.setup().roots[screen_num];
        let root_window = screen.root;

        // Create a window for clipboard operations
        let window = conn.generate_id()
            .map_err(|e| ClipboardError::AccessFailed(format!("Failed to generate window ID: {}", e)))?;

        conn.create_window(
            screen.root_depth,
            window,
            root_window,
            0,
            0,
            1,
            1,
            0,
            x11rb::protocol::xproto::WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &[],
        )
        .map_err(|e| ClipboardError::AccessFailed(format!("Failed to create window: {}", e)))?
        .check()
        .map_err(|e| ClipboardError::AccessFailed(format!("Failed to check create window: {}", e)))?;

        // Intern atoms
        let clipboard_atom = conn.intern_atom(false, b"CLIPBOARD")
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom failed: {}", e)))?
            .reply()
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom reply failed: {}", e)))?
            .atom;

        let primary_atom = conn.intern_atom(false, b"PRIMARY")
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom failed: {}", e)))?
            .reply()
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom reply failed: {}", e)))?
            .atom;

        let utf8_string_atom = conn.intern_atom(false, b"UTF8_STRING")
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom failed: {}", e)))?
            .reply()
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom reply failed: {}", e)))?
            .atom;

        let string_atom = conn.intern_atom(false, b"STRING")
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom failed: {}", e)))?
            .reply()
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom reply failed: {}", e)))?
            .atom;

        let targets_atom = conn.intern_atom(false, b"TARGETS")
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom failed: {}", e)))?
            .reply()
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom reply failed: {}", e)))?
            .atom;

        let selection_atom = conn.intern_atom(false, b"XSEL_DATA")
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom failed: {}", e)))?
            .reply()
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom reply failed: {}", e)))?
            .atom;

        let image_png_atom = conn.intern_atom(false, b"image/png")
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom failed: {}", e)))?
            .reply()
            .map_err(|e| ClipboardError::AccessFailed(format!("Intern atom reply failed: {}", e)))?
            .atom;

        // Get current selection owner
        let owner = conn.get_selection_owner(clipboard_atom)
            .map_err(|e| ClipboardError::X11(format!("GetSelectionOwner failed: {}", e)))?
            .reply()
            .map_err(|e| ClipboardError::X11(format!("GetSelectionOwner reply failed: {}", e)))?
            .owner;

        Ok(Self {
            conn,
            window,
            clipboard_atom,
            primary_atom,
            utf8_string_atom,
            string_atom,
            targets_atom,
            selection_atom,
            image_png_atom,
            last_owner: owner,
        })
    }

    /// Read text from clipboard
    pub fn read_text(&self) -> Result<Option<String>, ClipboardError> {
        // Try CLIPBOARD first, then PRIMARY
        if let Some(text) = self.read_selection(self.clipboard_atom)? {
            return Ok(Some(text));
        }
        if let Some(text) = self.read_selection(self.primary_atom)? {
            return Ok(Some(text));
        }
        Ok(None)
    }

    fn read_selection(&self, selection: Atom) -> Result<Option<String>, ClipboardError> {
        // Request selection
        self.conn.convert_selection(
            self.window,
            selection,
            self.utf8_string_atom,
            self.selection_atom,
            x11rb::CURRENT_TIME,
        )
        .map_err(|e| ClipboardError::AccessFailed(format!("ConvertSelection failed: {}", e)))?;

        self.conn.flush()
            .map_err(|e| ClipboardError::AccessFailed(format!("Flush failed: {}", e)))?;

        // Wait for SelectionNotify event
        // This is a simplified implementation - in practice, you'd need proper event loop
        // For now, we'll return None as a placeholder
        // TODO: Implement proper event handling
        Ok(None)
    }

    /// Write text to clipboard
    pub fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        // Set ourselves as selection owner
        self.conn.set_selection_owner(
            self.window,
            self.clipboard_atom,
            x11rb::CURRENT_TIME,
        )
        .map_err(|e| ClipboardError::AccessFailed(format!("SetSelectionOwner failed: {}", e)))?;

        self.conn.flush()
            .map_err(|e| ClipboardError::AccessFailed(format!("Flush failed: {}", e)))?;

        // Store text for later retrieval
        // In a full implementation, you'd handle SelectionRequest events
        // For now, this is a placeholder
        Ok(())
    }

    /// Write image to clipboard
    pub fn write_image(&self, data: &[u8]) -> Result<(), ClipboardError> {
        // Set ourselves as selection owner
        self.conn.set_selection_owner(
            self.window,
            self.clipboard_atom,
            x11rb::CURRENT_TIME,
        )
        .map_err(|e| ClipboardError::AccessFailed(format!("SetSelectionOwner failed: {}", e)))?;

        self.conn.flush()
            .map_err(|e| ClipboardError::AccessFailed(format!("Flush failed: {}", e)))?;

        // Store image data for later retrieval
        // In a full implementation, you'd handle SelectionRequest events
        // For now, this is a placeholder
        Ok(())
    }

    /// Read image from clipboard
    pub fn read_image(&self) -> Result<Option<Bytes>, ClipboardError> {
        // Request image/png from CLIPBOARD
        self.conn.convert_selection(
            self.window,
            self.clipboard_atom,
            self.image_png_atom,
            self.selection_atom,
            x11rb::CURRENT_TIME,
        )
        .map_err(|e| ClipboardError::AccessFailed(format!("ConvertSelection failed: {}", e)))?;

        self.conn.flush()
            .map_err(|e| ClipboardError::AccessFailed(format!("Flush failed: {}", e)))?;

        // Wait for SelectionNotify event
        // This is a simplified implementation - in practice, you'd need proper event loop
        // For now, we'll return None as a placeholder
        // TODO: Implement proper event handling
        Ok(None)
    }

    /// Check if clipboard changed
    pub fn has_changed(&mut self) -> Result<bool, ClipboardError> {
        let owner = self.conn.get_selection_owner(self.clipboard_atom)
            .map_err(|e| ClipboardError::X11(format!("GetSelectionOwner failed: {}", e)))?
            .reply()
            .map_err(|e| ClipboardError::X11(format!("GetSelectionOwner reply failed: {}", e)))?
            .owner;

        if owner != self.last_owner {
            self.last_owner = owner;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Wayland clipboard access (portal-based)
#[cfg(feature = "pipewire")]
pub struct WaylandClipboard {
    // TODO: Implement using ashpd portal clipboard API
}

#[cfg(feature = "pipewire")]
impl WaylandClipboard {
    /// Create Wayland clipboard handler
    pub fn new() -> Result<Self, ClipboardError> {
        // TODO: Request portal session via ashpd
        Ok(Self {})
    }

    /// Read text from clipboard
    pub fn read_text(&self) -> Result<Option<String>, ClipboardError> {
        // TODO: Implement using portal clipboard API
        Ok(None)
    }

    /// Write text to clipboard
    pub fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        // TODO: Implement using portal clipboard API
        Ok(())
    }
}
