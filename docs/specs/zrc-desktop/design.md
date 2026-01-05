# zrc-desktop Design

UI stack options:
- Rust-native: winit + pixels/wgpu + egui
- Or Tauri (Rust core + web UI) if desired later

Architecture:
- UI thread renders frames
- Network tasks (tokio) handle QUIC + control
- Input events marshalled to ControlMsgV1

Performance:
- Render at display refresh
- Drop frames when behind
