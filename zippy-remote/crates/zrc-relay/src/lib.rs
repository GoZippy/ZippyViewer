//! zrc-relay: QUIC relay server for ZRC
//!
//! This relay provides last-resort connectivity when NAT traversal fails,
//! forwarding encrypted QUIC datagrams without access to plaintext.

pub mod allocation;
pub mod admin;
pub mod bandwidth;
pub mod config;
pub mod forwarder;
pub mod ha;
pub mod metrics;
pub mod security;
pub mod server;
pub mod token;

pub use config::ServerConfig;
pub use server::RelayServer;
pub use token::{RelayTokenV1, TokenVerifier, TokenError};
