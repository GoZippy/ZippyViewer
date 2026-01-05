#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use std::collections::HashSet;
use thiserror::Error;
use windows::Win32::{
    Foundation::*,
    System::SystemInformation::*,
    UI::Input::KeyboardAndMouse::*,
    UI::WindowsAndMessaging::*,
};

#[derive(Debug, Error)]
pub enum InputError {
    #[error("SendInput failed")]
    SendFailed,
    #[error("elevation required")]
    ElevationRequired,
    #[error("coordinate out of bounds")]
    CoordinateOutOfBounds,
}

/// Coordinate mapping for multi-monitor
#[derive(Debug)]
pub struct CoordinateMapper {
    pub(crate) virtual_screen: RECT,
}

impl CoordinateMapper {
    pub fn new() -> Self {
        unsafe {
            let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let right = left + GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let bottom = top + GetSystemMetrics(SM_CYVIRTUALSCREEN);

            Self {
                virtual_screen: RECT {
                    left,
                    top,
                    right,
                    bottom,
                },
            }
        }
    }

    /// Convert logical coordinates to absolute (0-65535 range)
    pub fn to_absolute(&self, x: i32, y: i32) -> (i32, i32) {
        let width = self.virtual_screen.right - self.virtual_screen.left;
        let height = self.virtual_screen.bottom - self.virtual_screen.top;

        if width <= 0 || height <= 0 {
            return (0, 0);
        }

        // Clamp to virtual screen bounds
        let x = x.clamp(self.virtual_screen.left, self.virtual_screen.right - 1);
        let y = y.clamp(self.virtual_screen.top, self.virtual_screen.bottom - 1);

        // Normalize to 0..65535
        let nx = ((x - self.virtual_screen.left) * 65535) / (width - 1);
        let ny = ((y - self.virtual_screen.top) * 65535) / (height - 1);

        (nx, ny)
    }

    /// Clamp to valid screen bounds
    pub fn clamp(&self, x: i32, y: i32) -> (i32, i32) {
        (
            x.clamp(self.virtual_screen.left, self.virtual_screen.right - 1),
            y.clamp(self.virtual_screen.top, self.virtual_screen.bottom - 1),
        )
    }
}

/// Windows input injection via SendInput
pub struct WinInjector {
    pub(crate) held_keys: HashSet<u16>,
    pub(crate) coordinate_mapper: CoordinateMapper,
    is_elevated: bool,
}

#[cfg(test)]
impl WinInjector {
    /// Test helper to access held keys
    pub fn held_keys(&self) -> &HashSet<u16> {
        &self.held_keys
    }
    
    /// Test helper to access coordinate mapper
    pub fn coordinate_mapper(&self) -> &CoordinateMapper {
        &self.coordinate_mapper
    }
}

impl WinInjector {
    /// Create input injector
    pub fn new() -> Self {
        unsafe {
            let is_elevated = Self::check_elevation();
            Self {
                held_keys: HashSet::new(),
                coordinate_mapper: CoordinateMapper::new(),
                is_elevated,
            }
        }
    }

    /// Check if running with elevated privileges
    pub fn is_elevated(&self) -> bool {
        self.is_elevated
    }

    unsafe fn check_elevation() -> bool {
        // Simplified check - in production, use proper token checking
        false // Placeholder
    }

    /// Inject mouse move
    pub fn inject_mouse_move(&mut self, x: i32, y: i32) -> Result<(), InputError> {
        unsafe {
            let (abs_x, abs_y) = self.coordinate_mapper.to_absolute(x, y);

            let mut inp = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: abs_x,
                        dy: abs_y,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            let sent = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
            if sent == 1 {
                Ok(())
            } else {
                Err(InputError::SendFailed)
            }
        }
    }

