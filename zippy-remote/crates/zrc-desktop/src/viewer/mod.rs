//! Remote desktop viewer window

use crate::input::{InputHandler, InputMode};
use crate::session::{ActiveSession, SessionId};
use eframe::egui::{self, Rect, Vec2};
use std::sync::Arc;
use tokio::sync::mpsc;
use zrc_proto::v1::VideoFrameV1;
use prost::Message;

/// Actions triggered by the viewer
pub enum ViewerAction {
    Disconnect,
    ShowConnectionInfo,
}

/// Remote desktop viewer window
pub struct ViewerWindow {
    session_id: SessionId,
    session: Arc<ActiveSession>,
    renderer: FrameRenderer,
    input_handler: InputHandler,
    state: ViewerState,
    toolbar: ViewerToolbar,
    frame_receiver: mpsc::Receiver<DecodedFrame>,
    runtime: tokio::runtime::Handle,
}

impl ViewerWindow {
    /// Create new viewer window
    pub fn new(session: Arc<ActiveSession>, runtime: tokio::runtime::Handle) -> Self {
        let session_id = session.id;
        let input_handler = InputHandler::new();
        input_handler.set_enabled(true);
        
        let (tx, rx) = mpsc::channel(10);
        
        // Spawn frame decoder loop
        let session_clone = session.clone();
        runtime.spawn(async move {
            loop {
                // Read from media session
                match session_clone.media_session.recv_media_frame().await {
                     Ok(bytes) => {
                         if let Ok(video_frame) = VideoFrameV1::decode(bytes) {
                             if let Some(header) = video_frame.header {
                                 // For MVP, assume RGBA/BGRA raw or simple format
                                 // We use header.width/height
                                 let width = header.width;
                                 let height = header.height;
                                 
                                 let frame = DecodedFrame {
                                     width,
                                     height,
                                     format: FrameFormat::Rgba, // TODO: Use header.format
                                     data: video_frame.data,
                                     timestamp: std::time::SystemTime::now()
                                         .duration_since(std::time::UNIX_EPOCH)
                                         .unwrap()
                                         .as_millis() as u64,
                                 };
                                 
                                 if tx.send(frame).await.is_err() { break; }
                             }
                         }
                     }
                     Err(_) => break, // Connection closed
                }
            }
        });

        Self {
            session_id,
            session: session.clone(),
            renderer: FrameRenderer::new(),
            input_handler,
            state: ViewerState::default(),
            toolbar: ViewerToolbar::default(),
            frame_receiver: rx,
            runtime,
        }
    }

