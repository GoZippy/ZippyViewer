use eframe::egui;
use std::sync::Arc;
use crate::settings::Settings;
use crate::ui::UiState;
use crate::device::DeviceManager;
use crate::session::SessionManager;
use crate::platform::PlatformIntegration;
use tokio::sync::mpsc;

use zrc_core::keys::generate_identity_keys;
use zrc_core::store::InMemoryStore;

pub struct ZrcDesktopApp {
    pub settings: Settings,
    pub ui_state: UiState,
    pub device_manager: Arc<DeviceManager>,
    pub session_manager: Arc<SessionManager>,
    pub platform: PlatformIntegration,
    pub runtime: tokio::runtime::Handle,
    _session_event_receiver: Option<mpsc::Receiver<crate::session::SessionEvent>>,
}

impl ZrcDesktopApp {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: tokio::runtime::Handle) -> Self {
        // Load settings or default
        let settings = Settings::load();
        
        // TODO: Load keys/store from disk
        let keys = generate_identity_keys();
        let store = InMemoryStore::new_shared();

        let device_manager = Arc::new(DeviceManager::new());
        
        // Prepare for async load
        let store_clone = store.clone();
        let operator_id = keys.id32.clone(); // IdentityKeys derives Clone, but we can just copy id32 array if needed. keys.id32 is [u8; 32] which is Copy.
        let device_manager_clone = device_manager.clone();

        let mut session_manager = SessionManager::new(keys, store); // Consumes keys and store
        
        // Set up session event channel
        let (tx, rx) = mpsc::channel(100);
        session_manager.set_event_sender(tx);
        let session_manager = Arc::new(session_manager);
        
        // Apply visual settings
        apply_settings(&settings, cc);
        
        // Initialize platform integration
        let mut platform = PlatformIntegration::new();
        platform.initialize(cc);
        platform.setup_accessibility(&cc.egui_ctx);
        runtime.spawn(async move {
            device_manager_clone.load_from_store(store_clone, &operator_id).await;
        });
        
        Self {
            settings,
            ui_state: UiState::default(),
            device_manager,
            session_manager,
            platform,
            runtime,
            _session_event_receiver: Some(rx),
        }
    }

    /// Handle background events (called from update loop)
    fn handle_background_events(&mut self, _ctx: &egui::Context) {
        // Process session events
        if let Some(ref mut rx) = self._session_event_receiver {
            while let Ok(event) = rx.try_recv() {
                match event {
                    crate::session::SessionEvent::Connected { session_id } => {
                        // Open viewer window
                        if let Some(session) = self.session_manager.get_active_session(&session_id) {
                            let viewer = crate::viewer::ViewerWindow::new(session, self.runtime.clone());
                            self.ui_state.viewer_windows.insert(session_id, viewer);
                            self.ui_state.current_view = crate::ui::View::Session(session_id);
                        }
                        crate::ui::add_notification(
                            &mut self.ui_state,
                            format!("Connected to session {:?}", session_id),
                            crate::ui::NotificationLevel::Success,
                        );
                        // Show platform notification
                        self.platform.show_notification(
                            "Zippy Remote Control",
                            &format!("Connected to session {:?}", session_id),
                        );
                    }
                    crate::session::SessionEvent::Disconnected { session_id, reason } => {
                        crate::ui::add_notification(
                            &mut self.ui_state,
                            format!("Disconnected: {}", reason),
                            crate::ui::NotificationLevel::Info,
                        );
                        // Show platform notification
                        self.platform.show_notification(
                            "Zippy Remote Control",
                            &format!("Disconnected: {}", reason),
                        );
                        // Remove viewer window
                        self.ui_state.viewer_windows.remove(&session_id);
                    }
                    crate::session::SessionEvent::QualityChanged { session_id: _, quality: _ } => {
                        // Update connection quality indicator
                    }
                    crate::session::SessionEvent::Error { session_id: _, error } => {
                        let error_msg = error.clone();
                        // Remove connection progress dialog if present
                        self.ui_state.dialogs.retain(|d| {
                            !matches!(d, crate::ui::Dialog::ConnectionProgress { .. })
                        });
                        // Show error dialog
                        self.ui_state.dialogs.push(crate::ui::Dialog::ConnectionError {
                            device_id: "Unknown".to_string(), // TODO: Get from session
                            error: error_msg.clone(),
                        });
                        crate::ui::add_notification(
                            &mut self.ui_state,
                            format!("Connection error: {}", error_msg),
                            crate::ui::NotificationLevel::Error,
                        );
                    }
                }
            }
        }
    }
}

fn apply_settings(settings: &Settings, cc: &eframe::CreationContext<'_>) {
    let style = match settings.theme {
        crate::settings::Theme::System => {
            // Use system theme (default)
            cc.egui_ctx.style().as_ref().clone()
        }
        crate::settings::Theme::Light => {
            let mut style = (*cc.egui_ctx.style()).clone();
            style.visuals = egui::Visuals::light();
            style
        }
        crate::settings::Theme::Dark => {
            let mut style = (*cc.egui_ctx.style()).clone();
            style.visuals = egui::Visuals::dark();
            style
        }
    };
    cc.egui_ctx.set_style(style);
}

impl eframe::App for ZrcDesktopApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Handle background events
        self.handle_background_events(ctx);
        
        // Render UI
        crate::ui::render_ui(self, ctx, frame);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.settings.save();
    }
}
