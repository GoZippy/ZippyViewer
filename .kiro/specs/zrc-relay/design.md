# Design Document: zrc-relay

## Overview

The zrc-relay crate implements an optional QUIC relay server for the ZRC system. This relay provides last-resort connectivity when NAT traversal fails and direct peer-to-peer connections cannot be established. The relay forwards encrypted QUIC datagrams without access to plaintext content.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         zrc-relay                                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    QUIC Server                               │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │  Listener    │  │  Connection  │  │  Connection  │      │   │
│  │  │              │  │   Handler    │  │   Handler    │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│                              ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                  Allocation Manager                          │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │ Allocation   │  │ Allocation   │  │ Allocation   │      │   │
│  │  │  (id: xxx)   │  │  (id: yyy)   │  │  (id: zzz)   │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│  ┌───────────────────────────┼───────────────────────────────┐     │
│  │                           ▼                                │     │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │     │
│  │  │Token Verifier│  │ Quota Mgr    │  │   Metrics    │    │     │
│  │  └──────────────┘  └──────────────┘  └──────────────┘    │     │
│  └───────────────────────────────────────────────────────────┘     │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Admin API (optional)                      │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │GET /allocs   │  │DELETE /alloc │  │ GET /stats   │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### Relay Token

```rust
/// Relay token for allocation authorization
pub struct RelayTokenV1 {
    pub relay_id: [u8; 16],
    pub allocation_id: [u8; 16],
    pub device_id: [u8; 32],
    pub peer_id: [u8; 32],
    pub expires_at: u64,
    pub bandwidth_limit: u32,  // bytes/sec
    pub quota_bytes: u64,
    pub signature: [u8; 64],   // Ed25519 by device
}

impl RelayTokenV1 {
    /// Verify token signature
    pub fn verify(&self, device_pub: &[u8; 32]) -> Result<(), TokenError>;
    
    /// Check if token is expired
    pub fn is_expired(&self, now: u64) -> bool;
    
    /// Compute signature input
    fn signature_input(&self) -> Vec<u8>;
}
```

### Allocation Manager

```rust
pub struct AllocationManager {
    allocations: DashMap<[u8; 16], Allocation>,
    config: AllocationConfig,
    metrics: AllocationMetrics,
}

pub struct Allocation {
    pub id: [u8; 16],
    pub device_id: [u8; 32],
    pub peer_id: [u8; 32],
    pub created_at: Instant,
    pub expires_at: Instant,
    pub bandwidth_limit: u32,
    pub quota_bytes: u64,
    pub bytes_transferred: AtomicU64,
    pub last_activity: AtomicInstant,
    pub device_conn: Option<ConnectionHandle>,
    pub peer_conn: Option<ConnectionHandle>,
}

impl AllocationManager {
    pub fn new(config: AllocationConfig) -> Self;
    
    /// Create new allocation from token
    pub fn create(&self, token: &RelayTokenV1) -> Result<AllocationInfo, AllocationError>;
    
    /// Get allocation by ID
    pub fn get(&self, id: &[u8; 16]) -> Option<AllocationRef>;
    
    /// Associate connection with allocation
    pub fn associate(&self, id: &[u8; 16], conn: ConnectionHandle, is_device: bool) 
        -> Result<(), AllocationError>;
    
    /// Record bytes transferred
    pub fn record_transfer(&self, id: &[u8; 16], bytes: u64) -> Result<(), AllocationError>;
    
    /// Terminate allocation
    pub fn terminate(&self, id: &[u8; 16], reason: TerminateReason);
    
    /// Run expiration check
    pub fn expire_stale(&self);
    
    /// Get all allocations (for admin)
    pub fn list(&self) -> Vec<AllocationInfo>;
}

pub struct AllocationConfig {
    pub max_allocations: usize,           // Default: 1000
    pub default_bandwidth: u32,           // Default: 10 Mbps
    pub default_quota: u64,               // Default: 1 GB
    pub allocation_timeout: Duration,     // Default: 8 hours
    pub idle_timeout: Duration,           // Default: 30 seconds
    pub keepalive_interval: Duration,     // Default: 15 seconds
}

pub struct AllocationInfo {
    pub id: [u8; 16],
    pub device_id: [u8; 32],
    pub peer_id: [u8; 32],
    pub relay_addr: SocketAddr,
    pub expires_at: u64,
    pub bytes_transferred: u64,
    pub created_at: u64,
}
```

