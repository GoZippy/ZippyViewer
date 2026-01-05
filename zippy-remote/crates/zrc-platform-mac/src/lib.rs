#![cfg(target_os = "macos")]

// Re-export platform traits from zrc-core
pub use zrc_core::platform::{HostPlatform, InputEvent};

// Capture backends
pub mod capture_sck;
pub mod capture_cg;
pub mod capturer;
pub mod monitor;

// Input injection
pub mod injector;
pub mod mouse;
pub mod keyboard;

// System integration
pub mod permissions;
pub mod keychain;
pub mod launchd;
pub mod clipboard;
pub mod system_info;

// Main platform implementation
pub mod platform;

// Re-export main platform implementation
pub use platform::MacPlatform;
