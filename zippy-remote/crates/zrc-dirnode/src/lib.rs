//! zrc-dirnode: Home-hostable directory node for ZRC
//!
//! This directory node provides device discovery and presence information
//! while maintaining privacy-first defaults with invite-only access.

pub mod access;
pub mod api;
pub mod config;
pub mod discovery;
pub mod records;
pub mod search_protection;
pub mod server;
pub mod store;
#[cfg(feature = "web-ui")]
pub mod web_ui;

pub use config::ServerConfig;
pub use server::DirNodeServer;
