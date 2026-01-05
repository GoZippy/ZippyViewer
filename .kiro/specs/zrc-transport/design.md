# Design Document: zrc-transport

## Overview

The zrc-transport crate defines transport abstractions and common framing for the ZRC system. This crate provides traits for different transport mechanisms without OS-specific dependencies, enabling pluggable transport implementations while ensuring consistent behavior across all platforms.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        zrc-transport                                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      Traits                                  │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │ControlPlane  │  │ MediaPlane   │  │  Discovery   │      │   │
│  │  │  Transport   │  │  Transport   │  │  Transport   │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      Framing                                 │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │ LengthCodec  │  │  Channel     │  │ Multiplexer  │      │   │
│  │  │              │  │  Types       │  │              │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      Utilities                               │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │ Connection   │  │ Backpressure │  │   Metrics    │      │   │
│  │  │   State      │  │   Handler    │  │   Tracker    │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      Testing                                 │   │
│  │  ┌──────────────┐  ┌──────────────┐                         │   │
│  │  │MockTransport │  │  Loopback    │                         │   │
│  │  └──────────────┘  └──────────────┘                         │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### Transport Traits

```rust
/// Control plane transport for signaling messages
#[async_trait]
pub trait ControlPlaneTransport: Send + Sync {
    /// Send envelope to recipient
    async fn send(&self, recipient: &[u8; 32], envelope: &[u8]) -> Result<(), TransportError>;
    
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
    async fn send_frame(&self, channel: ChannelType, data: &[u8]) -> Result<SendResult, TransportError>;
    
    /// Receive frame data
    async fn recv_frame(&self) -> Result<(ChannelType, Vec<u8>), TransportError>;
    
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportType {
    Mesh,
    Direct,
    Rendezvous,
    Relay,
}

pub enum SendResult {
    Sent,
    Queued,
    Dropped,
}

pub struct CongestionState {
    pub window_available: usize,
    pub bytes_in_flight: usize,
    pub is_congested: bool,
}
```

### Framing

```rust
/// Length-prefixed frame codec
pub struct LengthCodec {
    max_frame_size: usize,
}

impl LengthCodec {
    pub fn new(max_frame_size: usize) -> Self;
    
    /// Encode data with length prefix
    /// Format: length (4 bytes BE) || data
    pub fn encode(&self, data: &[u8]) -> Result<Vec<u8>, FramingError>;
    
    /// Decode framed data
    pub fn decode(&self, framed: &[u8]) -> Result<Vec<u8>, FramingError>;
    
    /// Streaming decoder for partial reads
    pub fn decode_stream(&self, buf: &mut BytesMut) -> Result<Option<Vec<u8>>, FramingError>;
}

/// Channel types for multiplexing
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelType {
    Control = 0,
    Frames = 1,
    Clipboard = 2,
    Files = 3,
    Audio = 4,
}

impl ChannelType {
    pub fn priority(&self) -> u8 {
        match self {
            ChannelType::Control => 0,   // Highest
            ChannelType::Clipboard => 1,
            ChannelType::Files => 2,
            ChannelType::Audio => 3,
            ChannelType::Frames => 4,    // Lowest (can drop)
        }
    }
    
    pub fn is_lossy(&self) -> bool {
        matches!(self, ChannelType::Frames | ChannelType::Audio)
    }
}
```

### Connection State

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

pub struct ConnectionManager {
    state: AtomicCell<ConnectionState>,
    connected_at: Option<Instant>,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    reconnect_attempts: AtomicU32,
}

impl ConnectionManager {
    pub fn new() -> Self;
    
    /// Transition to new state
    pub fn transition(&self, new_state: ConnectionState) -> ConnectionState;
    
    /// Get current state
    pub fn state(&self) -> ConnectionState;
    
    /// Get connection duration
    pub fn duration(&self) -> Option<Duration>;
    
    /// Record bytes transferred
    pub fn record_sent(&self, bytes: u64);
    pub fn record_received(&self, bytes: u64);
    
    /// Get transfer statistics
    pub fn stats(&self) -> ConnectionStats;
}

