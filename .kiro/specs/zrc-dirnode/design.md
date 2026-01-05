# Design Document: zrc-dirnode

## Overview

The zrc-dirnode crate implements a home-hostable directory node for the ZRC system. This decentralized directory provides device discovery and presence information while maintaining privacy-first defaults. The directory supports invite-only access by default with optional time-bounded discoverability.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         zrc-dirnode                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      HTTP Server                             │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │POST /records │  │GET /records  │  │POST /discovery│     │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│                              ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Record Manager                            │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │  DirRecord   │  │  DirRecord   │  │  DirRecord   │      │   │
│  │  │ (subject_id) │  │ (subject_id) │  │ (subject_id) │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│  ┌───────────────────────────┼───────────────────────────────┐     │
│  │                           ▼                                │     │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │     │
│  │  │ Access Ctrl  │  │  Discovery   │  │  Persistence │    │     │
│  │  │  (invites)   │  │   Tokens     │  │   (SQLite)   │    │     │
│  │  └──────────────┘  └──────────────┘  └──────────────┘    │     │
│  └───────────────────────────────────────────────────────────┘     │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Web UI (optional)                         │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │ Token Gen    │  │  QR Display  │  │ Token Revoke │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### HTTP API

```
POST /v1/records
  Request:
    Content-Type: application/octet-stream
    Body: <DirRecordV1 bytes>
  
  Response:
    201 Created - Record stored
    400 Bad Request - Invalid record format
    403 Forbidden - Signature verification failed
    413 Payload Too Large - Record exceeds size limit

GET /v1/records/{subject_id_hex}
  Request:
    Authorization: Bearer <invite_token>
  
  Response:
    200 OK
      Headers:
        X-Record-Expires: <timestamp>
        X-Signature-Verified: true
      Body: <DirRecordV1 bytes>
    401 Unauthorized - Invite token required
    403 Forbidden - Invalid invite token
    404 Not Found - Record not found or expired

POST /v1/records/batch
  Request:
    Content-Type: application/json
    Authorization: Bearer <invite_token>
    Body: {"subject_ids": ["hex1", "hex2", ...]}
  
  Response:
    200 OK
      Body: {"records": [<DirRecordV1>, ...], "not_found": ["hex3"]}

POST /v1/discovery/tokens
  Request:
    Authorization: Bearer <admin_token>
    Content-Type: application/json
    Body: {"subject_id": "hex", "ttl_seconds": 600, "scope": "pairing"}
  
  Response:
    201 Created
      Body: {"token_id": "hex", "expires_at": 1234567890}

DELETE /v1/discovery/tokens/{token_id}
  Request:
    Authorization: Bearer <admin_token>
  
  Response:
    204 No Content
```

### Record Manager

```rust
pub struct RecordManager {
    records: DashMap<[u8; 32], StoredRecord>,
    store: Arc<dyn RecordStore>,
    config: RecordConfig,
}

struct StoredRecord {
    record: DirRecordV1,
    stored_at: Instant,
    access_count: AtomicU64,
}

impl RecordManager {
    pub fn new(store: Arc<dyn RecordStore>, config: RecordConfig) -> Self;
    
    /// Store or update record
    pub async fn store(&self, record: DirRecordV1) -> Result<(), RecordError>;
    
    /// Get record by subject ID
    pub async fn get(&self, subject_id: &[u8; 32]) -> Result<Option<DirRecordV1>, RecordError>;
    
    /// Get multiple records
    pub async fn get_batch(&self, subject_ids: &[[u8; 32]]) -> Vec<Option<DirRecordV1>>;
    
    /// Delete record
    pub async fn delete(&self, subject_id: &[u8; 32]) -> Result<(), RecordError>;
    
    /// Run expiration cleanup
    pub async fn cleanup_expired(&self);
    
    /// Verify record signature
    fn verify_record(&self, record: &DirRecordV1) -> Result<(), RecordError>;
}

pub struct RecordConfig {
    pub max_record_size: usize,      // Default: 4KB
    pub max_ttl_seconds: u32,        // Default: 86400 (24h)
    pub max_records: usize,          // Default: 100000
    pub cleanup_interval: Duration,  // Default: 1 hour
}

#[async_trait]
pub trait RecordStore: Send + Sync {
    async fn save(&self, subject_id: &[u8; 32], record: &DirRecordV1) -> Result<(), StoreError>;
    async fn load(&self, subject_id: &[u8; 32]) -> Result<Option<DirRecordV1>, StoreError>;
    async fn delete(&self, subject_id: &[u8; 32]) -> Result<(), StoreError>;
    async fn list_expired(&self, now: u64) -> Result<Vec<[u8; 32]>, StoreError>;
}
```