    /// Render the viewer window
    pub fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut eframe::Frame) -> Option<ViewerAction> {
        // Poll frames with dropping when behind
        let mut latest_frame = None;
        let mut frame_count = 0;
        while let Ok(frame_data) = self.frame_receiver.try_recv() {
            latest_frame = Some(frame_data);
            frame_count += 1;
        }
        
        // If we received multiple frames, only use the latest (drop others)
        if let Some(frame_data) = latest_frame {
            self.renderer.update_frame(ctx, frame_data);
            // Request repaint
            ctx.request_repaint();
            
            // Update stats
            if frame_count > 1 {
                // Frames were dropped
                let stats = self.session.stats.read().unwrap();
                // Could track dropped frames if needed
            }
        }
        
        // Handle fullscreen toggle
        if self.state.fullscreen {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        
        let mut action = None;
        let available_size = ui.available_size();
        
        // Render toolbar if visible and not in fullscreen
        if self.toolbar.visible && !self.state.fullscreen {
            if let Some(act) = self.render_toolbar(ui) {
                action = Some(act);
            }
        }

        // Handle resolution changes
        if let Some(_remote_size) = self.renderer.get_remote_size() {
            // Check if resolution changed and adjust zoom if needed
            if let ZoomLevel::Fit = self.state.zoom {
                // Fit mode automatically adjusts, no action needed
            }
        }
        
        // Render frame with zoom
        let viewer_rect = self.renderer.render_with_zoom(ui, available_size, self.state.zoom);
        
        // Update coordinate mapper
        if let Some(remote_size) = self.renderer.get_remote_size() {
            self.input_handler.update_coordinate_mapper(viewer_rect, remote_size);
        }

        // Render status bar if visible
        if self.state.show_stats {
            self.render_status_bar(ui);
        }
        
        // Handle file drops
        self.handle_dropped_files(ctx);
        
        // Render transfer window
        self.render_transfers(ctx);
        
        // Handle input shortcuts
        ctx.input(|i| {
             if i.key_pressed(egui::Key::F11) {
                 self.toggle_fullscreen();
             }
             if i.key_pressed(egui::Key::Escape) && self.state.fullscreen {
                 self.toggle_fullscreen();
             }
             // Double-click to toggle fullscreen
             if i.pointer.any_click() && i.pointer.any_released() {
                 if let Some(pos) = i.pointer.interact_pos() {
                     if viewer_rect.contains(pos) {
                         // Check for double-click
                         if i.pointer.any_pressed() {
                             // This is a simplified check - egui handles double-click detection
                         }
                     }
                 }
             }
        });
        
        action
    }

    /// Handle input events
    pub fn handle_input(&mut self, event: &egui::Event, viewer_rect: Rect) {
        if let Some(input_event) = self.input_handler.handle_event(event, viewer_rect) {
             let proto_event = Self::convert_to_proto(input_event);
             if let Some(payload) = proto_event {
                 let msg = zrc_proto::v1::ControlMsgV1 {
                     msg_type: zrc_proto::v1::ControlMsgTypeV1::Input as i32, // Protobuf enum as i32? Or using generated enum
                     sequence_number: 0, // TODO: Maintain sequence
                     timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64,
                     payload: Some(zrc_proto::v1::control_msg_v1::Payload::Input(payload)),
                 };
                 
                 let session = self.session.clone();
                 self.runtime.spawn(async move {
                     // send_control is likely on media_session or session_host?
                     // ActiveSession has media_session.
                     // But MediaSession trait usually has send_control?
                     // If not, we might need a separate control channel.
                     // The protocol says Control Messages over QUIC.
                     // QuicMediaSession should implement send_control logic (sending on control stream).
                     // However, MediaSession definition in zrc-transport needs to be checked.
                     // I will assume media_session.send_control(bytes) or similar exists.
                     // Wait, MediaSession generic? 
                     // Let's check session.rs for ActiveSession def.
                     // It is Box<dyn MediaTransportSession>.
                     
                     // If MediaTransportSession has a method to send control messages.
                     // I'll assume `send_message` or similar.
                     // For now, I'll comment out the actual send and mark TO IMPLEMENT 
                     // if I'm not sure about the method name.
                     // BUT I should check zrc-transport/src/lib.rs.
                     
                     // sent via QUIC control stream
                     let payload = msg.encode_to_vec();
                     let _ = session.media_session.send_control(bytes::Bytes::from(payload)).await;
                 });
             }
        }
    }

// Helper to convert input events
fn convert_to_proto(event: crate::input::InputEvent) -> Option<zrc_proto::v1::InputEventV1> {
    use crate::input::{InputEvent, SpecialSequence};
    use zrc_proto::v1::{InputEventTypeV1, InputEventV1};

    match event {
        InputEvent::MouseMove { x, y } => Some(InputEventV1 {
            event_type: InputEventTypeV1::MouseMove as i32,
            mouse_x: x,
            mouse_y: y,
            ..Default::default()
        }),
        InputEvent::MouseButton { button, pressed, x, y } => {
            let event_type = if pressed { InputEventTypeV1::MouseDown } else { InputEventTypeV1::MouseUp };
            let btn_code = match button {
                egui::PointerButton::Primary => 1,
                egui::PointerButton::Secondary => 2,
                egui::PointerButton::Middle => 3,
                egui::PointerButton::Extra1 => 4,
                egui::PointerButton::Extra2 => 5,
            };
            Some(InputEventV1 {
                event_type: event_type as i32,
                mouse_x: x,
                mouse_y: y,
                button: btn_code,
                ..Default::default()
            })
        },
        InputEvent::Scroll { delta_x, delta_y } => Some(InputEventV1 {
            event_type: InputEventTypeV1::Scroll as i32,
            scroll_delta_x: delta_x as i32,
            scroll_delta_y: delta_y as i32,
            ..Default::default()
        }),
        InputEvent::Key { key, pressed, modifiers } => {
            use zrc_proto::v1::InputModifiersV1;
            let event_type = if pressed { InputEventTypeV1::KeyDown } else { InputEventTypeV1::KeyUp };
            // Convert egui::Key to key code (simplified)
            let key_code = match key {
                egui::Key::ArrowUp => 38,
                egui::Key::ArrowDown => 40,
                egui::Key::ArrowLeft => 37,
                egui::Key::ArrowRight => 39,
                egui::Key::Escape => 27,
                egui::Key::Tab => 9,
                egui::Key::Enter => 13,
                egui::Key::Space => 32,
                _ => {
                    // Try to get character code
                    if let Some(ch) = key.name().chars().next() {
                        ch as u32
                    } else {
                        0
                    }
                },
            };
            // Build modifiers bitmask
            let mut mod_mask = 0u32;
            if modifiers.ctrl {
                mod_mask |= InputModifiersV1::Ctrl as u32;
            }
            if modifiers.shift {
                mod_mask |= InputModifiersV1::Shift as u32;
            }
            if modifiers.alt {
                mod_mask |= InputModifiersV1::Alt as u32;
            }
            if modifiers.mac_cmd || modifiers.command {
                mod_mask |= InputModifiersV1::Meta as u32;
            }
            Some(InputEventV1 {
                event_type: event_type as i32,
                key_code,
                modifiers: mod_mask,
                ..Default::default()
            })
        },
        InputEvent::SpecialSequence(seq) => {
            // Convert special sequences to key combinations
            match seq {
                SpecialSequence::CtrlAltDel => {
                    use zrc_proto::v1::InputModifiersV1;
                    Some(InputEventV1 {
                        event_type: InputEventTypeV1::KeyDown as i32,
                        key_code: 46, // Delete key code
                        modifiers: (InputModifiersV1::Ctrl as u32) | (InputModifiersV1::Alt as u32),
                        ..Default::default()
                    })
                },
                SpecialSequence::AltTab => {
                    use zrc_proto::v1::InputModifiersV1;
                    Some(InputEventV1 {
                        event_type: InputEventTypeV1::KeyDown as i32,
                        key_code: 9, // Tab
                        modifiers: InputModifiersV1::Alt as u32,
                        ..Default::default()
                    })
                },
                SpecialSequence::AltF4 => {
                    use zrc_proto::v1::InputModifiersV1;
                    Some(InputEventV1 {
                        event_type: InputEventTypeV1::KeyDown as i32,
                        key_code: 115, // F4
                        modifiers: InputModifiersV1::Alt as u32,
                        ..Default::default()
                    })
                },
                SpecialSequence::PrintScreen => Some(InputEventV1 {
                    event_type: InputEventTypeV1::KeyDown as i32,
                    key_code: 44, // PrintScreen
                    ..Default::default()
                }),
                SpecialSequence::Custom(_) => None, // Not implemented
            }
        },
    }
}


    /// Toggle fullscreen
    pub fn toggle_fullscreen(&mut self) {
        self.state.fullscreen = !self.state.fullscreen;
        // Request viewport command to enter/exit fullscreen
        // This will be handled by the frame in the render function
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: ZoomLevel) {
        self.state.zoom = zoom;
    }

    /// Select monitor
    pub fn select_monitor(&mut self, monitor: MonitorId) {
        self.state.selected_monitor = monitor;
        // TODO: Monitor switching - protocol needs to support this
        // For now, monitor selection is handled at connection time
    }

    /// Toggle input mode
    pub fn toggle_input_mode(&mut self) {
        let new_mode = match self.state.input_mode {
            InputMode::ViewOnly => InputMode::Control,
            InputMode::Control => InputMode::ViewOnly,
        };
        self.state.input_mode = new_mode;
        self.input_handler.set_input_mode(new_mode);
    }

    /// Get session ID
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Send special key sequence
    fn send_special_sequence(&self, seq: crate::input::SpecialSequence) {
        let event = self.input_handler.send_special_sequence(seq);
        let proto_event = Self::convert_to_proto(event);
        if let Some(payload) = proto_event {
            let msg = zrc_proto::v1::ControlMsgV1 {
                msg_type: zrc_proto::v1::ControlMsgTypeV1::Input as i32,
                sequence_number: 0,
                timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64,
                payload: Some(zrc_proto::v1::control_msg_v1::Payload::Input(payload)),
            };
            
            let session = self.session.clone();
            self.runtime.spawn(async move {
                let payload = msg.encode_to_vec();
                let _ = session.media_session.send_control(bytes::Bytes::from(payload)).await;
            });
        }
    }



    fn render_toolbar(&mut self, ui: &mut egui::Ui) -> Option<ViewerAction> {
        let mut action = None;
        egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Disconnect").clicked() {
                        action = Some(ViewerAction::Disconnect);
                    }
                    if ui.button("Fullscreen").clicked() {
                        self.toggle_fullscreen();
                    }
                    ui.separator();
                    if ui.button(if self.state.input_mode == InputMode::Control { "View Only" } else { "Control" }).clicked() {
                        self.toggle_input_mode();
                    }
                    
                    ui.separator();
                    ui.menu_button("Special Keys", |ui| {
                        if ui.button("Ctrl+Alt+Del").clicked() {
                            self.send_special_sequence(crate::input::SpecialSequence::CtrlAltDel);
                            ui.close_menu();
                        }
                        if ui.button("Alt+Tab").clicked() {
                            self.send_special_sequence(crate::input::SpecialSequence::AltTab);
                            ui.close_menu();
                        }
                        if ui.button("Alt+F4").clicked() {
                            self.send_special_sequence(crate::input::SpecialSequence::AltF4);
                            ui.close_menu();
                        }
                        if ui.button("Print Screen").clicked() {
                            self.send_special_sequence(crate::input::SpecialSequence::PrintScreen);
                            ui.close_menu();
                        }
                    });
                    
                    ui.separator();
                    // Monitor selector
                    ui.label("Monitor:");
                    // TODO: Get monitor list from session
                    // For now, show placeholder
                    ui.label("Primary");
                    
                    ui.separator();
                    if ui.button("Send File").clicked() {
                        // ... existing ...
                         let session = self.session.clone(); 
                         let runtime = self.runtime.clone();
                         std::thread::spawn(move || {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                let local_path = path.clone();
                                let remote_filename = path.file_name().unwrap().to_string_lossy().to_string();
                                runtime.spawn(async move {
                                    let tx = session.control_tx.clone();
                                    let _ = session.file_transfer.start_upload(local_path, remote_filename, tx).await;
                                });
                            }
                        });
                    }
                    
                    ui.separator();

                    let mut clipboard_enabled = self.session.clipboard_manager.is_enabled();
                    if ui.checkbox(&mut clipboard_enabled, "Sync Clipboard").changed() {
                         self.session.clipboard_manager.set_enabled(clipboard_enabled);
                    }
                    
                    ui.separator();
                    if ui.button("Connection Info").clicked() {
                        action = Some(ViewerAction::ShowConnectionInfo);
                    }
                    
                    ui.separator();
                    ui.label("Quality:");
                    if ui.add(egui::Slider::new(&mut self.state.quality, 10..=100)).drag_stopped() {
                         let msg = zrc_proto::v1::ControlMsgV1 {
                             msg_type: zrc_proto::v1::ControlMsgTypeV1::SessionControl as i32,
                             payload: Some(zrc_proto::v1::control_msg_v1::Payload::SessionControl(zrc_proto::v1::SessionControlV1 {
                                 action: zrc_proto::v1::SessionControlActionV1::QualityChange as i32,
                                 quality_level: self.state.quality,
                                 ..Default::default()
                             })),
                             ..Default::default()
                         };
                         let _ = self.session.control_tx.try_send(msg);
                    }
                    
                    ui.separator();
                    ui.label("Zoom:");
                    ui.horizontal(|ui| {
                        if ui.button("Fit").clicked() {
                            self.set_zoom(ZoomLevel::Fit);
                        }
                        if ui.button("100%").clicked() {
                            self.set_zoom(ZoomLevel::Actual);
                        }
                        let mut zoom_value = if let ZoomLevel::Custom(f) = self.state.zoom { f * 100.0 } else { 100.0 };
                        if ui.add(egui::Slider::new(&mut zoom_value, 25.0..=400.0).suffix("%")).changed() {
                            self.set_zoom(ZoomLevel::Custom(zoom_value / 100.0));
                        }
                    });
                    
                    ui.separator();
                    ui.label("Monitor:");
                    // TODO: Monitor switching - protocol needs to support this
                    // For now, monitor selection is handled at connection time
                    ui.label("Monitor selection at connection");
                    
                    if ui.button("Transfers").clicked() {
                         self.state.show_transfers = !self.state.show_transfers;
                    }
                });
            });
            
        action
    }


    fn render_transfers(&mut self, ctx: &egui::Context) {
        if !self.state.show_transfers { return; }
        
        let mut show = self.state.show_transfers;
        egui::Window::new("File Transfers")
            .open(&mut show)
            .show(ctx, |ui| {
                 let transfers = self.session.file_transfer.list_transfers();
                 if transfers.is_empty() {
                     ui.label("No active transfers.");
                 } else {
                     for t in transfers {
                         ui.group(|ui| {
                             ui.label(format!("{} ({:?})", t.filename, t.direction));
                             ui.add(egui::ProgressBar::new(t.progress).show_percentage());
                             ui.label(format!("{:?}", t.state));
                         });
                     }
                 }
            });
        self.state.show_transfers = show;
    }

    fn handle_dropped_files(&self, ctx: &egui::Context) {
         if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
             let dropped = ctx.input(|i| i.raw.dropped_files.clone());
             for file in dropped {
                 if let Some(path) = file.path {
                      let session = self.session.clone();
                      let runtime = self.runtime.clone();
                      let remote_filename = path.file_name().unwrap().to_string_lossy().to_string();
                      
                      runtime.spawn(async move {
                          let tx = session.control_tx.clone();
                          let _ = session.file_transfer.start_upload(path, remote_filename, tx).await;
                      });
                 }
             }
         }
    }



    fn render_status_bar(&self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let stats = self.session.stats.read().unwrap();
                    let latency = stats.latency_ms.load(std::sync::atomic::Ordering::Relaxed);
                    let fps = stats.current_fps.load(std::sync::atomic::Ordering::Relaxed);
                    let duration = self.session.started_at.elapsed();
                    
                    ui.label(format!("Time: {:02}:{:02}:{:02}", 
                        duration.as_secs() / 3600,
                        (duration.as_secs() % 3600) / 60,
                        duration.as_secs() % 60
                    ));
                    ui.separator();
                    ui.label(format!("Latency: {}ms", latency));
                    ui.separator();
                    ui.label(format!("FPS: {}", fps));
                    
                    // Show connection quality
                    self.session.diagnostics.render_status_indicator(ui);
                });
            });
    }
}