pub struct ConnectionStats {
    pub state: ConnectionState,
    pub duration: Option<Duration>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub reconnect_attempts: u32,
}
```

### Backpressure Handler

```rust
pub struct BackpressureHandler {
    send_buffer_limit: usize,
    current_buffer: AtomicUsize,
    drop_policy: DropPolicy,
    dropped_count: AtomicU64,
}

#[derive(Clone, Copy)]
pub enum DropPolicy {
    /// Block until buffer available
    Block,
    /// Drop oldest frames
    DropOldest,
    /// Drop newest frames
    DropNewest,
    /// Drop based on priority
    DropByPriority,
}

impl BackpressureHandler {
    pub fn new(limit: usize, policy: DropPolicy) -> Self;
    
    /// Check if send is allowed
    pub fn can_send(&self, size: usize) -> bool;
    
    /// Reserve buffer space
    pub async fn reserve(&self, size: usize, channel: ChannelType) -> Result<(), BackpressureError>;
    
    /// Release buffer space
    pub fn release(&self, size: usize);
    
    /// Get dropped frame count
    pub fn dropped_count(&self) -> u64;
}
```

### Transport Priority and Fallback

```rust
pub struct TransportLadder {
    transports: Vec<(TransportType, Box<dyn ControlPlaneTransport>)>,
    timeout: Duration,
}

impl TransportLadder {
    pub fn new() -> Self;
    
    /// Add transport with priority (lower index = higher priority)
    pub fn add(&mut self, transport_type: TransportType, transport: Box<dyn ControlPlaneTransport>);
    
    /// Try transports in order until one succeeds
    pub async fn connect(&self, target: &[u8; 32]) -> Result<ConnectedTransport, TransportError>;
    
    /// Try transports in parallel, use first success
    pub async fn connect_parallel(&self, target: &[u8; 32]) -> Result<ConnectedTransport, TransportError>;
}

pub struct ConnectedTransport {
    pub transport_type: TransportType,
    pub transport: Box<dyn ControlPlaneTransport>,
    pub connect_time: Duration,
}
```

### Multiplexer

```rust
pub struct Multiplexer {
    channels: HashMap<ChannelType, ChannelState>,
    encryption: Option<Box<dyn ChannelEncryption>>,
}

struct ChannelState {
    send_seq: AtomicU64,
    recv_seq: AtomicU64,
    send_buffer: Mutex<VecDeque<Vec<u8>>>,
    recv_buffer: Mutex<VecDeque<Vec<u8>>>,
}

impl Multiplexer {
    pub fn new() -> Self;
    
    /// Open a channel
    pub fn open_channel(&mut self, channel: ChannelType) -> Result<(), MuxError>;
    
    /// Close a channel
    pub fn close_channel(&mut self, channel: ChannelType) -> Result<(), MuxError>;
    
    /// Send on channel
    pub async fn send(&self, channel: ChannelType, data: &[u8]) -> Result<(), MuxError>;
    
    /// Receive from any channel
    pub async fn recv(&self) -> Result<(ChannelType, Vec<u8>), MuxError>;
    
    /// Set encryption for channel
    pub fn set_encryption(&mut self, encryption: Box<dyn ChannelEncryption>);
}

#[async_trait]
pub trait ChannelEncryption: Send + Sync {
    async fn encrypt(&self, channel: ChannelType, seq: u64, data: &[u8]) -> Result<Vec<u8>, CryptoError>;
    async fn decrypt(&self, channel: ChannelType, seq: u64, data: &[u8]) -> Result<Vec<u8>, CryptoError>;
}
```

### Metrics

```rust
pub struct TransportMetrics {
    bytes_sent: Counter,
    bytes_received: Counter,
    messages_sent: Counter,
    messages_received: Counter,
    frames_dropped: Counter,
    rtt_histogram: Histogram,
    connection_duration: Histogram,
}

impl TransportMetrics {
    pub fn new(prefix: &str) -> Self;
    
    pub fn record_send(&self, channel: ChannelType, bytes: usize);
    pub fn record_recv(&self, channel: ChannelType, bytes: usize);
    pub fn record_drop(&self, channel: ChannelType);
    pub fn record_rtt(&self, rtt: Duration);
    
