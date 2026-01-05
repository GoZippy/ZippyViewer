# Design Document: zrc-core

## Overview

The zrc-core crate implements the business logic and state machines for the ZRC system. This crate handles pairing workflows, session management, policy enforcement, message dispatch, and transport negotiation. It serves as the shared foundation for both agent and controller applications.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           zrc-core                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    State Machines                            │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │ PairingHost  │  │PairingCtrl   │  │ SessionHost  │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  │  ┌──────────────┐                                           │   │
│  │  │ SessionCtrl  │                                           │   │
│  │  └──────────────┘                                           │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Services                                  │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │   Policy     │  │  Dispatch    │  │  Transport   │      │   │
│  │  │   Engine     │  │   Router     │  │  Negotiator  │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Infrastructure                            │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │    Store     │  │    Audit     │  │ RateLimiter  │      │   │
│  │  │   (trait)    │  │   Events     │  │              │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│  Dependencies: zrc-proto, zrc-crypto, tokio, thiserror              │
└─────────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### Pairing Host State Machine

```rust
pub enum PairingHostState {
    Idle,
    InviteGenerated { invite: InviteV1, secret: [u8; 32], expires_at: u64 },
    AwaitingRequest { invite: InviteV1, secret: [u8; 32] },
    AwaitingApproval { request: PairRequestV1, operator_pub: PublicKeyBundle },
    Paired { operator_id: [u8; 32], permissions: u32 },
    Failed { reason: PairingError },
}

pub struct PairingHost {
    state: PairingHostState,
    device_identity: Arc<Identity>,
    store: Arc<dyn Store>,
    consent_handler: Box<dyn ConsentHandler>,
    rate_limiter: RateLimiter,
}

impl PairingHost {
    pub fn new(identity: Arc<Identity>, store: Arc<dyn Store>) -> Self;
    
    /// Generate new invite
    pub async fn generate_invite(&mut self, ttl_seconds: u32) -> Result<InviteV1, PairingError>;
    
    /// Process incoming pair request
    pub async fn handle_request(&mut self, request: PairRequestV1) -> Result<PairingAction, PairingError>;
    
    /// Complete pairing after consent
    pub async fn approve(&mut self, permissions: u32) -> Result<PairReceiptV1, PairingError>;
    
    /// Reject pairing
    pub async fn reject(&mut self) -> Result<(), PairingError>;
}

pub enum PairingAction {
    AwaitingConsent { sas: String, operator_id: [u8; 32] },
    AutoApproved { receipt: PairReceiptV1 },
    Rejected { reason: String },
}
```

### Pairing Controller State Machine

```rust
pub enum PairingControllerState {
    Idle,
    InviteImported { invite: InviteV1 },
    RequestSent { request: PairRequestV1 },
    AwaitingSAS { sas: String },
    Paired { device_id: [u8; 32], permissions: u32 },
    Failed { reason: PairingError },
}

pub struct PairingController {
    state: PairingControllerState,
    operator_identity: Arc<Identity>,
    store: Arc<dyn Store>,
}

impl PairingController {
    pub fn new(identity: Arc<Identity>, store: Arc<dyn Store>) -> Self;
    
    /// Import invite from base64/QR/file
    pub fn import_invite(&mut self, invite_data: &[u8]) -> Result<(), PairingError>;
    
    /// Send pair request
    pub async fn send_request(&mut self, permissions: u32) -> Result<PairRequestV1, PairingError>;
    
    /// Handle pair receipt
    pub async fn handle_receipt(&mut self, receipt: PairReceiptV1) -> Result<PairingAction, PairingError>;
    
    /// Confirm SAS verification
    pub async fn confirm_sas(&mut self) -> Result<(), PairingError>;
}
```

### Session Host State Machine

```rust
pub enum SessionHostState {
    Idle,
    RequestReceived { request: SessionInitRequestV1 },
    AwaitingConsent { request: SessionInitRequestV1, operator_id: [u8; 32] },
    Negotiating { session_id: [u8; 32] },
    Active { session: ActiveSession },
    Ended { reason: SessionEndReason },
}

pub struct SessionHost {
    state: SessionHostState,
    device_identity: Arc<Identity>,
    store: Arc<dyn Store>,
    policy: Arc<PolicyEngine>,
    consent_handler: Box<dyn ConsentHandler>,
}

impl SessionHost {
    /// Handle session init request
    pub async fn handle_request(&mut self, request: SessionInitRequestV1) 
        -> Result<SessionAction, SessionError>;
    
    /// Approve session after consent
    pub async fn approve(&mut self) -> Result<SessionInitResponseV1, SessionError>;
    
    /// Reject session
    pub async fn reject(&mut self, reason: &str) -> Result<ErrorV1, SessionError>;
    
    /// End active session
    pub async fn end_session(&mut self, reason: SessionEndReason) -> Result<(), SessionError>;
}

pub struct ActiveSession {
    pub session_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub permissions: u32,
    pub ticket: SessionTicketV1,
    pub started_at: u64,
    pub keys: SessionKeys,
}
```

