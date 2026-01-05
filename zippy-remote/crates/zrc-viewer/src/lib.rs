#![forbid(unsafe_code)]

use anyhow::Context;
use pixels::{Pixels, SurfaceTexture};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use zrc_core::quic_mux::FramePacketV1;
use zrc_proto::v1::{ControlMsgV1, InputEventV1, MouseMoveV1, MouseButtonV1};

pub fn run_viewer(
    mut frames_rx: mpsc::UnboundedReceiver<FramePacketV1>,
    mut input_tx: mpsc::UnboundedSender<ControlMsgV1>,
) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("ZRC Viewer")
        .with_inner_size(LogicalSize::new(960.0, 540.0))
        .build(&event_loop)?;

    let latest: Arc<Mutex<Option<FramePacketV1>>> = Arc::new(Mutex::new(None));
    let latest2 = latest.clone();

    // Receive frames on a background thread (winit wants main thread)
    std::thread::spawn(move || {
        while let Some(pkt) = frames_rx.blocking_recv() {
            *latest2.lock().unwrap() = Some(pkt);
        }
    });

    // Start with a placeholder surface; will resize once we have a frame
    let mut pixels = {
        let size = window.inner_size();
        let st = SurfaceTexture::new(size.width, size.height, &window);
        Pixels::new(320, 180, st)?
    };

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::CursorMoved { position, .. } => {
                    // Send mouse move (absolute in window space for MVP)
                    let msg = ControlMsgV1 {
                        msg: Some(zrc_proto::v1::control_msg_v1::Msg::Input(InputEventV1 {
                            kind: Some(zrc_proto::v1::input_event_v1::Kind::MouseMove(MouseMoveV1 {
                                x: position.x as i32,
                                y: position.y as i32,
                            })),
                        })),
                    };
                    let _ = input_tx.send(msg);
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    let b = match button {
                        MouseButton::Left => 1,
                        MouseButton::Right => 2,
                        MouseButton::Middle => 3,
                        _ => 1,
                    };
                    let down = state == ElementState::Pressed;
                    let msg = ControlMsgV1 {
                        msg: Some(zrc_proto::v1::control_msg_v1::Msg::Input(InputEventV1 {
                            kind: Some(zrc_proto::v1::input_event_v1::Kind::MouseButton(MouseButtonV1 {
                                button: b,
                                down,
                            })),
                        })),
                    };
                    let _ = input_tx.send(msg);
                }
                WindowEvent::Resized(size) => {
                    pixels.resize_surface(size.width, size.height);
                }
                _ => {}
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                if let Some(pkt) = latest.lock().unwrap().clone() {
                    // Resize pixel buffer to match incoming frame
                    if pixels.texture_width() != pkt.width || pixels.texture_height() != pkt.height {
                        let size = window.inner_size();
                        let st = SurfaceTexture::new(size.width, size.height, &window);
                        pixels = Pixels::new(pkt.width, pkt.height, st).context("pixels resize")?;
                    }

                    // pkt.format=1 is BGRA; pixels expects RGBA. Convert in place.
                    let frame = pixels.frame_mut();
                    let mut i = 0usize;
                    while i + 4 <= pkt.pixels.len() && i + 4 <= frame.len() {
                        let b = pkt.pixels[i];
                        let g = pkt.pixels[i + 1];
                        let r = pkt.pixels[i + 2];
                        let a = pkt.pixels[i + 3];
                        frame[i] = r;
                        frame[i + 1] = g;
                        frame[i + 2] = b;
                        frame[i + 3] = a;
                        i += 4;
                    }
                }

                if pixels.render().is_err() {
                    elwt.exit();
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}