### Access Control

```rust
pub struct AccessController {
    mode: AccessMode,
    invite_tokens: DashMap<String, InviteToken>,
    admin_tokens: HashSet<String>,
    rate_limiter: RateLimiter,
}

pub enum AccessMode {
    /// Require invite token for all lookups
    InviteOnly,
    /// Allow lookups with active discovery token
    DiscoveryEnabled,
    /// Open access (not recommended)
    Open,
}

struct InviteToken {
    subject_ids: HashSet<[u8; 32]>,
    expires_at: Option<u64>,
    created_by: [u8; 32],
}

impl AccessController {
    pub fn new(mode: AccessMode) -> Self;
    
    /// Check if lookup is authorized
    pub fn authorize_lookup(
        &self,
        subject_id: &[u8; 32],
        token: Option<&str>,
    ) -> Result<(), AccessError>;
    
    /// Create invite token
    pub fn create_invite(
        &self,
        subject_ids: Vec<[u8; 32]>,
        ttl: Option<Duration>,
        created_by: [u8; 32],
    ) -> String;
    
    /// Revoke invite token
    pub fn revoke_invite(&self, token: &str) -> Result<(), AccessError>;
    
    /// Check admin authorization
    pub fn authorize_admin(&self, token: &str) -> Result<(), AccessError>;
}
```

### Discovery Token Manager

```rust
pub struct DiscoveryManager {
    tokens: DashMap<[u8; 16], DiscoveryToken>,
    subject_index: DashMap<[u8; 32], Vec<[u8; 16]>>,
    config: DiscoveryConfig,
}

struct DiscoveryToken {
    token_id: [u8; 16],
    subject_id: [u8; 32],
    expires_at: u64,
    scope: DiscoveryScope,
    created_at: u64,
}

pub enum DiscoveryScope {
    PairingOnly,
    SessionOnly,
}

impl DiscoveryManager {
    pub fn new(config: DiscoveryConfig) -> Self;
    
    /// Create discovery token
    pub fn create(
        &self,
        subject_id: [u8; 32],
        ttl: Duration,
        scope: DiscoveryScope,
    ) -> Result<DiscoveryTokenV1, DiscoveryError>;
    
    /// Check if subject is discoverable
    pub fn is_discoverable(&self, subject_id: &[u8; 32]) -> bool;
    
    /// Revoke discovery token
    pub fn revoke(&self, token_id: &[u8; 16]) -> Result<(), DiscoveryError>;
    
    /// Get active tokens for subject
    pub fn get_tokens(&self, subject_id: &[u8; 32]) -> Vec<DiscoveryTokenV1>;
    
    /// Cleanup expired tokens
    pub fn cleanup_expired(&self);
}

pub struct DiscoveryConfig {
    pub max_ttl: Duration,              // Default: 1 hour
    pub default_ttl: Duration,          // Default: 10 minutes
    pub max_tokens_per_subject: usize,  // Default: 3
}
```

### SQLite Store

```rust
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn new(path: &Path) -> Result<Self, StoreError>;
    
    /// Initialize schema
    async fn init_schema(&self) -> Result<(), StoreError>;
    
    /// Backup database
    pub async fn backup(&self, dest: &Path) -> Result<(), StoreError>;
    
    /// Export all records as JSON
    pub async fn export_json(&self) -> Result<String, StoreError>;
    
    /// Import records from JSON
    pub async fn import_json(&self, json: &str) -> Result<usize, StoreError>;
}

// Schema
/*
CREATE TABLE records (
    subject_id BLOB PRIMARY KEY,
    record_data BLOB NOT NULL,
    signature BLOB NOT NULL,
    timestamp INTEGER NOT NULL,
    ttl_seconds INTEGER NOT NULL,
    stored_at INTEGER NOT NULL
);

CREATE INDEX idx_records_expiry ON records (timestamp + ttl_seconds);

CREATE TABLE discovery_tokens (
    token_id BLOB PRIMARY KEY,
    subject_id BLOB NOT NULL,
    expires_at INTEGER NOT NULL,
    scope TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_discovery_subject ON discovery_tokens (subject_id);
CREATE INDEX idx_discovery_expiry ON discovery_tokens (expires_at);
*/
```

### Web UI

