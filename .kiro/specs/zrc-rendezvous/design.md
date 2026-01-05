# Design Document: zrc-rendezvous

## Overview

The zrc-rendezvous crate implements a self-hostable HTTP mailbox server for the ZRC system. This server provides untrusted byte-forwarding between endpoints, enabling session initiation when direct peer-to-peer connectivity is not immediately available. The server never has access to plaintext message content.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      zrc-rendezvous                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      HTTP Server                             │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │ POST /mailbox│  │ GET /mailbox │  │ GET /health  │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│                              ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Mailbox Manager                           │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │   Mailbox    │  │   Mailbox    │  │   Mailbox    │      │   │
│  │  │  (rid_hex)   │  │  (rid_hex)   │  │  (rid_hex)   │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│  ┌───────────────────────────┼───────────────────────────────┐     │
│  │                           ▼                                │     │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │     │
│  │  │ Rate Limiter │  │   Evictor    │  │   Metrics    │    │     │
│  │  └──────────────┘  └──────────────┘  └──────────────┘    │     │
│  └───────────────────────────────────────────────────────────┘     │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### HTTP API

```
POST /v1/mailbox/{recipient_id_hex}
  Request:
    Content-Type: application/octet-stream
    Authorization: Bearer <token> (optional)
    Body: <envelope bytes>
  
  Response:
    202 Accepted - Message enqueued
    413 Payload Too Large - Message exceeds size limit
    429 Too Many Requests - Rate limited (Retry-After header)
    507 Insufficient Storage - Mailbox queue full
    401 Unauthorized - Auth required but missing
    403 Forbidden - Invalid auth token

GET /v1/mailbox/{recipient_id_hex}?wait_ms=30000
  Request:
    Authorization: Bearer <token> (optional)
  
  Response:
    200 OK - Message returned
      Headers:
        X-Message-Sequence: <sequence_number>
        X-Queue-Length: <remaining_count>
      Body: <envelope bytes>
    204 No Content - No messages (timeout)
    401 Unauthorized - Auth required but missing
    403 Forbidden - Invalid auth token
    429 Too Many Requests - Rate limited

GET /health
  Response:
    200 OK
      Body: {"status": "healthy", "uptime_seconds": 12345, "version": "1.0.0"}
    503 Service Unavailable - Overloaded

GET /metrics
  Response:
    200 OK
      Body: <prometheus format metrics>
```

### Mailbox Manager

```rust
pub struct MailboxManager {
    mailboxes: DashMap<[u8; 32], Mailbox>,
    config: MailboxConfig,
    metrics: MailboxMetrics,
}

struct Mailbox {
    messages: VecDeque<Message>,
    waiters: Vec<Waker>,
    last_activity: Instant,
    sequence: u64,
}

struct Message {
    data: Vec<u8>,
    sequence: u64,
    received_at: Instant,
}

impl MailboxManager {
    pub fn new(config: MailboxConfig) -> Self;
    
    /// Post message to mailbox
    pub fn post(&self, recipient: [u8; 32], data: Vec<u8>) -> Result<(), MailboxError>;
    
    /// Get next message (with optional wait)
    pub async fn get(&self, recipient: [u8; 32], wait: Duration) -> Result<Option<Message>, MailboxError>;
    
    /// Get mailbox statistics
    pub fn stats(&self, recipient: [u8; 32]) -> Option<MailboxStats>;
    
    /// Run eviction pass
    pub fn evict_expired(&self);
    
    /// Get global statistics
    pub fn global_stats(&self) -> GlobalStats;
}

pub struct MailboxConfig {
    pub max_message_size: usize,      // Default: 64KB
    pub max_queue_length: usize,      // Default: 100
    pub message_ttl: Duration,        // Default: 5 minutes
    pub idle_mailbox_ttl: Duration,   // Default: 1 hour
    pub max_mailboxes: usize,         // Default: 10000
}
```

### Rate Limiter

```rust
pub struct RateLimiter {
    limits: RateLimitConfig,
    buckets: DashMap<IpAddr, TokenBucket>,
    allowlist: HashSet<IpAddr>,
    blocklist: HashSet<IpAddr>,
}

struct TokenBucket {
    tokens: f64,
    last_update: Instant,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self;
    
    /// Check if request is allowed
    pub fn check(&self, ip: IpAddr, request_type: RequestType) -> Result<(), RateLimitError>;
    
    /// Add IP to allowlist
    pub fn allow(&mut self, ip: IpAddr);
    
    /// Add IP to blocklist
    pub fn block(&mut self, ip: IpAddr);
    
    /// Clean up old buckets
    pub fn cleanup(&self);
}

pub struct RateLimitConfig {
    pub posts_per_minute: u32,    // Default: 60
    pub gets_per_minute: u32,     // Default: 120
    pub window_seconds: u32,      // Default: 60
}

pub enum RequestType {
    Post,
    Get,
}
```

### Authentication

