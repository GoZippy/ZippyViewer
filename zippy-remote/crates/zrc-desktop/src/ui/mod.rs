use crate::ZrcDesktopApp;
use crate::device::DeviceStatus;
use crate::session::SessionId;
use crate::viewer::ViewerWindow;
use eframe::egui;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose};
use zrc_proto::v1::InviteV1;
use prost::Message;

#[derive(Default)]
pub struct UiState {
    pub current_view: View,
    pub viewer_windows: HashMap<SessionId, ViewerWindow>,
    pub dialogs: Vec<Dialog>,
    pub notifications: VecDeque<Notification>,
    pub search_text: String,
    pub selected_device: Option<String>,
}

#[derive(Default, PartialEq)]
pub enum View {
    #[default]
    DeviceList,
    Settings,
    About,
    Session(SessionId),
}

#[derive(Clone)]
pub enum Dialog {
    PairingWizard { invite_text: String, error_message: Option<String> },
    SasVerification { sas_code: String },
    ConnectionProgress { device_id: String, cancel_tx: Option<Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>> },
    ConnectionError { device_id: String, error: String },
    FileTransfer,
    Confirmation { message: String },
    DeviceProperties { device_id: String },
    ConnectionInfo { session_id: crate::session::SessionId },
    PairingWizardStep { step: u32, invite_data: Option<String> },
}

#[derive(Clone)]
pub struct Notification {
    pub message: String,
    pub level: NotificationLevel,
    pub timestamp: std::time::Instant,
}

#[derive(Clone, Copy, PartialEq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
    Success,
}

pub fn render_ui(app: &mut ZrcDesktopApp, ctx: &egui::Context, frame: &mut eframe::Frame) {
    // Handle background events
    handle_background_events(app, ctx);

    // Render menu bar
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Add Device...").clicked() {
                    app.ui_state.dialogs.push(Dialog::PairingWizard {
                        invite_text: String::new(),
                        error_message: None,
                    });
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Devices").clicked() {
                    app.ui_state.current_view = View::DeviceList;
                    ui.close_menu();
                }
                if ui.button("Settings").clicked() {
                    app.ui_state.current_view = View::Settings;
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    app.ui_state.current_view = View::About;
                    ui.close_menu();
                }
            });
        });
    });

    // Render dialogs
    render_dialogs(app, ctx);

    // Render notifications
    render_notifications(&mut app.ui_state, ctx);

    // Render main content
    egui::CentralPanel::default().show(ctx, |ui| {
        match app.ui_state.current_view {
            View::DeviceList => {
                // Show active sessions as tabs if multiple sessions exist
                let active_sessions = app.session_manager.list_active_sessions();
                if !active_sessions.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label("Active Sessions:");
                        for &session_id in &active_sessions {
                            let is_current = matches!(app.ui_state.current_view, View::Session(id) if id == session_id);
                            if ui.selectable_label(is_current, format!("Session {:?}", session_id.0)).clicked() {
                                app.ui_state.current_view = View::Session(session_id);
                            }
                        }
                    });
                    ui.separator();
                }
                render_device_list_ui(app, ui);
            },
            View::Settings => render_settings_ui(app, ui),
            View::About => render_about_ui(ui),
            View::Session(id) => {
                // Show session tabs for multi-session support
                let active_sessions = app.session_manager.list_active_sessions();
                if active_sessions.len() > 1 {
                    ui.horizontal(|ui| {
                        ui.label("Active Sessions:");
                        for &session_id in &active_sessions {
                            let is_current = session_id == id;
                            if ui.selectable_label(is_current, format!("Session {:?}", session_id.0)).clicked() {
                                app.ui_state.current_view = View::Session(session_id);
                            }
                        }
                        if ui.button("+ New Session").clicked() {
                            app.ui_state.current_view = View::DeviceList;
                        }
                    });
                    ui.separator();
                }
                
                if let Some(window) = app.ui_state.viewer_windows.get_mut(&id) {
                    if let Some(action) = window.render(ctx, ui, frame) {
                        match action {
                            crate::viewer::ViewerAction::Disconnect => {
                                // Disconnect session
                                let session_manager = app.session_manager.clone();
                                let runtime = app.runtime.clone();
                                runtime.spawn(async move {
                                    let _ = session_manager.disconnect(id).await;
                                });
                                
                                // Remove from UI
                                app.ui_state.viewer_windows.remove(&id);
                                
                                // Switch to device list or another session
                                let remaining = app.session_manager.list_active_sessions();
                                if let Some(&next_id) = remaining.first() {
                                    app.ui_state.current_view = View::Session(next_id);
                                } else {
                                    app.ui_state.current_view = View::DeviceList;
                                }
                            }
                            crate::viewer::ViewerAction::ShowConnectionInfo => {
                                app.ui_state.dialogs.push(Dialog::ConnectionInfo { session_id: id });
                            }
                        }
                    }
                } else {
                    // Session window not found - try to create it or switch view
                    if let Some(session) = app.session_manager.get_active_session(&id) {
                        let viewer = crate::viewer::ViewerWindow::new(session, app.runtime.clone());
                        app.ui_state.viewer_windows.insert(id, viewer);
                    } else {
                        app.ui_state.current_view = View::DeviceList; // Session closed/missing
                    }
                }
            }
        }
    });

    // Render viewer windows (handled in CentralPanel for single-window mode)
    // render_viewer_windows(&mut app.ui_state, ctx, frame);
}