### Policy Engine

```rust
pub struct PolicyEngine {
    consent_mode: ConsentMode,
    allowed_operators: Option<HashSet<[u8; 32]>>,
    time_restrictions: Option<TimeRestrictions>,
    permission_limits: u32,
}

pub enum ConsentMode {
    AlwaysRequire,
    UnattendedAllowed,
    TrustedOperatorsOnly,
}

impl PolicyEngine {
    /// Check if session requires consent
    pub fn requires_consent(&self, operator_id: &[u8; 32], permissions: u32) -> bool;
    
    /// Validate requested permissions
    pub fn validate_permissions(&self, operator_id: &[u8; 32], requested: u32, paired: u32) 
        -> Result<u32, PolicyError>;
    
    /// Check time-based restrictions
    pub fn check_time_restrictions(&self) -> Result<(), PolicyError>;
}

pub struct TimeRestrictions {
    pub allowed_hours: (u8, u8),  // Start, end hour (24h)
    pub allowed_days: Vec<Weekday>,
}
```

### Message Dispatcher

```rust
pub struct Dispatcher {
    handlers: HashMap<MsgType, Box<dyn MessageHandler>>,
    crypto: Arc<CryptoContext>,
}

impl Dispatcher {
    pub fn register_handler(&mut self, msg_type: MsgType, handler: Box<dyn MessageHandler>);
    
    /// Dispatch incoming envelope
    pub async fn dispatch(&self, envelope: EnvelopeV1) -> Result<Option<EnvelopeV1>, DispatchError>;
}

#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle(&self, sender_id: [u8; 32], payload: &[u8]) 
        -> Result<Option<Vec<u8>>, HandlerError>;
}

// Dispatch flow:
// 1. Verify envelope signature
// 2. Decrypt payload
// 3. Route to handler by msg_type
// 4. Return response (if any)
```

### Transport Negotiator

```rust
pub struct TransportNegotiator {
    preferences: TransportPreferences,
    quic_config: QuicConfig,
    relay_tokens: Vec<RelayTokenV1>,
}

impl TransportNegotiator {
    /// Generate transport parameters for session response
    pub fn generate_params(&self) -> TransportNegotiationV1;
    
    /// Select best transport from offered options
    pub fn select_transport(&self, offered: &TransportNegotiationV1) 
        -> Result<SelectedTransport, TransportError>;
}

pub struct TransportPreferences {
    pub priority: Vec<TransportType>,
    pub allow_relay: bool,
    pub prefer_mesh: bool,
}

pub enum TransportType {
    Mesh,
    Direct,
    Rendezvous,
    Relay,
}

pub enum SelectedTransport {
    Quic { params: QuicParamsV1 },
    Relay { token: RelayTokenV1, params: QuicParamsV1 },
}
```

### Store Trait

```rust
#[async_trait]
pub trait Store: Send + Sync {
    // Invites
    async fn save_invite(&self, invite: &InviteV1, secret: &[u8; 32]) -> Result<(), StoreError>;
    async fn get_invite(&self, device_id: &[u8; 32]) -> Result<Option<(InviteV1, [u8; 32])>, StoreError>;
    async fn delete_invite(&self, device_id: &[u8; 32]) -> Result<(), StoreError>;
    
    // Pairings
    async fn save_pairing(&self, pairing: &Pairing) -> Result<(), StoreError>;
    async fn get_pairing(&self, device_id: &[u8; 32], operator_id: &[u8; 32]) 
        -> Result<Option<Pairing>, StoreError>;
    async fn list_pairings(&self) -> Result<Vec<Pairing>, StoreError>;
    async fn delete_pairing(&self, device_id: &[u8; 32], operator_id: &[u8; 32]) 
        -> Result<(), StoreError>;
    
    // Tickets
    async fn save_ticket(&self, ticket: &SessionTicketV1) -> Result<(), StoreError>;
    async fn get_ticket(&self, ticket_id: &[u8; 16]) -> Result<Option<SessionTicketV1>, StoreError>;
    async fn revoke_ticket(&self, ticket_id: &[u8; 16]) -> Result<(), StoreError>;
}

pub struct Pairing {
    pub device_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub device_sign_pub: [u8; 32],
    pub operator_sign_pub: [u8; 32],
    pub permissions: u32,
    pub paired_at: u64,
    pub last_session: Option<u64>,
}
```

### Audit Events