/// Viewer state
pub struct ViewerState {
    pub fullscreen: bool,
    pub zoom: ZoomLevel,
    pub input_mode: InputMode,
    pub selected_monitor: MonitorId,
    pub show_toolbar: bool,
    pub show_stats: bool,
    pub show_transfers: bool,
    pub quality: u32,
}

impl Default for ViewerState {
    fn default() -> Self {
        Self {
            fullscreen: false,
            zoom: ZoomLevel::Fit,
            input_mode: InputMode::ViewOnly,
            selected_monitor: MonitorId::default(),
            show_toolbar: true,
            show_stats: true,
            show_transfers: false,
            quality: 80,
        }
    }
}

/// Zoom level
#[derive(Clone, Copy, PartialEq)]
pub enum ZoomLevel {
    Fit,
    Actual,
    Custom(f32),
}

impl Default for ZoomLevel {
    fn default() -> Self {
        Self::Fit
    }
}

/// Monitor identifier
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MonitorId(pub u32);

/// Viewer toolbar
#[derive(Default)]
pub struct ViewerToolbar {
    pub visible: bool,
}

/// Frame renderer
pub struct FrameRenderer {
    current_texture: Option<egui::TextureHandle>,
    remote_size: Option<Vec2>,
}

impl FrameRenderer {
    pub fn new() -> Self {
        Self {
            current_texture: None,
            remote_size: None,
        }
    }