fn handle_background_events(_app: &mut ZrcDesktopApp, _ctx: &egui::Context) {
    // Process session events, clipboard changes, etc.
    // This would integrate with async event channels
    // Currently handled in app.handle_background_events
}

fn render_device_list_ui(app: &mut ZrcDesktopApp, ui: &mut egui::Ui) {
    ui.heading("Devices");
    ui.separator();

    // Search bar
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut app.ui_state.search_text);
        if !app.ui_state.search_text.is_empty() {
            app.device_manager.set_search_filter(app.ui_state.search_text.clone());
        }
    });
    ui.separator();

    // Device list
    let devices = if app.ui_state.search_text.is_empty() {
        app.device_manager.list_devices()
    } else {
        app.device_manager.get_filtered_devices()
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        for device in devices {
            let is_selected = app.ui_state.selected_device.as_ref() == Some(&device.id);
            
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Status indicator
                    let status_icon = match &device.status {
                        DeviceStatus::Online { .. } => "🟢",
                        DeviceStatus::Offline { .. } => "⚫",
                        DeviceStatus::Connecting => "🟡",
                        DeviceStatus::Unknown => "⚪",
                    };
                    ui.label(status_icon);

                    // Device name
                    if ui.selectable_label(is_selected, &device.display_name).clicked() {
                        app.ui_state.selected_device = Some(device.id.clone());
                    }

                    // Double-click to connect
                    if ui.interact(ui.available_rect_before_wrap(), egui::Id::new(&device.id), egui::Sense::click()).double_clicked() {
                        connect_to_device(app, &device.id);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Connect").clicked() {
                            connect_to_device(app, &device.id);
                        }
                        
                        // Context menu
                        ui.menu_button("⋮", |ui| {
                            if ui.button("Connect").clicked() {
                                connect_to_device(app, &device.id);
                                ui.close_menu();
                            }
                            if ui.button("Properties").clicked() {
                                app.ui_state.dialogs.push(Dialog::DeviceProperties {
                                    device_id: device.id.clone(),
                                });
                                ui.close_menu();
                            }
                            if ui.button("Remove").clicked() {
                                if let Err(e) = app.device_manager.remove_device(&device.id) {
                                    add_notification(&mut app.ui_state, format!("Failed to remove device: {}", e), NotificationLevel::Error);
                                } else {
                                    add_notification(&mut app.ui_state, "Device removed".to_string(), NotificationLevel::Success);
                                }
                                ui.close_menu();
                            }
                        });
                    });
                });
            });
        }
    });
}