    /// Inject mouse button
    pub fn inject_mouse_button(&mut self, button: u32, down: bool) -> Result<(), InputError> {
        unsafe {
            let flag = match (button, down) {
                (1, true) => MOUSEEVENTF_LEFTDOWN,
                (1, false) => MOUSEEVENTF_LEFTUP,
                (2, true) => MOUSEEVENTF_RIGHTDOWN,
                (2, false) => MOUSEEVENTF_RIGHTUP,
                (3, true) => MOUSEEVENTF_MIDDLEDOWN,
                (3, false) => MOUSEEVENTF_MIDDLEUP,
                (4, true) => MOUSEEVENTF_XDOWN,
                (4, false) => MOUSEEVENTF_XUP,
                (5, true) => MOUSEEVENTF_XDOWN,
                (5, false) => MOUSEEVENTF_XUP,
                _ => return Err(InputError::SendFailed),
            };

            let mut inp = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: if button >= 4 {
                            if button == 4 {
                                XBUTTON1 as u32
                            } else {
                                XBUTTON2 as u32
                            }
                        } else {
                            0
                        },
                        dwFlags: flag,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            let sent = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
            if sent == 1 {
                Ok(())
            } else {
                Err(InputError::SendFailed)
            }
        }
    }

    /// Inject mouse scroll
    pub fn inject_mouse_scroll(&mut self, delta: i32, horizontal: bool) -> Result<(), InputError> {
        unsafe {
            let flag = if horizontal {
                MOUSEEVENTF_HWHEEL
            } else {
                MOUSEEVENTF_WHEEL
            };

            let mut inp = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: (delta * WHEEL_DELTA as i32) as u32,
                        dwFlags: flag,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            let sent = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
            if sent == 1 {
                Ok(())
            } else {
                Err(InputError::SendFailed)
            }
        }
    }

    /// Inject key
    pub fn inject_key(&mut self, vk: u32, down: bool) -> Result<(), InputError> {
        unsafe {
            let flags = if down {
                KEYBD_EVENT_FLAGS(0)
            } else {
                KEYEVENTF_KEYUP
            };

            // Track held keys
            if down {
                self.held_keys.insert(vk as u16);
            } else {
                self.held_keys.remove(&(vk as u16));
            }

            // Handle extended keys
            let mut ext_flag = KEYBD_EVENT_FLAGS(0);
            let vk_u16 = vk as u16;
            if vk_u16 == VK_RIGHT.0 
                || vk_u16 == VK_LEFT.0 
                || vk_u16 == VK_UP.0 
                || vk_u16 == VK_DOWN.0 
                || vk_u16 == VK_RETURN.0  // Numpad Enter uses same VK as regular Enter
                || vk_u16 == VK_RCONTROL.0 
                || vk_u16 == VK_RMENU.0 {
                ext_flag = KEYEVENTF_EXTENDEDKEY;
            }

            let mut inp = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(vk as u16),
                        wScan: 0,
                        dwFlags: flags | ext_flag,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            let sent = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
            if sent == 1 {
                Ok(())
            } else {
                Err(InputError::SendFailed)
            }
        }
    }

    /// Inject text (Unicode)
    pub fn inject_text(&mut self, text: &str) -> Result<(), InputError> {
        unsafe {
            for ch in text.chars() {
                let mut inp = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: ch as u16,
                            dwFlags: KEYEVENTF_UNICODE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };

                // Send key down
                let sent = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
                if sent != 1 {
                    return Err(InputError::SendFailed);
                }

                // Send key up
                inp.Anonymous.ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;
                let sent = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
                if sent != 1 {
                    return Err(InputError::SendFailed);
                }
            }

            Ok(())
        }
    }

    /// Release all held keys
    pub fn release_all_keys(&mut self) -> Result<(), InputError> {
        let held = self.held_keys.clone();
        for vk in held {
            self.inject_key(vk as u32, false)?;
        }
        Ok(())
    }
}

impl Drop for WinInjector {
    fn drop(&mut self) {
        // Release all held keys on drop
        let _ = self.release_all_keys();
    }
}
