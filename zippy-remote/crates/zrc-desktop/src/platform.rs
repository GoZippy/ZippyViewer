//! Platform-specific integration features

use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Platform integration manager
pub struct PlatformIntegration {
    system_tray_enabled: Arc<AtomicBool>,
    notifications_enabled: Arc<AtomicBool>,
    high_dpi_scale: f32,
}

impl PlatformIntegration {
    pub fn new() -> Self {
        Self {
            system_tray_enabled: Arc::new(AtomicBool::new(false)),
            notifications_enabled: Arc::new(AtomicBool::new(true)),
            high_dpi_scale: 1.0,
        }
    }

    /// Initialize platform integration
    pub fn initialize(&mut self, cc: &eframe::CreationContext<'_>) {
        // Enable high-DPI support
        self.setup_high_dpi(cc);
        
        // Setup system tray (platform-specific, would need tray-rs or similar)
        // For now, this is a placeholder
    }

    /// Setup high-DPI support
    fn setup_high_dpi(&mut self, cc: &eframe::CreationContext<'_>) {
        // eframe/egui handles DPI automatically, but we can adjust scale
        let pixels_per_point = cc.egui_ctx.pixels_per_point();
        self.high_dpi_scale = pixels_per_point;
        
        // Adjust UI scale for high-DPI displays
        if pixels_per_point > 1.5 {
            let mut style = (*cc.egui_ctx.style()).clone();
            // Scale fonts and UI elements appropriately
            style.text_styles.values_mut().for_each(|font| {
                font.size *= pixels_per_point;
            });
            cc.egui_ctx.set_style(style);
        }
    }

    /// Enable or disable system tray
    pub fn set_system_tray_enabled(&self, enabled: bool) {
        self.system_tray_enabled.store(enabled, Ordering::Relaxed);
        // TODO: Actually create/destroy system tray icon
    }

    /// Check if system tray is enabled
    pub fn is_system_tray_enabled(&self) -> bool {
        self.system_tray_enabled.load(Ordering::Relaxed)
    }

    /// Enable or disable notifications
    pub fn set_notifications_enabled(&self, enabled: bool) {
        self.notifications_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if notifications are enabled
    pub fn is_notifications_enabled(&self) -> bool {
        self.notifications_enabled.load(Ordering::Relaxed)
    }

    /// Show platform notification
    pub fn show_notification(&self, title: &str, message: &str) {
        if !self.is_notifications_enabled() {
            return;
        }

        // eframe doesn't have built-in notifications, but we can use egui toasts
        // For actual OS notifications, would need platform-specific crates like:
        // - notify-rust (Linux/Windows)
        // - mac-notification-sys (macOS)
        
        // For now, this is a placeholder that would be handled by the UI layer
        tracing::info!("Notification: {} - {}", title, message);
    }

    /// Get high-DPI scale factor
    pub fn high_dpi_scale(&self) -> f32 {
        self.high_dpi_scale
    }

    /// Setup accessibility features
    pub fn setup_accessibility(&self, ctx: &egui::Context) {
        // egui has built-in keyboard navigation support
        // We can enhance it with screen reader support
        
        // Enable keyboard navigation hints
        let style = (*ctx.style()).clone();
        // Ensure interactive elements are keyboard accessible
        // This is mostly handled by egui automatically
        
        ctx.set_style(style);
    }

    /// Register URL scheme handler (zrc://)
    pub fn register_url_scheme(&self) {
        // Platform-specific URL scheme registration
        // Would need platform-specific code:
        // - Windows: Registry entries
        // - macOS: Info.plist entries
        // - Linux: .desktop file entries
        
        tracing::info!("URL scheme registration would be done at build/install time");
    }

    /// Handle URL scheme invocation
    pub fn handle_url(&self, url: &str) -> Result<(), PlatformError> {
        if !url.starts_with("zrc://") {
            return Err(PlatformError::InvalidUrl);
        }

        // Parse zrc:// URLs
        // Format: zrc://action?params
        // Examples:
        //   zrc://connect?device_id=xxx
        //   zrc://pair?invite=xxx
        
        let path = url.strip_prefix("zrc://").unwrap();
        let parts: Vec<&str> = path.split('?').collect();
        let action = parts[0];
        
        match action {
            "connect" => {
                // TODO: Extract device_id and initiate connection
                tracing::info!("URL action: connect");
            }
            "pair" => {
                // TODO: Extract invite and start pairing
                tracing::info!("URL action: pair");
            }
            _ => {
                return Err(PlatformError::UnknownAction(action.to_string()));
            }
        }
        
        Ok(())
    }
}

/// Platform integration errors
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("Invalid URL")]
    InvalidUrl,
    
    #[error("Unknown action: {0}")]
    UnknownAction(String),
    
    #[error("Platform error: {0}")]
    Other(String),
}

/// Accessibility helper functions
pub mod accessibility {
    use eframe::egui;

    /// Ensure UI element is accessible
    pub fn make_accessible(ui: &mut egui::Ui, _label: &str) {
        // egui automatically handles keyboard navigation
        // This function can be used to add additional accessibility hints
        ui.ctx().output_mut(|o| {
            // Add screen reader text if needed
            o.cursor_icon = egui::CursorIcon::PointingHand;
        });
    }

    /// Check if keyboard navigation is active
    pub fn is_keyboard_navigation_active(_ctx: &egui::Context) -> bool {
        // egui tracks keyboard focus automatically
        // Keyboard navigation is always available in egui
        // This function can be extended to check specific focus state if needed
        true
    }

    /// Get accessible label for element
    pub fn get_accessible_label(element: &str) -> String {
        format!("{} - Press Enter to activate", element)
    }
}
