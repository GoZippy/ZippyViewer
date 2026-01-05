#![cfg(target_os = "linux")]

// Re-export platform traits from zrc-core
pub use zrc_core::platform::{HostPlatform, InputEvent};

// Capture backends
pub mod capture_x11_shm;
pub mod capture_x11_basic;
#[cfg(feature = "pipewire")]
pub mod capture_pipewire;
pub mod capturer;
pub mod monitor;

// Input injection
pub mod injector;
pub mod input_xtest;
#[cfg(feature = "uinput")]
pub mod input_uinput;
pub mod wayland_input;

// System integration
pub mod secret_store;
pub mod systemd;
pub mod clipboard;
pub mod desktop_env;
pub mod system_info;

// Main platform implementation
pub mod platform;

// Re-export main platform implementation
pub use platform::LinuxPlatform;