### Forwarder

```rust
pub struct Forwarder {
    allocation_mgr: Arc<AllocationManager>,
    bandwidth_limiter: BandwidthLimiter,
}

impl Forwarder {
    pub fn new(allocation_mgr: Arc<AllocationManager>) -> Self;
    
    /// Forward datagram between endpoints
    pub async fn forward_datagram(
        &self,
        allocation_id: &[u8; 16],
        from_device: bool,
        data: &[u8],
    ) -> Result<(), ForwardError>;
    
    /// Forward stream data
    pub async fn forward_stream(
        &self,
        allocation_id: &[u8; 16],
        from_device: bool,
        stream: &mut QuicStream,
    ) -> Result<(), ForwardError>;
}

pub struct BandwidthLimiter {
    buckets: DashMap<[u8; 16], TokenBucket>,
}

impl BandwidthLimiter {
    /// Check if transfer is allowed
    pub fn check(&self, allocation_id: &[u8; 16], bytes: usize) -> bool;
    
    /// Consume bandwidth tokens
    pub fn consume(&self, allocation_id: &[u8; 16], bytes: usize);
}
```

### QUIC Server

```rust
pub struct RelayServer {
    endpoint: Endpoint,
    allocation_mgr: Arc<AllocationManager>,
    forwarder: Arc<Forwarder>,
    token_verifier: Arc<TokenVerifier>,
    config: ServerConfig,
}

impl RelayServer {
    pub async fn new(config: ServerConfig) -> Result<Self, ServerError>;
    
    /// Run the relay server
    pub async fn run(&self) -> Result<(), ServerError>;
    
    /// Handle incoming connection
    async fn handle_connection(&self, conn: Connection) -> Result<(), ConnectionError>;
    
    /// Graceful shutdown
    pub async fn shutdown(&self, timeout: Duration);
}

pub struct ServerConfig {
    pub listen_addr: SocketAddr,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub allocation: AllocationConfig,
    pub admin_addr: Option<SocketAddr>,
    pub admin_token: Option<String>,
}
```

### Token Verifier

```rust
pub struct TokenVerifier {
    /// Pinned device public keys (optional, for known devices)
    pinned_keys: DashMap<[u8; 32], [u8; 32]>,
    /// Token cache to avoid repeated verification
    verified_cache: Cache<[u8; 16], VerifiedToken>,
}

struct VerifiedToken {
    device_id: [u8; 32],
    expires_at: u64,
}

impl TokenVerifier {
    pub fn new() -> Self;
    
    /// Verify relay token
    pub fn verify(&self, token: &RelayTokenV1, device_pub: &[u8; 32]) 
        -> Result<(), TokenError>;
    
    /// Pin device public key
    pub fn pin_device(&self, device_id: [u8; 32], pub_key: [u8; 32]);
    
    /// Get pinned key for device
    pub fn get_pinned(&self, device_id: &[u8; 32]) -> Option<[u8; 32]>;
}
```

### Admin API

```rust
// GET /admin/allocations
pub struct ListAllocationsResponse {
    pub allocations: Vec<AllocationInfo>,
    pub total: usize,
}

// DELETE /admin/allocations/{id}
pub struct TerminateRequest {
    pub reason: String,
}

// GET /admin/stats
pub struct RelayStats {
    pub active_allocations: usize,
    pub total_allocations: u64,
    pub bytes_forwarded: u64,
    pub packets_forwarded: u64,
    pub uptime_seconds: u64,
    pub bandwidth_usage: BandwidthStats,
}

pub struct BandwidthStats {
    pub current_bps: u64,
    pub peak_bps: u64,
    pub average_bps: u64,
}
```

### Metrics

