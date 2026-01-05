//! Input capture and transmission for remote control

use eframe::egui::{self, Pos2, Rect, Vec2};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

/// Handles input capture and transmission to remote device
pub struct InputHandler {
    enabled: AtomicBool,
    input_mode: InputMode,
    coordinate_mapper: Option<CoordinateMapper>,
    pressed_keys: HashSet<egui::Key>,
    pressed_buttons: Vec<egui::PointerButton>,
    modifiers: egui::Modifiers,
}

impl Default for InputHandler {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            input_mode: InputMode::Control,
            coordinate_mapper: None,
            pressed_keys: HashSet::new(),
            pressed_buttons: Vec::new(),
            modifiers: egui::Modifiers::default(),
        }
    }
}

impl InputHandler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable input capture
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if input is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Set input mode (view-only vs control)
    pub fn set_input_mode(&mut self, mode: InputMode) {
        self.input_mode = mode;
    }

    /// Update coordinate mapper for viewer window
    pub fn update_coordinate_mapper(&mut self, viewer_rect: Rect, remote_size: Vec2) {
        self.coordinate_mapper = Some(CoordinateMapper {
            viewer_rect,
            remote_size,
        });
    }

    /// Handle egui input event
    pub fn handle_event(&mut self, event: &egui::Event, viewer_rect: Rect) -> Option<InputEvent> {
        if !self.is_enabled() || self.input_mode == InputMode::ViewOnly {
            return None;
        }

        match event {
            egui::Event::PointerMoved(pos) => {
                if viewer_rect.contains(*pos) {
                    if let Some(mapper) = &self.coordinate_mapper {
                        let (x, y) = mapper.map_to_remote(*pos);
                        return Some(InputEvent::MouseMove { x, y });
                    }
                }
            }
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: _,
            } => {
                if viewer_rect.contains(*pos) {
                    if let Some(mapper) = &self.coordinate_mapper {
                        let (x, y) = mapper.map_to_remote(*pos);
                        if *pressed {
                            if !self.pressed_buttons.contains(button) {
                                self.pressed_buttons.push(*button);
                            }
                        } else {
                            self.pressed_buttons.retain(|b| b != button);
                        }
                        return Some(InputEvent::MouseButton {
                            button: *button,
                            pressed: *pressed,
                            x,
                            y,
                        });
                    }
                }
            }
            egui::Event::MouseWheel { delta, .. } => {
                return Some(InputEvent::Scroll {
                    delta_x: delta.x,
                    delta_y: delta.y,
                });
            }
            egui::Event::Key { key, pressed, modifiers, .. } => {
                if *pressed {
                    self.pressed_keys.insert(*key);
                } else {
                    self.pressed_keys.remove(key);
                }
                self.modifiers = *modifiers;
                return Some(InputEvent::Key {
                    key: *key,
                    pressed: *pressed,
                    modifiers: *modifiers,
                });
            }
            _ => {}
        }
        None
    }

    /// Send special key sequence (e.g., Ctrl+Alt+Del)
    pub fn send_special_sequence(&self, seq: SpecialSequence) -> InputEvent {
        InputEvent::SpecialSequence(seq)
    }

    /// Flush pending events (for batching)
    pub fn flush(&mut self) {
        // Clear any pending state if needed
    }
}

/// Input mode for viewer
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    ViewOnly,
    Control,
}

/// Mapped input event for transmission
#[derive(Debug, Clone)]
pub enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseButton {
        button: egui::PointerButton,
        pressed: bool,
        x: i32,
        y: i32,
    },
    Scroll { delta_x: f32, delta_y: f32 },
    Key {
        key: egui::Key,
        pressed: bool,
        modifiers: egui::Modifiers,
    },
    SpecialSequence(SpecialSequence),
}

/// Special key sequences
#[derive(Debug, Clone)]
pub enum SpecialSequence {
    CtrlAltDel,
    AltTab,
    AltF4,
    PrintScreen,
    Custom(Vec<egui::Key>),
}

/// Maps local viewer coordinates to remote display coordinates
#[derive(Clone, Copy)]
pub struct CoordinateMapper {
    pub viewer_rect: Rect,
    pub remote_size: Vec2,
}

impl CoordinateMapper {
    /// Map local coordinates to remote coordinates
    pub fn map_to_remote(&self, local: Pos2) -> (i32, i32) {
        let relative = local - self.viewer_rect.min;
        let scale_x = self.remote_size.x / self.viewer_rect.width();
        let scale_y = self.remote_size.y / self.viewer_rect.height();
        
        let remote_x = (relative.x * scale_x).round() as i32;
        let remote_y = (relative.y * scale_y).round() as i32;
        
        // Clamp to remote display bounds
        let remote_x = remote_x.max(0).min(self.remote_size.x as i32 - 1);
        let remote_y = remote_y.max(0).min(self.remote_size.y as i32 - 1);
        
        (remote_x, remote_y)
    }

    /// Check if point is within viewer
    pub fn contains(&self, point: Pos2) -> bool {
        self.viewer_rect.contains(point)
    }
}
