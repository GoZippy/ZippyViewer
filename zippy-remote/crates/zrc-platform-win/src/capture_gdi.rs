#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use std::ptr::null_mut;
use thiserror::Error;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::*,
    UI::WindowsAndMessaging::*,
};

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("win32 error")]
    Win32,
    #[error("unexpected capture size")]
    Size,
    #[error("resource creation failed")]
    ResourceCreation,
}

#[derive(Debug, Clone)]
pub struct BgraFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32, // bytes per row
    pub bgra: Vec<u8>,
}

/// GDI-based capture fallback
pub struct GdiCapturer {
    screen_dc: HDC,
    memory_dc: HDC,
    bitmap: HBITMAP,
    width: i32,
    height: i32,
    buffer: Vec<u8>,
}

impl GdiCapturer {
    /// Create GDI capturer for primary display
    pub fn new() -> Result<Self, CaptureError> {
        unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN) as i32;
            let h = GetSystemMetrics(SM_CYSCREEN) as i32;
            if w <= 0 || h <= 0 {
                return Err(CaptureError::Size);
            }

            let screen_dc = GetDC(None);
            if screen_dc.is_invalid() {
                return Err(CaptureError::Win32);
            }

            let memory_dc = CreateCompatibleDC(Some(screen_dc));
            if memory_dc.is_invalid() {
                let _ = ReleaseDC(None, screen_dc);
                return Err(CaptureError::ResourceCreation);
            }

            let bitmap = CreateCompatibleBitmap(screen_dc, w, h);
            if bitmap.is_invalid() {
                let _ = DeleteDC(memory_dc);
                let _ = ReleaseDC(None, screen_dc);
                return Err(CaptureError::ResourceCreation);
            }

            let old = SelectObject(memory_dc, bitmap.into());
            if old.is_invalid() {
                let _ = DeleteObject(bitmap.into());
                let _ = DeleteDC(memory_dc);
                let _ = ReleaseDC(None, screen_dc);
                return Err(CaptureError::ResourceCreation);
            }

            let stride = (w as u32) * 4;
            let buffer = vec![0u8; (stride as usize) * (h as usize)];

            Ok(Self {
                screen_dc,
                memory_dc,
                bitmap,
                width: w,
                height: h,
                buffer,
            })
        }
    }

    /// Capture frame using BitBlt
    pub fn capture_frame(&mut self) -> Result<BgraFrame, CaptureError> {
        unsafe {
            // Check if resolution changed
            let w = GetSystemMetrics(SM_CXSCREEN) as i32;
            let h = GetSystemMetrics(SM_CYSCREEN) as i32;
            if w != self.width || h != self.height {
                // Resolution changed, recreate resources
                self.handle_resolution_change()?;
            }

            let old = SelectObject(self.memory_dc, self.bitmap.into());
            if old.is_invalid() {
                return Err(CaptureError::Win32);
            }

            BitBlt(
                self.memory_dc,
                0,
                0,
                self.width,
                self.height,
                Some(self.screen_dc),
                0,
                0,
                SRCCOPY,
            ).map_err(|_| CaptureError::Win32)?;

            let stride = (self.width as u32) * 4;

            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: self.width,
                    biHeight: -self.height, // top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0 as u32,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbReserved: 0,
                }],
            };

            let scanlines = GetDIBits(
                self.memory_dc,
                self.bitmap,
                0,
                self.height as u32,
                Some(self.buffer.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            let _ = SelectObject(self.memory_dc, old);

            if scanlines == 0 {
                return Err(CaptureError::Win32);
            }

            Ok(BgraFrame {
                width: self.width as u32,
                height: self.height as u32,
                stride,
                bgra: self.buffer.clone(),
            })
        }
    }

    /// Handle resolution change
    pub fn handle_resolution_change(&mut self) -> Result<(), CaptureError> {
        unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN) as i32;
            let h = GetSystemMetrics(SM_CYSCREEN) as i32;
            if w <= 0 || h <= 0 {
                return Err(CaptureError::Size);
            }

            // Cleanup old resources
            let _ = SelectObject(self.memory_dc, HGDIOBJ::default());
            let _ = DeleteObject(self.bitmap.into());
            let _ = ReleaseDC(None, self.screen_dc);

            // Recreate resources
            self.screen_dc = GetDC(None);
            if self.screen_dc.is_invalid() {
                return Err(CaptureError::Win32);
            }

            self.bitmap = CreateCompatibleBitmap(self.screen_dc, w, h);
            if self.bitmap.is_invalid() {
                let _ = ReleaseDC(None, self.screen_dc);
                return Err(CaptureError::ResourceCreation);
            }

            let old = SelectObject(self.memory_dc, self.bitmap.into());
            if old.is_invalid() {
                let _ = DeleteObject(self.bitmap.into());
                let _ = ReleaseDC(None, self.screen_dc);
                return Err(CaptureError::ResourceCreation);
            }

            self.width = w;
            self.height = h;
            let stride = (w as u32) * 4;
            self.buffer = vec![0u8; (stride as usize) * (h as usize)];

            Ok(())
        }
    }
}

impl Drop for GdiCapturer {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.memory_dc, HGDIOBJ::default());
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.memory_dc);
            let _ = ReleaseDC(None, self.screen_dc);
        }
    }
}

/// Simple screen capture using GDI BitBlt into a 32bpp DIB.
/// Works on most Windows setups; not the fastest, but great as an MVP stub.
pub fn capture_primary_bgra() -> Result<BgraFrame, CaptureError> {
    let mut capturer = GdiCapturer::new()?;
    capturer.capture_frame()
}