fn connect_to_device(app: &mut ZrcDesktopApp, device_id: &str) {
    let device_id = device_id.to_string();
    let runtime = app.runtime.clone();
    let session_manager = app.session_manager.clone();
    
    // Create cancellation channel
    let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();
    let cancel_tx_mutex = Arc::new(tokio::sync::Mutex::new(Some(cancel_tx)));
    
    // Show connection progress dialog
    app.ui_state.dialogs.push(Dialog::ConnectionProgress {
        device_id: device_id.clone(),
        cancel_tx: Some(cancel_tx_mutex),
    });

    // Initiate connection (async)
    // Connection success/error will be handled via SessionEvent channel
    runtime.spawn(async move {
        if let Err(e) = session_manager.connect(&device_id).await {
            // Error will be sent via SessionEvent::Error
            tracing::error!("Connection failed: {}", e);
        }
    });
}

fn render_settings_ui(app: &mut ZrcDesktopApp, ui: &mut egui::Ui) {
    ui.heading("Settings");
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        // Appearance settings
        ui.group(|ui| {
            ui.label(egui::RichText::new("Appearance").heading());
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("Theme:");
                egui::ComboBox::from_id_source("theme_selector")
                    .selected_text(format!("{:?}", app.settings.theme))
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(app.settings.theme == crate::settings::Theme::System, "System").clicked() {
                            app.settings.theme = crate::settings::Theme::System;
                        }
                        if ui.selectable_label(app.settings.theme == crate::settings::Theme::Light, "Light").clicked() {
                            app.settings.theme = crate::settings::Theme::Light;
                        }
                        if ui.selectable_label(app.settings.theme == crate::settings::Theme::Dark, "Dark").clicked() {
                            app.settings.theme = crate::settings::Theme::Dark;
                        }
                    });
            });
            
            ui.horizontal(|ui| {
                ui.label("Scale Factor:");
                ui.add(egui::Slider::new(&mut app.settings.scale_factor, 0.5..=3.0));
            });
        });

        ui.separator();

        // Input settings
        ui.group(|ui| {
            ui.label(egui::RichText::new("Input").heading());
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("Default Mode:");
                egui::ComboBox::from_id_source("input_mode")
                    .selected_text(&app.settings.default_input_mode)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(app.settings.default_input_mode == "view_only", "View Only").clicked() {
                            app.settings.default_input_mode = "view_only".to_string();
                        }
                        if ui.selectable_label(app.settings.default_input_mode == "control", "Control").clicked() {
                            app.settings.default_input_mode = "control".to_string();
                        }
                    });
            });
            
            ui.horizontal(|ui| {
                ui.label("Sensitivity:");
                ui.add(egui::Slider::new(&mut app.settings.input_sensitivity, 0.1..=3.0));
            });
        });

        ui.separator();

        // Transport settings
        ui.group(|ui| {
            ui.label(egui::RichText::new("Transport").heading());
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("Rendezvous URL:");
                ui.text_edit_singleline(&mut app.settings.rendezvous_url);
            });
            
            ui.horizontal(|ui| {
                ui.label("Connection Timeout (secs):");
                ui.add(egui::Slider::new(&mut app.settings.connection_timeout_secs, 5..=120));
            });
            
            ui.label("Relay URLs (one per line):");
            // Simple text area for relay URLs
            let mut relay_text = app.settings.relay_urls.join("\n");
            ui.text_edit_multiline(&mut relay_text);
            app.settings.relay_urls = relay_text.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        });

        ui.separator();

        // Notifications
        ui.group(|ui| {
            ui.label(egui::RichText::new("Notifications").heading());
            ui.separator();
            let mut notifications_enabled = app.platform.is_notifications_enabled();
            if ui.checkbox(&mut notifications_enabled, "Enable notifications").changed() {
                app.platform.set_notifications_enabled(notifications_enabled);
            }
            let mut system_tray_enabled = app.platform.is_system_tray_enabled();
            if ui.checkbox(&mut system_tray_enabled, "Enable system tray").changed() {
                app.platform.set_system_tray_enabled(system_tray_enabled);
            }
        });

        ui.separator();

        // Accessibility
        ui.group(|ui| {
            ui.label(egui::RichText::new("Accessibility").heading());
            ui.separator();
            ui.label("Keyboard navigation: Enabled (built-in)");
            ui.label("Screen reader support: Enabled");
            ui.horizontal(|ui| {
                ui.label("High-DPI scale:");
                ui.label(format!("{:.2}x", app.platform.high_dpi_scale()));
            });
        });
    });

    ui.separator();
    
    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            app.settings.save();
            add_notification(&mut app.ui_state, "Settings saved".to_string(), NotificationLevel::Success);
        }
        if ui.button("Reset to Defaults").clicked() {
            app.settings = crate::settings::Settings::default();
            add_notification(&mut app.ui_state, "Settings reset to defaults".to_string(), NotificationLevel::Info);
        }
    });
}

