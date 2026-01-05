//! Multi-monitor support for remote sessions

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Monitor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: MonitorId,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

/// Monitor identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorId(pub u32);

/// Monitor layout manager
pub struct MonitorManager {
    monitors: HashMap<MonitorId, MonitorInfo>,
    preferences: HashMap<String, MonitorId>, // device_id -> preferred monitor
}

impl MonitorManager {
    pub fn new() -> Self {
        Self {
            monitors: HashMap::new(),
            preferences: HashMap::new(),
        }
    }

    /// Update monitor list from remote
    pub fn update_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        self.monitors.clear();
        for monitor in monitors {
            self.monitors.insert(monitor.id, monitor);
        }
    }

    /// Get monitor by ID
    pub fn get_monitor(&self, id: MonitorId) -> Option<&MonitorInfo> {
        self.monitors.get(&id)
    }

    /// List all monitors
    pub fn list_monitors(&self) -> Vec<&MonitorInfo> {
        self.monitors.values().collect()
    }

    /// Get primary monitor
    pub fn get_primary(&self) -> Option<&MonitorInfo> {
        self.monitors.values().find(|m| m.is_primary)
    }

    /// Set preferred monitor for device
    pub fn set_preference(&mut self, device_id: &str, monitor_id: MonitorId) {
        self.preferences.insert(device_id.to_string(), monitor_id);
    }

    /// Get preferred monitor for device
    pub fn get_preference(&self, device_id: &str) -> Option<MonitorId> {
        self.preferences.get(device_id).copied()
    }

    /// Render monitor layout diagram
    pub fn render_layout_diagram(&self, ui: &mut egui::Ui) -> Option<MonitorId> {
        if self.monitors.is_empty() {
            ui.label("No monitors available");
            return None;
        }

        let mut selected = None;
        
        // Calculate bounding box
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        
        for monitor in self.monitors.values() {
            min_x = min_x.min(monitor.x);
            min_y = min_y.min(monitor.y);
            max_x = max_x.max(monitor.x + monitor.width as i32);
            max_y = max_y.max(monitor.y + monitor.height as i32);
        }
        
        let total_width = (max_x - min_x) as f32;
        let total_height = (max_y - min_y) as f32;
        
        // Scale to fit in available space
        let available_size = ui.available_size();
        let scale = (available_size.x / total_width)
            .min(available_size.y / total_height)
            .min(1.0);
        
        // Render monitors
        let (response, painter) = ui.allocate_painter(available_size, egui::Sense::click());
        
        for monitor in self.monitors.values() {
            let x = (monitor.x - min_x) as f32 * scale;
            let y = (monitor.y - min_y) as f32 * scale;
            let w = monitor.width as f32 * scale;
            let h = monitor.height as f32 * scale;
            
            let rect = egui::Rect::from_min_size(
                response.rect.min + egui::vec2(x, y),
                egui::vec2(w, h),
            );
            
            // Draw monitor rectangle
            let color = if monitor.is_primary {
                egui::Color32::from_rgb(100, 150, 255)
            } else {
                egui::Color32::from_rgb(80, 80, 80)
            };
            
            painter.rect_filled(rect, 0.0, color);
            painter.rect_stroke(rect, 0.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
            
            // Draw label
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &monitor.name,
                egui::FontId::default(),
                egui::Color32::WHITE,
            );
            
            // Check if clicked
            if response.clicked() && rect.contains(response.interact_pointer_pos().unwrap_or_default()) {
                selected = Some(monitor.id);
            }
        }
        
        selected
    }

    /// Render monitor selector dropdown
    pub fn render_selector(&self, ui: &mut egui::Ui, current: Option<MonitorId>) -> Option<MonitorId> {
        if self.monitors.is_empty() {
            ui.label("No monitors");
            return current;
        }
        
        let mut selected = current;
        
        egui::ComboBox::from_id_source("monitor_selector")
            .selected_text(
                selected
                    .and_then(|id| self.get_monitor(id))
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| "Select monitor".to_string()),
            )
            .show_ui(ui, |ui| {
                for monitor in self.list_monitors() {
                    let is_selected = selected == Some(monitor.id);
                    if ui.selectable_label(is_selected, &monitor.name).clicked() {
                        selected = Some(monitor.id);
                    }
                }
            });
        
        selected
    }
}
