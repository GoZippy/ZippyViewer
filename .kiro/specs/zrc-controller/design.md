# Design Document: zrc-controller

## Overview

The zrc-controller crate implements a command-line interface (CLI) for the Zippy Remote Control (ZRC) system. This power-user tool enables pairing with devices, initiating sessions, and debugging transport and cryptography. The controller serves as both a standalone tool and a reference implementation for the controller-side protocol flows.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            zrc-controller                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         CLI Layer (clap)                              │   │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐       │   │
│  │  │  pair   │ │ session │ │  input  │ │ pairings│ │  debug  │       │   │
│  │  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘       │   │
│  └───────┼──────────┼──────────┼──────────┼──────────┼────────────────┘   │
│          └──────────┴──────────┴──────────┴──────────┘                      │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Controller Core                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Identity   │  │   Pairing   │  │   Session   │                  │   │
│  │  │  Manager    │  │   Client    │  │   Client    │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Config    │  │   Output    │  │   Debug     │                  │   │
│  │  │   Manager   │  │  Formatter  │  │   Tools     │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Transport Layer                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │    Mesh     │  │ Rendezvous  │  │    QUIC     │                  │   │
│  │  │   Client    │  │   Client    │  │   Client    │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Storage Layer                                    │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Identity   │  │  Pairings   │  │   Config    │                  │   │
│  │  │   Store     │  │    Store    │  │    File     │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### CLI Commands Structure

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zrc-controller")]
#[command(about = "ZRC Controller CLI - Remote control client")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Output format
    #[arg(long, default_value = "table")]
    pub output: OutputFormat,
    
    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
    
    /// Debug mode
    #[arg(long)]
    pub debug: bool,
    
    /// Config file path
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Pair with a device
    Pair(PairArgs),
    /// Manage sessions
    Session(SessionArgs),
    /// Send input commands
    Input(InputArgs),
    /// Manage pairings
    Pairings(PairingsArgs),
    /// Manage operator identity
    Identity(IdentityArgs),
    /// Receive and display frames
    Frames(FramesArgs),
    /// Debug and diagnostic tools
    Debug(DebugArgs),
}

#[derive(clap::Args)]
pub struct PairArgs {
    /// Import invite from base64, file, or QR image
    #[arg(long)]
    pub invite: Option<String>,
    
    /// Device ID to pair with
    #[arg(long)]
    pub device: Option<String>,
    
    /// Requested permissions
    #[arg(long)]
    pub permissions: Option<String>,
    
    /// Dry run (validate only)
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(clap::Args)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub action: SessionAction,
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// Start a new session
    Start {
        #[arg(long)]
        device: String,
        #[arg(long)]
        capabilities: Option<String>,
    },
    /// Connect to established session via QUIC
    Connect {
        #[arg(long)]
        quic: String,
        #[arg(long)]
        cert: String,
        #[arg(long)]
        ticket: String,
        #[arg(long)]
        relay: Option<String>,
    },
    /// List active sessions
    List,
    /// End a session
    End {
        #[arg(long)]
        session: String,
    },
}
```

### Identity Manager

```rust
/// Manages operator cryptographic identity
pub struct IdentityManager {
    operator_id: OperatorId,
    signing_key: SigningKeyPair,
    kex_key: KexKeyPair,
    key_store: Box<dyn KeyStore>,
    created_at: SystemTime,
}

impl IdentityManager {
    /// Initialize or load existing identity
    pub async fn init(config: &IdentityConfig) -> Result<Self, IdentityError>;
    
    /// Get operator ID
    pub fn operator_id(&self) -> &OperatorId;
    
    /// Get public key bundle
    pub fn public_bundle(&self) -> PublicKeyBundleV1;
    
    /// Sign data
    pub fn sign(&self, data: &[u8]) -> Signature;
    
    /// Perform key exchange
    pub fn key_exchange(&self, peer_kex_pub: &[u8; 32]) -> SharedSecret;
    
    /// Export identity (public info only)
    pub fn export_public(&self) -> IdentityExport;
    