```rust
pub struct WebUI {
    templates: Tera,
    discovery_mgr: Arc<DiscoveryManager>,
    auth: Arc<AccessController>,
}

impl WebUI {
    /// Serve UI at /ui
    pub fn routes() -> Router;
    
    // Pages:
    // GET /ui - Dashboard
    // GET /ui/tokens - Token management
    // POST /ui/tokens/create - Create discovery token
    // POST /ui/tokens/{id}/revoke - Revoke token
    // GET /ui/qr/{token_id} - QR code for token
}
```

## Data Models

### DirRecordV1 Structure

```
DirRecordV1
├── subject_id: [u8; 32]        // Device ID
├── device_sign_pub: [u8; 32]   // For signature verification
├── endpoints: EndpointHintsV1
│   ├── direct_addrs: Vec<String>
│   ├── relay_tokens: Vec<RelayTokenV1>
│   ├── rendezvous_urls: Vec<String>
│   └── mesh_hints: Vec<String>
├── ttl_seconds: u32
├── timestamp: u64              // Unix timestamp
└── signature: [u8; 64]         // Ed25519 over transcript
```

### Signature Transcript

```
Transcript for DirRecordV1 signature:
  append(TAG_DOMAIN, "zrc_dir_record_v1")
  append(TAG_SUBJECT_ID, subject_id)
  append(TAG_SIGN_PUB, device_sign_pub)
  append(TAG_ENDPOINTS, endpoints.encode())
  append(TAG_TTL, ttl_seconds.to_be_bytes())
  append(TAG_TIMESTAMP, timestamp.to_be_bytes())
  finalize() -> hash
  
Signature = Ed25519.sign(device_priv, hash)
```

### Access Flow

```
Operator                    Dirnode                     Device
  │                           │                           │
  │                           │  POST /v1/records         │
  │                           │ ◀─────────────────────────│
  │                           │  (signed DirRecordV1)     │
  │                           │                           │
  │                           │  verify_signature()       │
  │                           │  store_record()           │
  │                           │                           │
  │  GET /v1/records/{id}     │                           │
  │  Authorization: Bearer    │                           │
  │ ─────────────────────────▶│                           │
  │                           │  authorize_lookup()       │
  │                           │  get_record()             │
  │                           │                           │
  │  200 OK (DirRecordV1)     │                           │
  │ ◀─────────────────────────│                           │
  │                           │                           │
  │  verify_signature()       │                           │
  │  extract_endpoints()      │                           │
  │                           │                           │
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system.*

### Property 1: Signature Verification
*For any* stored record, the signature SHALL be verified against the subject's public key before storage.
**Validates: Requirements 1.2, 1.3**

### Property 2: Subject ID Binding
*For any* record, the subject_id SHALL match SHA256(device_sign_pub)[0..32].
**Validates: Requirements 1.4**

### Property 3: TTL Enforcement
*For any* record where timestamp + ttl_seconds < now, lookups SHALL return 404.
**Validates: Requirements 2.4**

### Property 4: Invite-Only Access
*For any* lookup without valid invite token (when in InviteOnly mode), the request SHALL be rejected with 401.
**Validates: Requirements 3.1, 3.2**

### Property 5: Discovery Token Expiry
*For any* discovery token past expires_at, the subject SHALL no longer be discoverable.
**Validates: Requirements 4.5**

### Property 6: Record Integrity
*For any* stored record, retrieval SHALL return byte-identical data to what was stored.
**Validates: Requirements 6.2**

### Property 7: Store Round-Trip
*For any* valid DirRecordV1, saving then loading from SQLite SHALL return equivalent data.
**Validates: Requirements 8.1**

## Error Handling

| Error | HTTP Status | Condition |
|-------|-------------|-----------|
| InvalidSignature | 403 | Record signature verification failed |
| SubjectMismatch | 403 | subject_id doesn't match signing key |
| RecordTooLarge | 413 | Record exceeds max_record_size |
| TTLTooLong | 400 | ttl_seconds exceeds max_ttl |
| Unauthorized | 401 | Invite token required but missing |
| Forbidden | 403 | Invalid invite token |
| NotFound | 404 | Record not found or expired |
| RateLimited | 429 | Rate limit exceeded |

## Testing Strategy

### Unit Tests
- Record signature verification
- Access control authorization
- Discovery token lifecycle
- SQLite store operations

### Property-Based Tests
- Signature verification (100+ random records)
- TTL enforcement timing
- Store round-trip consistency
- Access control invariants

### Integration Tests
- Full record publish/lookup flow
- Discovery token creation and usage
- Concurrent access patterns
- Database backup/restore

### Security Tests
- Enumeration attack resistance
- Timing attack resistance
- Invalid signature rejection
