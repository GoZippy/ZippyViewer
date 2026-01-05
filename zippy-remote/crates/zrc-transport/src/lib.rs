//! Transport abstractions and common framing for the ZRC system.
//!
//! This crate provides traits for different transport mechanisms without OS-specific
//! dependencies, enabling pluggable transport implementations while ensuring consistent
//! behavior across all platforms.

pub mod traits;
pub mod framing;
pub mod connection;
pub mod backpressure;
pub mod mux;
pub mod metrics;
pub mod testing;
pub mod quic;
pub mod http;

pub use traits::*;
pub use framing::*;
pub use connection::*;
pub use backpressure::*;
pub use mux::*;
pub use metrics::*;
pub use testing::*;
pub use quic::*;
pub use http::*;