    /// Rotate identity (warning: breaks pairings)
    pub async fn rotate(&mut self) -> Result<(), IdentityError>;
    
    /// Display identity info
    pub fn display_info(&self) -> IdentityInfo;
}

pub struct IdentityInfo {
    pub operator_id: String,
    pub fingerprint: String,
    pub created_at: SystemTime,
    pub key_algorithm: String,
}
```

### Pairing Client

```rust
/// Handles pairing operations
pub struct PairingClient {
    identity: Arc<IdentityManager>,
    transport: Arc<TransportClient>,
    pairings_store: PairingsStore,
}

impl PairingClient {
    /// Import and validate an invite
    pub fn import_invite(&self, source: InviteSource) -> Result<InviteV1, PairingError>;
    
    /// Execute pairing flow
    pub async fn pair(
        &self,
        invite: &InviteV1,
        options: PairOptions,
    ) -> Result<PairingResult, PairingError>;
    
    /// Verify SAS code with user
    pub fn verify_sas(&self, sas: &str) -> Result<bool, PairingError>;
    
    /// Store successful pairing
    pub fn store_pairing(&self, pairing: PairingInfo) -> Result<(), PairingError>;
}

pub enum InviteSource {
    Base64(String),
    File(PathBuf),
    QrImage(PathBuf),
    Clipboard,
}

pub struct PairOptions {
    pub requested_permissions: Permissions,
    pub timeout: Duration,
    pub transport_preference: TransportPreference,
}

pub struct PairingResult {
    pub device_id: DeviceId,
    pub permissions_granted: Permissions,
    pub paired_at: SystemTime,
    pub sas_verified: bool,
}
```

### Session Client

```rust
/// Handles session operations
pub struct SessionClient {
    identity: Arc<IdentityManager>,
    transport: Arc<TransportClient>,
    pairings_store: Arc<PairingsStore>,
    active_sessions: RwLock<HashMap<SessionId, ActiveSession>>,
}

impl SessionClient {
    /// Initiate a new session
    pub async fn start_session(
        &self,
        device_id: &DeviceId,
        options: SessionOptions,
    ) -> Result<SessionInitResult, SessionError>;
    
    /// Connect to session via QUIC
    pub async fn connect_quic(
        &self,
        params: QuicConnectParams,
    ) -> Result<QuicSession, SessionError>;
    
    /// Send control message
    pub async fn send_control(
        &self,
        session: &SessionId,
        msg: ControlMsgV1,
    ) -> Result<(), SessionError>;
    
    /// Receive frames
    pub fn frame_receiver(&self, session: &SessionId) -> Option<FrameReceiver>;
    
    /// End session
    pub async fn end_session(&self, session: &SessionId) -> Result<(), SessionError>;
}

pub struct SessionOptions {
    pub capabilities: Capabilities,
    pub transport_preference: TransportPreference,
    pub timeout: Duration,
}

pub struct SessionInitResult {
    pub session_id: SessionId,
    pub granted_capabilities: Capabilities,
    pub quic_params: QuicParamsV1,
    pub ticket: SessionTicketV1,
}

pub struct QuicConnectParams {
    pub host: String,
    pub port: u16,
    pub cert_fingerprint: [u8; 32],
    pub ticket: SessionTicketV1,
    pub relay_url: Option<String>,
}

pub struct QuicSession {
    pub session_id: SessionId,
    pub connection: quinn::Connection,
    pub control_stream: BiStream,
    pub frame_stream: RecvStream,
}
```

### Input Commands

```rust
/// Input command builder and sender
pub struct InputCommands {
    session: Arc<SessionClient>,
}

impl InputCommands {
    /// Send mouse move
    pub async fn mouse_move(&self, session: &SessionId, x: i32, y: i32) -> Result<(), InputError>;
    
    /// Send mouse click
    pub async fn mouse_click(
        &self,
        session: &SessionId,
        x: i32,
        y: i32,
        button: MouseButton,
    ) -> Result<(), InputError>;
    