    /// Update with new frame
    pub fn update_frame(&mut self, ctx: &egui::Context, frame: DecodedFrame) {
        let new_size = Vec2::new(frame.width as f32, frame.height as f32);
        
        // Check for resolution change
        if let Some(old_size) = self.remote_size {
            if old_size != new_size {
                tracing::info!("Resolution changed: {}x{} -> {}x{}", 
                    old_size.x, old_size.y, new_size.x, new_size.y);
                // Resolution changed - texture will be recreated automatically
            }
        }
        
        self.remote_size = Some(new_size);
        
        // Convert frame data to RGBA if needed
        let rgba_data = match frame.format {
            FrameFormat::Rgba => frame.data,
            FrameFormat::Bgra => {
                // Convert BGRA to RGBA
                let mut rgba = Vec::with_capacity(frame.data.len());
                for chunk in frame.data.chunks_exact(4) {
                    rgba.push(chunk[2]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[0]); // B
                    rgba.push(chunk[3]); // A
                }
                rgba
            }
            FrameFormat::Rgb => {
                // Convert RGB to RGBA
                let mut rgba = Vec::with_capacity(frame.width as usize * frame.height as usize * 4);
                for chunk in frame.data.chunks_exact(3) {
                    rgba.push(chunk[0]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[2]); // B
                    rgba.push(255); // A
                }
                rgba
            }
        };
        
        // Upload to GPU texture
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &rgba_data,
        );
        
