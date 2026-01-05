//! ZRC Core - Business logic and state machines for Zippy Remote Control.
//!
//! This crate implements:
//! - Pairing state machines (host and controller)
//! - Session state machines (host and controller)
//! - Policy engine for consent and permissions
//! - Message dispatch and routing
//! - Transport negotiation
//! - Persistent storage abstraction
//! - Audit event generation
//! - Rate limiting

#![forbid(unsafe_code)]

// Core state machines
pub mod pairing;
pub mod session;

// Services
pub mod policy;
pub mod dispatch;
pub mod transport;

// Infrastructure
pub mod store;
pub mod audit;
pub mod rate_limit;

// Supporting modules
pub mod errors;
pub mod types;
pub mod keys;
pub mod harness;

// Platform abstraction (optional)
pub mod platform;

// Optional transport implementations
#[cfg(feature = "http-mailbox")]
pub mod http_mailbox;

#[cfg(feature = "quic")]
pub mod quic;

#[cfg(feature = "quic")]
pub mod quic_mux;

// Optional storage implementations
#[cfg(feature = "sqlite")]
pub mod sqlite_store;
