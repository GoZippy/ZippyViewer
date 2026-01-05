pub mod app;
pub mod clipboard;
pub mod transport;
pub mod device;
pub mod diagnostics;
pub mod input;
pub mod monitor;
pub mod platform;
pub mod session;
pub mod settings;
pub mod transfer;
pub mod ui;
pub mod viewer;

#[cfg(test)]
mod proptests;

pub use app::ZrcDesktopApp;
