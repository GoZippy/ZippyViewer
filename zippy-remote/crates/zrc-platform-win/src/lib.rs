#![cfg(windows)]
// Note: Individual modules use #![allow(unsafe_code)] for Windows API calls

// Re-export platform traits from zrc-core
pub use zrc_core::platform::{HostPlatform, InputEvent};

// Capture backends
pub mod capture_gdi;
pub mod capture_dxgi;
pub mod capture_wgc;
pub mod capturer;

// Input injection
pub mod input_sendinput;
pub mod injector;
pub mod special_keys;

// System integration
pub mod service;
pub mod keystore;
pub mod clipboard;
pub mod uac;
pub mod system_info;
pub mod monitor;

// Main platform implementation
pub mod platform;

// Re-export main platform implementation
pub use platform::WinPlatform;