    /// Send key event
    pub async fn key(
        &self,
        session: &SessionId,
        code: KeyCode,
        down: bool,
    ) -> Result<(), InputError>;
    
    /// Send text input
    pub async fn text(&self, session: &SessionId, text: &str) -> Result<(), InputError>;
    
    /// Send scroll
    pub async fn scroll(&self, session: &SessionId, delta: i32) -> Result<(), InputError>;
}
```

### Pairings Store

```rust
/// Persistent storage for pairings
pub struct PairingsStore {
    db_path: PathBuf,
    conn: Connection,
}

impl PairingsStore {
    /// Open or create pairings database
    pub fn open(path: &Path) -> Result<Self, StoreError>;
    
    /// List all pairings
    pub fn list(&self) -> Result<Vec<StoredPairing>, StoreError>;
    
    /// Get pairing by device ID
    pub fn get(&self, device_id: &DeviceId) -> Result<Option<StoredPairing>, StoreError>;
    
    /// Store new pairing
    pub fn store(&self, pairing: StoredPairing) -> Result<(), StoreError>;
    
    /// Update pairing
    pub fn update(&self, pairing: &StoredPairing) -> Result<(), StoreError>;
    
    /// Delete pairing
    pub fn delete(&self, device_id: &DeviceId) -> Result<(), StoreError>;
    
    /// Export pairings to file
    pub fn export(&self, path: &Path) -> Result<(), StoreError>;
    
    /// Import pairings from file
    pub fn import(&self, path: &Path) -> Result<u32, StoreError>;
}

pub struct StoredPairing {
    pub device_id: DeviceId,
    pub device_name: Option<String>,
    pub device_sign_pub: [u8; 32],
    pub device_kex_pub: [u8; 32],
    pub permissions: Permissions,
    pub paired_at: SystemTime,
    pub last_session: Option<SystemTime>,
    pub session_count: u32,
}
```

### Output Formatter

```rust
/// Formats output for different modes
pub struct OutputFormatter {
    format: OutputFormat,
    verbose: bool,
}

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Table,
    Json,
    Quiet,
}

impl OutputFormatter {
    /// Format pairing list
    pub fn format_pairings(&self, pairings: &[StoredPairing]) -> String;
    
    /// Format session info
    pub fn format_session(&self, session: &SessionInitResult) -> String;
    
    /// Format identity info
    pub fn format_identity(&self, info: &IdentityInfo) -> String;
    
    /// Format error
    pub fn format_error(&self, error: &dyn std::error::Error) -> String;
    
    /// Format progress message
    pub fn progress(&self, message: &str);
    
    /// Format success message
    pub fn success(&self, message: &str);
}

impl OutputFormatter {
    fn to_json<T: Serialize>(&self, value: &T) -> String {
        serde_json::to_string_pretty(value).unwrap()
    }
    
    fn to_table(&self, headers: &[&str], rows: Vec<Vec<String>>) -> String {
        // Use comfy-table or similar
        todo!()
    }
}
```

### Debug Tools

```rust
/// Debugging and diagnostic utilities
pub struct DebugTools {
    identity: Arc<IdentityManager>,
}

impl DebugTools {
    /// Decode and display envelope
    pub fn decode_envelope(&self, base64: &str) -> Result<EnvelopeDebugInfo, DebugError>;
    
    /// Compute transcript hash
    pub fn compute_transcript(&self, inputs: &[&[u8]]) -> [u8; 32];
    
    /// Compute SAS from transcript
    pub fn compute_sas(&self, transcript: &[u8; 32]) -> String;
    
    /// Test transport connectivity
    pub async fn test_transport(&self, url: &str) -> Result<TransportTestResult, DebugError>;
    
    /// Capture packets to file
    pub async fn capture_packets(
        &self,
        output: &Path,
        duration: Duration,
    ) -> Result<u32, DebugError>;
}

pub struct EnvelopeDebugInfo {
    pub version: u32,
    pub msg_type: String,
    pub sender_id: String,
    pub recipient_id: String,
    pub timestamp: SystemTime,
    pub payload_size: usize,
    pub signature_valid: Option<bool>,
}