```rust
pub struct AllocationMetrics {
    active_allocations: Gauge,
    total_allocations: Counter,
    bytes_forwarded: Counter,
    packets_forwarded: Counter,
    allocation_duration: Histogram,
    bandwidth_usage: Gauge,
    quota_exceeded: Counter,
    rate_limit_drops: Counter,
}

impl AllocationMetrics {
    pub fn record_allocation_created(&self);
    pub fn record_allocation_terminated(&self, duration: Duration);
    pub fn record_forward(&self, bytes: usize);
    pub fn record_quota_exceeded(&self);
    pub fn record_rate_limit_drop(&self);
    
    /// Export Prometheus format
    pub fn export(&self) -> String;
}
```

## Data Models

### Allocation Lifecycle

```
┌─────────────┐
│   Created   │
└──────┬──────┘
       │ associate(device)
       ▼
┌─────────────┐
│   Waiting   │ (waiting for peer)
└──────┬──────┘
       │ associate(peer)
       ▼
┌─────────────┐
│   Active    │◀──────────────┐
└──────┬──────┘               │
       │                      │ (activity)
       │ idle_timeout         │
       │ or quota_exceeded    │
       │ or expires_at        │
       ▼                      │
┌─────────────┐               │
│ Terminating │───────────────┘
└──────┬──────┘
       │ cleanup
       ▼
┌─────────────┐
│ Terminated  │
└─────────────┘
```

### Forwarding Flow

```
Device                      Relay                        Peer
  │                           │                           │
  │  QUIC Connect + Token     │                           │
  │ ─────────────────────────▶│                           │
  │                           │  verify_token()           │
  │                           │  create_allocation()      │
  │  Allocation Info          │                           │
  │ ◀─────────────────────────│                           │
  │                           │                           │
  │                           │  QUIC Connect + Token     │
  │                           │ ◀─────────────────────────│
  │                           │  associate(peer)          │
  │                           │                           │
  │  Datagram [encrypted]     │                           │
  │ ─────────────────────────▶│                           │
  │                           │  forward_datagram()       │
  │                           │ ─────────────────────────▶│
  │                           │                           │
  │                           │  Datagram [encrypted]     │
  │                           │ ◀─────────────────────────│
  │  Datagram [encrypted]     │  forward_datagram()       │
  │ ◀─────────────────────────│                           │
  │                           │                           │
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system.*

### Property 1: Token Signature Verification
*For any* relay token, the relay SHALL verify the signature before creating an allocation.
**Validates: Requirements 1.2, 1.3**

### Property 2: Bandwidth Enforcement
*For any* allocation, bytes forwarded per second SHALL not exceed the bandwidth_limit.
**Validates: Requirements 4.1, 4.5**

### Property 3: Quota Enforcement
*For any* allocation, total bytes forwarded SHALL not exceed quota_bytes.
**Validates: Requirements 4.2, 4.7**

### Property 4: Expiration Enforcement
*For any* allocation past expires_at, the allocation SHALL be terminated.
**Validates: Requirements 2.5**

### Property 5: Data Integrity
*For any* forwarded datagram, the relay SHALL not modify the content.
**Validates: Requirements 3.2**

### Property 6: Allocation Isolation
*For any* allocation error, other allocations SHALL continue operating independently.
**Validates: Requirements 5.6**

## Error Handling

| Error | Condition | Action |
|-------|-----------|--------|
| TokenError::InvalidSignature | Signature verification failed | Reject connection |
| TokenError::Expired | Token past expires_at | Reject connection |
| AllocationError::QuotaExceeded | bytes_transferred > quota | Terminate allocation |
| AllocationError::MaxAllocations | At capacity | Reject new allocation |
| ForwardError::PeerDisconnected | Peer connection lost | Terminate allocation |
| ForwardError::RateLimited | Bandwidth exceeded | Drop packet |

## Testing Strategy

### Unit Tests
- Token signature verification
- Allocation lifecycle management
- Bandwidth limiter token bucket
- Quota tracking

### Property-Based Tests
- Token verification (100+ random tokens)
- Bandwidth enforcement under load
- Quota enforcement accuracy

### Integration Tests
- Full allocation flow (create → forward → terminate)
- Concurrent allocations
- Connection migration
- Graceful shutdown

### Load Tests
- Maximum concurrent allocations
- Sustained forwarding throughput
- Memory usage under load
