//! ZRC Platform Android - Rust library for Android controller application
//!
//! This crate provides JNI bindings to expose ZRC core functionality
//! to Kotlin/Java applications on Android.

mod core;
mod error;
mod frame;
mod input;
mod session;

// Re-export main types
pub use core::ZrcCore;
pub use error::ZrcError;
pub use frame::FrameData;
pub use input::InputEvent;

// JNI bindings - only compile on Android targets
#[cfg(target_os = "android")]
mod jni_bindings;
