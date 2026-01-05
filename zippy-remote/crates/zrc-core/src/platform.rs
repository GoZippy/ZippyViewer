// crates/zrc-core/src/platform.rs
use async_trait::async_trait;
use bytes::Bytes;

#[derive(Clone, Debug)]
pub enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseButton { button: u8, down: bool },
    Key { keycode: u32, down: bool },
    Text(String),
}

#[async_trait]
pub trait HostPlatform: Send + Sync {
    async fn capture_frame(&self) -> anyhow::Result<Bytes>; // encoded or raw (MVP: raw/RGB; later: encoded)
    async fn apply_input(&self, evt: InputEvent) -> anyhow::Result<()>;
    async fn set_clipboard(&self, data: Bytes) -> anyhow::Result<()>;
    async fn get_clipboard(&self) -> anyhow::Result<Bytes>;
}