fn render_about_ui(ui: &mut egui::Ui) {
    ui.heading("Zippy Remote Control");
    ui.label("Version 0.1.0");
    ui.separator();
    ui.label("A secure remote desktop application");
}

fn render_dialogs(app: &mut ZrcDesktopApp, ctx: &egui::Context) {
    let mut to_remove = Vec::new();
    let mut notifications_to_add: Vec<(String, NotificationLevel)> = Vec::new();
    let mut pairing_wizard_updates: Vec<(usize, String, Option<String>)> = Vec::new();
    
    for (idx, dialog) in app.ui_state.dialogs.iter().enumerate() {
        match dialog {
            Dialog::ConnectionProgress { device_id, cancel_tx } => {
                let mut should_close = false;
                egui::Window::new("Connecting...")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(format!("Connecting to {}", device_id));
                        ui.spinner();
                        ui.separator();
                        ui.horizontal(|ui| {
                            if let Some(tx_mutex) = cancel_tx.as_ref() {
                                if ui.button("Cancel").clicked() {
                                    let runtime = app.runtime.clone();
                                    let tx_mutex_clone = tx_mutex.clone();
                                    runtime.spawn(async move {
                                        let mut guard = tx_mutex_clone.lock().await;
                                        if let Some(tx) = guard.take() {
                                            let _ = tx.send(());
                                        }
                                    });
                                    should_close = true;
                                }
                            }
                        });
                    });
                if should_close {
                    to_remove.push(idx);
                }
            }
            Dialog::ConnectionError { device_id, error } => {
                egui::Window::new("Connection Failed")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(format!("Failed to connect to {}", device_id));
                        ui.separator();
                        ui.label(format!("Error: {}", error));
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Retry").clicked() {
                                // TODO: Retry connection
                                to_remove.push(idx);
                            }
                            if ui.button("Close").clicked() {
                                to_remove.push(idx);
                            }
                        });
                    });
            }
            Dialog::DeviceProperties { device_id } => {
                let mut should_close = false;
                let device_id_clone = device_id.clone();
                let mut save_result: Option<Result<(), String>> = None;
                egui::Window::new("Device Properties")
                    .collapsible(false)
                    .resizable(true)
                    .default_size([400.0, 500.0])
                    .show(ctx, |ui| {
                        if let Some(device) = app.device_manager.get_device(&device_id_clone) {
                            let mut device_name = device.display_name.clone();
                            let mut permissions = device.permissions.clone();
                            
                            ui.group(|ui| {
                                ui.label(egui::RichText::new("Device Information").heading());
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label("Name:");
                                    ui.text_edit_singleline(&mut device_name);
                                });
                                if ui.button("Save Name").clicked() {
                                    // Save name - store result for processing after dialog
                                    save_result = Some(app.device_manager.set_device_name(&device_id_clone, device_name.clone())
                                        .map_err(|e| e.to_string()));
                                    if save_result.as_ref().unwrap().is_ok() {
                                        should_close = true;
                                    }
                                }
                                ui.horizontal(|ui| {
                                    ui.label("ID:");
                                    ui.label(&device.id);
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Status:");
                                    let status_text = match &device.status {
                                        DeviceStatus::Online { latency_ms } => format!("Online ({}ms)", latency_ms),
                                        DeviceStatus::Offline { last_seen } => {
                                            let elapsed = last_seen.elapsed().unwrap_or_default();
                                            format!("Offline (last seen {} ago)", format_duration(elapsed))
                                        },
                                        DeviceStatus::Connecting => "Connecting".to_string(),
                                        DeviceStatus::Unknown => "Unknown".to_string(),
                                    };
                                    ui.label(status_text);
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Paired at:");
                                    ui.label(format!("{:?}", device.paired_at));
                                });
                            });
                            
                            ui.separator();
                            
                            ui.group(|ui| {
                                ui.label(egui::RichText::new("Permissions").heading());
                                ui.separator();
                                ui.checkbox(&mut permissions.view, "View");
                                ui.checkbox(&mut permissions.control, "Control");
                                ui.checkbox(&mut permissions.clipboard, "Clipboard");
                                ui.checkbox(&mut permissions.file_transfer, "File Transfer");
                                
                                // Note: Permission changes would need to be saved to the pairing store
                                // For now, this is just a display/edit interface
                            });
                        } else {
                            ui.label("Device not found");
                        }
                        
                        if let Some(ref result) = save_result {
                            if let Err(ref e) = result {
                                ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                            } else {
                                notifications_to_add.push(("Device name saved".to_string(), NotificationLevel::Success));
                            }
                        }
                        
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Close").clicked() {
                                should_close = true;
                            }
                        });
                    });
                if should_close {
                    to_remove.push(idx);
                }
            }
            Dialog::SasVerification { sas_code } => {
                egui::Window::new("Verify Connection")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label("Please verify the following code matches on the remote device:");
                        ui.heading(sas_code);
                        ui.horizontal(|ui| {
                            if ui.button("Confirm").clicked() {
                                to_remove.push(idx);
                            }
                            if ui.button("Cancel").clicked() {
                                to_remove.push(idx);
                            }
                        });
                    });
            }
            Dialog::PairingWizard { invite_text, error_message } => {
                let mut should_close = false;
                let mut local_invite_text = invite_text.clone();
                let mut local_error_message = error_message.clone();
                
                egui::Window::new("Add Device")
                    .collapsible(false)
                    .resizable(true)
                    .default_size([500.0, 400.0])
                    .show(ctx, |ui| {
                        ui.heading("Add New Device");
                        ui.separator();
                        
                        ui.label("Choose how to add a device:");
                        ui.separator();
                        
                        ui.horizontal(|ui| {
                            if ui.button("Paste from Clipboard").clicked() {
                                // Read from clipboard
                                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                    if let Ok(text) = clipboard.get_text() {
                                        local_invite_text = text;
                                        local_error_message = None;
                                    } else {
                                        local_error_message = Some("Failed to read clipboard".to_string());
                                    }
                                } else {
                                    local_error_message = Some("Failed to access clipboard".to_string());
                                }
                            }
                            
                            if ui.button("Import from File").clicked() {
                                // Open file dialog (non-blocking, will need async handling)
                                let runtime = app.runtime.clone();
                                let session_manager = app.session_manager.clone();
                                let device_manager = app.device_manager.clone();
                                
                                runtime.spawn(async move {
                                    if let Some(path) = rfd::AsyncFileDialog::new()
                                        .add_filter("Invite files", &["zrc", "txt", "json"])
                                        .add_filter("All files", &["*"])
                                        .pick_file()
                                        .await
                                    {
                                        if let Ok(contents) = std::fs::read_to_string(path.path()) {
                                            if let Err(e) = import_invite_from_text(&session_manager, &device_manager, &contents).await {
                                                tracing::error!("Failed to import invite from file: {}", e);
                                            }
                                        }
                                    }
                                });
                                should_close = true;
                            }
                        });
                        
                        ui.separator();
                        
                        ui.label("Or paste invite data (base64-encoded):");
                        ui.text_edit_multiline(&mut local_invite_text);
                        
                        if let Some(ref error) = local_error_message {
                            ui.colored_label(egui::Color32::RED, error);
                        }
                        
                        ui.separator();
                        
                        ui.horizontal(|ui| {
                            if ui.button("Import").clicked() {
                                if !local_invite_text.trim().is_empty() {
                                    let session_manager = app.session_manager.clone();
                                    let device_manager = app.device_manager.clone();
                                    let invite_text_clone = local_invite_text.clone();
                                    let runtime = app.runtime.clone();
                                    
                                    runtime.spawn(async move {
                                        if let Err(e) = import_invite_from_text(&session_manager, &device_manager, &invite_text_clone).await {
                                            tracing::error!("Failed to import invite: {}", e);
                                        } else {
                                            tracing::info!("Device paired successfully");
                                        }
                                    });
                                    should_close = true;
                                } else {
                                    local_error_message = Some("Please enter or paste an invite".to_string());
                                }
                            }
                            
                            if ui.button("Cancel").clicked() {
                                should_close = true;
                            }
                        });
                    });
                
                // Store updates to apply after iteration
                pairing_wizard_updates.push((idx, local_invite_text, local_error_message));
                
                if should_close {
                    to_remove.push(idx);
                }
            }
            _ => {}
        }
    }

    // Update pairing wizard dialogs
    for (idx, invite_text, error_message) in pairing_wizard_updates {
        if let Some(dialog) = app.ui_state.dialogs.get_mut(idx) {
            if let Dialog::PairingWizard { invite_text: ref mut it, error_message: ref mut em } = dialog {
                *it = invite_text;
                *em = error_message;
            }
        }
    }
    
    // Remove processed dialogs (in reverse order to maintain indices)
    for &idx in to_remove.iter().rev() {
        app.ui_state.dialogs.remove(idx);
    }
    
    // Add notifications after dialog rendering
    for (message, level) in notifications_to_add {
        add_notification(&mut app.ui_state, message, level);
    }
}