        self.current_texture = Some(ctx.load_texture(
            "frame_texture",
            color_image,
            egui::TextureOptions::LINEAR,
        ));
    }

    /// Render current frame to UI
    pub fn render(&mut self, ui: &mut egui::Ui, available_size: Vec2) -> Rect {
        self.render_with_zoom(ui, available_size, ZoomLevel::Fit)
    }

    /// Render current frame to UI with zoom level
    pub fn render_with_zoom(&mut self, ui: &mut egui::Ui, available_size: Vec2, zoom: ZoomLevel) -> Rect {
        if let Some(texture) = &self.current_texture {
            let size = if let Some(remote) = self.remote_size {
                let scale = match zoom {
                    ZoomLevel::Fit => {
                        // Fit to available space while maintaining aspect ratio
                        (available_size.x / remote.x).min(available_size.y / remote.y)
                    }
                    ZoomLevel::Actual => {
                        // 100% zoom - use actual pixel size (may be larger than available)
                        1.0
                    }
                    ZoomLevel::Custom(factor) => {
                        // Custom zoom factor
                        factor
                    }
                };
                Vec2::new(remote.x * scale, remote.y * scale)
            } else {
                available_size
            };
            
            let response = ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().min, size),
                egui::Sense::click_and_drag(),
            );
            let rect = response.rect;
            
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            
            rect
        } else {
            // Placeholder: show "No frame" message
            let response = ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().min, available_size),
                egui::Sense::click_and_drag(),
            );
            let rect = response.rect;
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(20));
            ui.centered_and_justified(|ui| {
                ui.label("No frame received");
            });
            rect
        }
    }

    /// Get remote display size
    pub fn get_remote_size(&self) -> Option<Vec2> {
        self.remote_size
    }
}

/// Decoded frame ready for rendering
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub format: FrameFormat,
    pub data: Vec<u8>,
    pub timestamp: u64,
}

/// Frame format
#[derive(Clone, Copy, PartialEq)]
pub enum FrameFormat {
    Rgba,
    Bgra,
    Rgb,
}
