//! Transport traits for control plane, media plane, and discovery.

use async_trait::async_trait;
use bytes::Bytes;
use std::time::Duration;

/// Endpoint identifier (32-byte device ID wrapped in Bytes)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EndpointId(pub Bytes);

/// Envelope bytes wrapper
#[derive(Clone, Debug)]
pub struct EnvelopeBytes(pub Bytes);

/// Route hint for establishing connections
#[derive(Clone, Debug)]
pub enum RouteHint {
    /// Direct IP connection
    DirectIp { host: String, port: u16 },
    /// Rendezvous server URL
    RendezvousUrl { url: String },
    /// Mesh mailbox routing
    MeshMailbox { mailbox_id: [u8; 32] },
}

/// Parameters for opening a media session
#[derive(Clone, Debug)]
pub struct MediaOpenParams {
    /// Peer's device ID
    pub peer_id: [u8; 32],
    /// Route hint for connection
    pub route: RouteHint,
    /// Optional ALPN protocol
    pub alpn: Option<String>,
    /// Optional relay token (also used for cert in some implementations)
    pub relay_token: Option<Bytes>,
}

/// Media session trait for real-time communication
#[async_trait]
pub trait MediaSession: Send + Sync {
    /// Send control message
    async fn send_control(&self, data: Bytes) -> anyhow::Result<()>;
    /// Receive control message
    async fn recv_control(&self) -> anyhow::Result<Bytes>;
    /// Send media frame
    async fn send_media_frame(&self, data: Bytes) -> anyhow::Result<()>;
    /// Receive media frame
    async fn recv_media_frame(&self) -> anyhow::Result<Bytes>;
    /// Close the session
    async fn close(&self) -> anyhow::Result<()>;
}

/// Media transport factory trait
#[async_trait]
pub trait MediaTransport: Send + Sync {
    /// Open a new media session
    async fn open(&self, params: MediaOpenParams) -> anyhow::Result<Box<dyn MediaSession>>;
}

/// Transport type identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransportType {
    Mesh,
    Direct,
    Rendezvous,
    Relay,
}

/// Result of a send operation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendResult {
    Sent,
    Queued,
    Dropped,
}

/// Congestion state information
#[derive(Clone, Copy, Debug)]
pub struct CongestionState {
    pub window_available: usize,
    pub bytes_in_flight: usize,
    pub is_congested: bool,
}

/// Control plane transport for signaling messages
#[async_trait]
pub trait ControlPlaneTransport: Send + Sync {
    /// Send envelope to recipient
    async fn send(
        &self,
        recipient: &[u8; 32],
        envelope: &[u8],
    ) -> Result<(), TransportError>;

    /// Receive next envelope (blocking)
    async fn recv(&self) -> Result<(/* sender */ [u8; 32], /* envelope */ Vec<u8>), TransportError>;

    /// Check if transport is connected
    fn is_connected(&self) -> bool;

    /// Get transport type identifier
    fn transport_type(&self) -> TransportType;
}

/// Media plane transport for real-time data
#[async_trait]
pub trait MediaPlaneTransport: Send + Sync {
    /// Send frame data (may drop if congested)
    async fn send_frame(
        &self,
        channel: crate::mux::ChannelType,
        data: &[u8],
    ) -> Result<SendResult, TransportError>;

    /// Receive frame data
    async fn recv_frame(&self) -> Result<(crate::mux::ChannelType, Vec<u8>), TransportError>;

    /// Get current congestion state
    fn congestion_state(&self) -> CongestionState;

    /// Get round-trip time estimate
    fn rtt_estimate(&self) -> Duration;
}

/// Discovery transport for finding endpoints
#[async_trait]
pub trait DiscoveryTransport: Send + Sync {
    /// Publish presence record
    async fn publish(&self, record: &[u8]) -> Result<(), TransportError>;

    /// Lookup endpoint by ID
    async fn lookup(&self, id: &[u8; 32]) -> Result<Option<Vec<u8>>, TransportError>;

    /// Subscribe to presence updates
    async fn subscribe(&self, id: &[u8; 32]) -> Result<(), TransportError>;
}

/// Common transport error type
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Transport disconnected")]
    Disconnected,

    #[error("Operation timed out")]
    Timeout,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Framing error: {0}")]
    Framing(#[from] FramingError),

    #[error("Backpressure error: {0}")]
    Backpressure(#[from] BackpressureError),

    #[error("Multiplexer error: {0}")]
    Multiplexer(#[from] MuxError),

    #[error("Other error: {0}")]
    Other(String),
}

use crate::backpressure::BackpressureError;
use crate::framing::FramingError;
use crate::mux::MuxError;
