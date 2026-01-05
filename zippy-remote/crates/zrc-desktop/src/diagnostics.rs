//! Connection diagnostics and quality monitoring

use eframe::egui;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Connection diagnostics data
#[derive(Clone)]
pub struct ConnectionDiagnostics {
    pub latency_ms: Arc<AtomicU32>,
    pub packet_loss: Arc<Mutex<f32>>,
    pub bandwidth_bps: Arc<AtomicU64>,
    pub connection_type: Arc<std::sync::Mutex<ConnectionType>>,
    pub quality: Arc<std::sync::Mutex<ConnectionQuality>>,
    pub last_update: Arc<std::sync::Mutex<Instant>>,
}

impl Default for ConnectionDiagnostics {
    fn default() -> Self {
        Self {
            latency_ms: Arc::new(AtomicU32::new(0)),
            packet_loss: Arc::new(Mutex::new(0.0)),
            bandwidth_bps: Arc::new(AtomicU64::new(0)),
            connection_type: Arc::new(std::sync::Mutex::new(ConnectionType::Unknown)),
            quality: Arc::new(std::sync::Mutex::new(ConnectionQuality::Unknown)),
            last_update: Arc::new(std::sync::Mutex::new(Instant::now())),
        }
    }
}

impl ConnectionDiagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update latency
    pub fn update_latency(&self, ms: u32) {
        self.latency_ms.store(ms, Ordering::Relaxed);
        self.update_quality();
    }

    /// Update packet loss
    pub fn update_packet_loss(&self, percent: f32) {
        *self.packet_loss.lock().unwrap() = percent;
        self.update_quality();
    }

    /// Update bandwidth
    pub fn update_bandwidth(&self, bps: u64) {
        self.bandwidth_bps.store(bps, Ordering::Relaxed);
    }

    /// Update connection type
    pub fn update_connection_type(&self, conn_type: ConnectionType) {
        *self.connection_type.lock().unwrap() = conn_type;
    }

    /// Update quality based on current metrics
    fn update_quality(&self) {
        let latency = self.latency_ms.load(Ordering::Relaxed);
        let packet_loss = *self.packet_loss.lock().unwrap();
        
        let quality = if latency < 50 && packet_loss < 0.01 {
            ConnectionQuality::Excellent
        } else if latency < 100 && packet_loss < 0.05 {
            ConnectionQuality::Good
        } else if latency < 200 && packet_loss < 0.10 {
            ConnectionQuality::Fair
        } else {
            ConnectionQuality::Poor
        };
        
        *self.quality.lock().unwrap() = quality;
        *self.last_update.lock().unwrap() = Instant::now();
    }

    /// Get current quality
    pub fn get_quality(&self) -> ConnectionQuality {
        *self.quality.lock().unwrap()
    }

    /// Render diagnostics display
    pub fn render(&self, ui: &mut egui::Ui) {
        let latency = self.latency_ms.load(Ordering::Relaxed);
        let packet_loss = *self.packet_loss.lock().unwrap();
        let bandwidth = self.bandwidth_bps.load(Ordering::Relaxed);
        let quality = self.get_quality();
        let conn_type = self.connection_type.lock().unwrap().clone();
        
        ui.group(|ui| {
            ui.heading("Connection Diagnostics");
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("Latency:");
                ui.label(format!("{} ms", latency));
            });
            
            ui.horizontal(|ui| {
                ui.label("Packet Loss:");
                ui.label(format!("{:.2}%", packet_loss * 100.0));
            });
            
            ui.horizontal(|ui| {
                ui.label("Bandwidth:");
                ui.label(format_bandwidth(bandwidth));
            });
            
            ui.horizontal(|ui| {
                ui.label("Connection Type:");
                ui.label(format!("{:?}", conn_type));
            });
            
            ui.horizontal(|ui| {
                ui.label("Quality:");
                let (color, text) = quality_color_text(quality);
                ui.colored_label(color, text);
            });
        });
    }

    /// Render compact status indicator
    pub fn render_status_indicator(&self, ui: &mut egui::Ui) {
        let quality = self.get_quality();
        let latency = self.latency_ms.load(Ordering::Relaxed);
        
        let (color, text) = quality_color_text(quality);
        ui.horizontal(|ui| {
            ui.colored_label(color, "●");
            ui.label(format!("{} ({}ms)", text, latency));
        });
    }
}

/// Connection type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionType {
    Direct,
    Relay,
    Mesh,
    Unknown,
}

/// Connection quality
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
    Unknown,
}

fn format_bandwidth(bps: u64) -> String {
    if bps < 1024 {
        format!("{} B/s", bps)
    } else if bps < 1024 * 1024 {
        format!("{:.2} KB/s", bps as f64 / 1024.0)
    } else if bps < 1024 * 1024 * 1024 {
        format!("{:.2} MB/s", bps as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB/s", bps as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn quality_color_text(quality: ConnectionQuality) -> (egui::Color32, &'static str) {
    match quality {
        ConnectionQuality::Excellent => (egui::Color32::from_rgb(0, 255, 0), "Excellent"),
        ConnectionQuality::Good => (egui::Color32::from_rgb(150, 255, 0), "Good"),
        ConnectionQuality::Fair => (egui::Color32::from_rgb(255, 200, 0), "Fair"),
        ConnectionQuality::Poor => (egui::Color32::from_rgb(255, 0, 0), "Poor"),
        ConnectionQuality::Unknown => (egui::Color32::GRAY, "Unknown"),
    }
}