pub struct TransportTestResult {
    pub reachable: bool,
    pub latency_ms: Option<u64>,
    pub protocol_version: Option<String>,
    pub error: Option<String>,
}
```

## Data Models

### Configuration Schema

```toml
# Controller configuration (controller.toml)

[identity]
key_path = ""  # Empty = default location
key_store = "os"  # "os" | "file"

[transport]
default = "auto"  # "auto" | "mesh" | "rendezvous" | "direct" | "relay"
rendezvous_urls = ["https://rendezvous.zippyremote.io"]
relay_urls = ["https://relay.zippyremote.io"]
mesh_nodes = []
timeout_seconds = 30

[output]
format = "table"  # "table" | "json" | "quiet"
verbose = false
colors = true

[pairings]
db_path = ""  # Empty = default location

[logging]
level = "warn"
file = ""
```

### Exit Codes

```rust
pub enum ExitCode {
    Success = 0,
    GeneralError = 1,
    AuthenticationFailed = 2,
    Timeout = 3,
    ConnectionFailed = 4,
    InvalidInput = 5,
    NotPaired = 6,
    PermissionDenied = 7,
}
```

## Correctness Properties

### Property 1: Invite Validation
*For any* imported invite, the controller SHALL verify: valid protobuf encoding, non-expired timestamp, valid signature if present.
**Validates: Requirements 1.5, 1.6**

### Property 2: Pairing Proof Correctness
*For any* pairing request, the invite_proof SHALL be correctly computed as HMAC(invite_secret, transcript).
**Validates: Requirement 2.2**

### Property 3: Session Ticket Verification
*For any* QUIC connection, the controller SHALL verify the server certificate matches the provided fingerprint before sending the ticket.
**Validates: Requirements 4.3, 4.4**

### Property 4: Transport Ladder Order
*When* transport is "auto", the controller SHALL attempt transports in order: mesh → direct → rendezvous → relay.
**Validates: Requirement 8.3**

### Property 5: Output Format Consistency
*For any* command with --output json, the output SHALL be valid JSON matching the documented schema.
**Validates: Requirements 9.1, 9.4**

### Property 6: Exit Code Accuracy
*For any* command execution, the exit code SHALL accurately reflect the outcome (0=success, non-zero=specific error).
**Validates: Requirement 9.6**

### Property 7: Configuration Override Precedence
*For any* configuration option, command-line arguments SHALL override config file values.
**Validates: Requirement 10.5**

### Property 8: Identity Persistence
*For any* identity operation, the operator_id SHALL remain stable across restarts unless explicitly rotated.
**Validates: Requirements 11.1, 11.2**

## Error Handling

| Error Condition | Exit Code | Message Format |
|-----------------|-----------|----------------|
| Invalid invite format | 5 | "Invalid invite: {reason}" |
| Invite expired | 5 | "Invite expired at {timestamp}" |
| Device not paired | 6 | "Device {id} is not paired" |
| Authentication failed | 2 | "Authentication failed: {reason}" |
| Connection timeout | 3 | "Connection timed out after {seconds}s" |
| Transport unreachable | 4 | "Cannot reach device via {transport}" |
| Permission denied | 7 | "Permission denied: {capability}" |
| Config file invalid | 1 | "Config error: {details}" |

## Testing Strategy

### Unit Tests
- CLI argument parsing for all commands
- Invite parsing from all sources (base64, file, QR)
- Output formatting for all formats
- Configuration loading and merging
- Pairing proof computation
- SAS computation

### Property-Based Tests
- Invite validation across malformed inputs
- Transport ladder ordering
- Output format consistency
- Exit code accuracy
- Configuration precedence

### Integration Tests
- Full pairing flow with mock agent
- Session establishment with mock agent
- QUIC connection with mock server
- Input command transmission
- Frame reception

### CLI Tests
- All command variations
- Error message formatting
- Exit code verification
- JSON output schema validation
- Verbose/debug output