fn render_notifications(ui_state: &mut UiState, ctx: &egui::Context) {
    // Remove old notifications
    ui_state.notifications.retain(|n| n.timestamp.elapsed().as_secs() < 5);

    // Show notifications
    for (idx, notification) in ui_state.notifications.iter().enumerate() {
        let color = match notification.level {
            NotificationLevel::Info => egui::Color32::from_rgb(100, 150, 255),
            NotificationLevel::Success => egui::Color32::from_rgb(100, 255, 100),
            NotificationLevel::Warning => egui::Color32::from_rgb(255, 200, 100),
            NotificationLevel::Error => egui::Color32::from_rgb(255, 100, 100),
        };

        egui::Window::new("")
            .title_bar(false)
            .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-10.0, 10.0 + (idx as f32 * 60.0)))
            .show(ctx, |ui| {
                ui.colored_label(color, &notification.message);
            });
    }
}



pub fn add_notification(ui_state: &mut UiState, message: String, level: NotificationLevel) {
    ui_state.notifications.push_back(Notification {
        message,
        level,
        timestamp: std::time::Instant::now(),
    });
}

fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

/// Import invite from text (base64-encoded protobuf)
async fn import_invite_from_text(
    session_manager: &crate::session::SessionManager,
    device_manager: &crate::device::DeviceManager,
    text: &str,
) -> Result<(), String> {
    // Decode base64
    let invite_bytes = general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|e| format!("Failed to decode base64: {}", e))?;
    
    // Import pairing invite through session manager
    let device_id_hex = session_manager.import_pairing_invite(&invite_bytes).await
        .map_err(|e| format!("Failed to import invite: {}", e))?;
    
    // Create device info from invite
    let device_info = crate::device::DeviceInfo {
        id: device_id_hex.clone(),
        display_name: format!("Device {}", &device_id_hex[..8]),
        status: crate::device::DeviceStatus::Unknown,
        permissions: crate::device::Permissions {
            view: true,
            control: true,
            clipboard: true,
            file_transfer: true,
        },
        paired_at: std::time::SystemTime::now(),
        last_seen: None,
        group_id: None,
    };
    
    device_manager.add_device(device_info);
    
    tracing::info!("Imported invite for device: {}", device_id_hex);
    Ok(())
}