```rust
pub struct AuthManager {
    mode: AuthMode,
    server_tokens: HashSet<String>,
    mailbox_tokens: DashMap<[u8; 32], HashSet<String>>,
}

pub enum AuthMode {
    Disabled,
    ServerWide,
    PerMailbox,
}

impl AuthManager {
    pub fn new(mode: AuthMode) -> Self;
    
    /// Validate authorization header
    pub fn validate(&self, recipient: &[u8; 32], auth_header: Option<&str>) -> Result<(), AuthError>;
    
    /// Add server-wide token
    pub fn add_server_token(&mut self, token: String);
    
    /// Add mailbox-specific token
    pub fn add_mailbox_token(&mut self, recipient: [u8; 32], token: String);
    
    /// Rotate tokens
    pub fn rotate_tokens(&mut self, old_token: &str, new_token: String);
}
```

### Metrics

```rust
pub struct MailboxMetrics {
    active_mailboxes: Gauge,
    total_messages: Counter,
    messages_posted: Counter,
    messages_delivered: Counter,
    messages_evicted: Counter,
    request_latency: Histogram,
    rate_limit_hits: Counter,
    error_counts: CounterVec,
}

impl MailboxMetrics {
    pub fn new() -> Self;
    
    pub fn record_post(&self, duration: Duration);
    pub fn record_get(&self, duration: Duration, found: bool);
    pub fn record_eviction(&self, count: usize);
    pub fn record_rate_limit(&self);
    pub fn record_error(&self, error_type: &str);
    
    /// Export Prometheus format
    pub fn export(&self) -> String;
}
```

### Server Configuration

```rust
pub struct ServerConfig {
    // Network
    pub listen_addr: SocketAddr,
    pub tls_config: Option<TlsConfig>,
    
    // Mailbox
    pub mailbox: MailboxConfig,
    
    // Rate limiting
    pub rate_limit: RateLimitConfig,
    
    // Authentication
    pub auth_mode: AuthMode,
    pub auth_tokens: Vec<String>,
    
    // Operational
    pub graceful_shutdown_timeout: Duration,
    pub eviction_interval: Duration,
}

pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub min_version: TlsVersion,
}

impl ServerConfig {
    /// Load from environment variables
    pub fn from_env() -> Result<Self, ConfigError>;
    
    /// Load from TOML file
    pub fn from_file(path: &Path) -> Result<Self, ConfigError>;
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError>;
}
```

## Data Models

### Message Flow

```
Sender                    Rendezvous                  Recipient
  │                           │                           │
  │  POST /mailbox/{rid}      │                           │
  │  [envelope bytes]         │                           │
  │ ─────────────────────────▶│                           │
  │                           │  enqueue(rid, envelope)   │
  │                           │ ─────────────────────────▶│
  │  202 Accepted             │                           │
  │ ◀─────────────────────────│                           │
  │                           │                           │
  │                           │  GET /mailbox/{rid}       │
  │                           │ ◀─────────────────────────│
  │                           │  dequeue(rid)             │
  │                           │                           │
  │                           │  200 OK [envelope]        │
  │                           │ ─────────────────────────▶│
  │                           │                           │
```

### Memory Layout

```
MailboxManager
├── mailboxes: DashMap<[u8; 32], Mailbox>
│   ├── [recipient_id_1] -> Mailbox
│   │   ├── messages: VecDeque<Message>
│   │   │   ├── Message { data, sequence, received_at }
│   │   │   └── ...
│   │   ├── waiters: Vec<Waker>
│   │   ├── last_activity: Instant
│   │   └── sequence: u64
│   └── ...
├── config: MailboxConfig
└── metrics: MailboxMetrics
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system.*

### Property 1: Message Ordering
*For any* mailbox, messages SHALL be delivered in FIFO order (first posted, first retrieved).
**Validates: Requirements 1.7, 2.6**

### Property 2: Message Integrity
*For any* posted message, the retrieved message SHALL be byte-identical to the posted message.
**Validates: Requirements 1.1, 2.3**

### Property 3: TTL Enforcement
*For any* message older than the configured TTL, the message SHALL be evicted and not returned.
**Validates: Requirements 3.1**

### Property 4: Rate Limit Enforcement
*For any* source exceeding the rate limit, subsequent requests SHALL receive 429 response.
**Validates: Requirements 4.3, 4.4**

### Property 5: Queue Length Enforcement
*For any* mailbox at max queue length, new posts SHALL receive 507 response.
**Validates: Requirements 1.6**

## Error Handling

| Error | HTTP Status | Condition |
|-------|-------------|-----------|
| MessageTooLarge | 413 | Message exceeds max_message_size |
| QueueFull | 507 | Mailbox at max_queue_length |
| RateLimited | 429 | Rate limit exceeded |
| Unauthorized | 401 | Auth required but missing |
| Forbidden | 403 | Invalid auth token |
| ServiceUnavailable | 503 | Server overloaded |

## Testing Strategy

### Unit Tests
- Mailbox enqueue/dequeue operations
- Rate limiter token bucket logic
- Authentication validation
- Configuration parsing

### Property-Based Tests
- Message ordering preservation (100+ random sequences)
- Message integrity (100+ random payloads)
- TTL enforcement timing
- Rate limit behavior under load

### Integration Tests
- Full HTTP request/response cycle
- Long-poll timeout behavior
- Concurrent access patterns
- Graceful shutdown behavior

### Load Tests
- Sustained throughput measurement
- Memory usage under load
- Latency percentiles (p50, p95, p99)