```rust
pub enum AuditEvent {
    PairRequestReceived { operator_id: [u8; 32], timestamp: u64 },
    PairApproved { operator_id: [u8; 32], permissions: u32, timestamp: u64 },
    PairDenied { operator_id: [u8; 32], reason: String, timestamp: u64 },
    PairRevoked { operator_id: [u8; 32], timestamp: u64 },
    SessionRequested { operator_id: [u8; 32], session_id: [u8; 32], timestamp: u64 },
    SessionStarted { operator_id: [u8; 32], session_id: [u8; 32], permissions: u32, timestamp: u64 },
    SessionEnded { session_id: [u8; 32], reason: SessionEndReason, timestamp: u64 },
    SessionDenied { operator_id: [u8; 32], reason: String, timestamp: u64 },
    PolicyViolation { operator_id: [u8; 32], violation: String, timestamp: u64 },
}

#[async_trait]
pub trait AuditSink: Send + Sync {
    async fn emit(&self, event: AuditEvent) -> Result<(), AuditError>;
}
```

## Data Models

### State Transition Diagrams

```
PairingHost States:
┌──────┐  generate_invite  ┌─────────────────┐
│ Idle │─────────────────▶│ InviteGenerated │
└──────┘                   └────────┬────────┘
    ▲                               │ (timeout or explicit)
    │                               ▼
    │                      ┌─────────────────┐
    │                      │ AwaitingRequest │
    │                      └────────┬────────┘
    │                               │ handle_request
    │                               ▼
    │                      ┌─────────────────┐
    │                      │AwaitingApproval │
    │                      └────────┬────────┘
    │                        approve│ │reject
    │                               ▼ ▼
    │  ┌────────┐          ┌────────┐
    └──│ Paired │          │ Failed │
       └────────┘          └────────┘

SessionHost States:
┌──────┐  handle_request  ┌─────────────────┐
│ Idle │─────────────────▶│RequestReceived  │
└──────┘                   └────────┬────────┘
    ▲                               │ (policy check)
    │                               ▼
    │                      ┌─────────────────┐
    │                      │AwaitingConsent  │◀─┐
    │                      └────────┬────────┘  │ (if required)
    │                        approve│           │
    │                               ▼           │
    │                      ┌─────────────────┐  │
    │                      │  Negotiating    │──┘
    │                      └────────┬────────┘
    │                               │ (transport ready)
    │                               ▼
    │                      ┌─────────────────┐
    │                      │     Active      │
    │                      └────────┬────────┘
    │                               │ end_session
    │                               ▼
    │                      ┌─────────────────┐
    └──────────────────────│     Ended       │
                           └─────────────────┘
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system.*

### Property 1: Pairing Proof Verification
*For any* valid invite secret and pair request, generating then verifying the invite proof SHALL succeed.
**Validates: Requirements 1.3, 1.4**

### Property 2: Permission Enforcement
*For any* session, granted permissions SHALL never exceed the permissions established during pairing.
**Validates: Requirements 5.6**

### Property 3: Consent Policy Enforcement
*For any* session request when consent mode is ALWAYS_REQUIRE, the state machine SHALL transition to AwaitingConsent before Active.
**Validates: Requirements 5.2**

### Property 4: Ticket Binding Verification
*For any* session ticket, verification SHALL fail if session_binding doesn't match the expected value.
**Validates: Requirements 3.5**

### Property 5: Rate Limit Enforcement
*For any* source exceeding the configured rate limit, subsequent requests SHALL be rejected with RATE_LIMITED error.
**Validates: Requirements 10.3**

### Property 6: Store Round-Trip
*For any* pairing data, saving then loading from the store SHALL return equivalent data.
**Validates: Requirements 8.8**

### Property 7: Audit Event Completeness
*For any* pairing or session state transition, an appropriate audit event SHALL be emitted.
**Validates: Requirements 9.1, 9.2, 9.3**

## Error Handling

| Error Type | Condition | Recovery |
|------------|-----------|----------|
| PairingError::InviteExpired | Invite TTL exceeded | Generate new invite |
| PairingError::InvalidProof | HMAC verification failed | Reject request |
| SessionError::NotPaired | No pairing exists | Require pairing first |
| SessionError::PermissionDenied | Policy violation | Reject with reason |
| SessionError::TicketExpired | Ticket TTL exceeded | Request new session |
| PolicyError::TimeRestriction | Outside allowed hours | Reject with reason |
| StoreError::NotFound | Record doesn't exist | Handle gracefully |

## Testing Strategy

### Unit Tests
- State machine transitions for all paths
- Policy engine permission validation
- Dispatcher routing logic
- Store operations (with mock)

### Property-Based Tests
- Pairing proof generation/verification (100+ iterations)
- Permission enforcement invariants
- Rate limiter behavior under load
- Store round-trip consistency

### Integration Tests
- Full pairing flow (host + controller)
- Full session flow with consent
- Multi-session scenarios
- Error recovery paths