    /// Export in Prometheus format
    pub fn export_prometheus(&self) -> String;
}
```

### Testing Utilities

```rust
/// Mock transport for testing
pub struct MockTransport {
    sent: Mutex<Vec<(/* recipient */ [u8; 32], /* data */ Vec<u8>)>>,
    recv_queue: Mutex<VecDeque<(/* sender */ [u8; 32], /* data */ Vec<u8>)>>,
    connected: AtomicBool,
    latency: Duration,
    packet_loss: f64,
}

impl MockTransport {
    pub fn new() -> Self;
    
    /// Configure simulated latency
    pub fn with_latency(self, latency: Duration) -> Self;
    
    /// Configure simulated packet loss (0.0 - 1.0)
    pub fn with_packet_loss(self, loss: f64) -> Self;
    
    /// Inject message to receive queue
    pub fn inject_recv(&self, sender: [u8; 32], data: Vec<u8>);
    
    /// Get sent messages
    pub fn get_sent(&self) -> Vec<(/* recipient */ [u8; 32], /* data */ Vec<u8>)>;
    
    /// Simulate disconnect
    pub fn disconnect(&self);
}

/// Loopback transport for local testing
pub struct LoopbackTransport {
    local_id: [u8; 32],
    peer: Arc<LoopbackTransport>,
}

impl LoopbackTransport {
    /// Create connected pair
    pub fn pair() -> (Self, Self);
}
```

## Data Models

### Frame Format

```
┌─────────────────────────────────────────────────────────┐
│                    Framed Message                        │
├─────────────────────────────────────────────────────────┤
│  Length (4 bytes, big-endian)                           │
├─────────────────────────────────────────────────────────┤
│  Channel ID (1 byte)                                    │
├─────────────────────────────────────────────────────────┤
│  Sequence Number (8 bytes, big-endian)                  │
├─────────────────────────────────────────────────────────┤
│  Payload (variable)                                     │
└─────────────────────────────────────────────────────────┘
```

### Transport Priority Order

| Priority | Transport | Use Case |
|----------|-----------|----------|
| 1 | Mesh | Preferred for privacy |
| 2 | Direct | Best performance |
| 3 | Rendezvous | NAT traversal |
| 4 | Relay | Last resort |

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system.*

### Property 1: Framing Round-Trip
*For any* valid data within size limits, encoding then decoding SHALL return the original data.
**Validates: Requirements 2.6**

### Property 2: Sequence Monotonicity
*For any* channel, sequence numbers SHALL be strictly monotonically increasing.
**Validates: Requirements 5.4**

### Property 3: Backpressure Enforcement
*For any* send attempt when buffer is full, the backpressure handler SHALL either block or drop according to policy.
**Validates: Requirements 6.2, 6.3**

### Property 4: Channel Independence
*For any* error on one channel, other channels SHALL continue operating independently.
**Validates: Requirements 7.7**

### Property 5: Transport Fallback
*For any* transport failure, the transport ladder SHALL attempt the next transport in priority order.
**Validates: Requirements 3.4**

## Error Handling

| Error Type | Condition | Recovery |
|------------|-----------|----------|
| FramingError::TooLarge | Message exceeds max size | Reject message |
| FramingError::Incomplete | Partial frame received | Buffer and wait |
| TransportError::Disconnected | Connection lost | Trigger reconnection |
| TransportError::Timeout | Operation timed out | Retry or fail |
| BackpressureError::BufferFull | Send buffer exhausted | Block or drop |
| MuxError::ChannelClosed | Channel not open | Return error |

## Testing Strategy

### Unit Tests
- Frame encoding/decoding
- Channel state management
- Backpressure policy enforcement
- Metrics recording

### Property-Based Tests
- Framing round-trip (100+ random payloads)
- Sequence number monotonicity
- Backpressure behavior under load

### Integration Tests
- Mock transport message flow
- Loopback transport pair communication
- Transport ladder fallback behavior
- Multiplexer channel isolation
