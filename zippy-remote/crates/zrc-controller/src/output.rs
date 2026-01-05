//! Output formatting for CLI results
//!
//! This module provides consistent output formatting across all CLI commands.
//! It supports three output formats:
//! - Table: Human-readable tables (default)
//! - JSON: Structured JSON for scripting and automation
//! - Quiet: Minimal output, exit codes only
//!
//! Requirements: 9.1, 9.2, 9.3, 9.4

use std::str::FromStr;

use comfy_table::{presets::UTF8_FULL, Table};
use serde::Serialize;

use crate::identity::IdentityInfo;
use crate::pairing::ParsedInvite;
use crate::pairings::StoredPairing;
use crate::session::SessionInitResult;
use crate::ExitCode;

/// Output format options
/// Requirements: 9.1, 9.2, 9.3
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable table format (Requirements: 9.2)
    #[default]
    Table,
    /// JSON format for scripting (Requirements: 9.1, 9.4)
    Json,
    /// Minimal output - exit codes only (Requirements: 9.3)
    Quiet,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(Self::Table),
            "json" => Ok(Self::Json),
            "quiet" => Ok(Self::Quiet),
            _ => Err(format!("Unknown output format: {s}")),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Table => write!(f, "table"),
            Self::Json => write!(f, "json"),
            Self::Quiet => write!(f, "quiet"),
        }
    }
}

/// Standard JSON response wrapper for consistent schema
/// Requirements: 9.4, 9.5
#[derive(Serialize)]
pub struct JsonResponse<T: Serialize> {
    /// Whether the operation was successful
    pub success: bool,
    /// The response data (present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error message (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Command that was executed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

impl<T: Serialize> JsonResponse<T> {
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            command: None,
        }
    }

    /// Create a successful response with command context
    pub fn success_with_command(data: T, command: &str) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            command: Some(command.to_string()),
        }
    }
}

impl JsonResponse<()> {
    /// Create an error response
    pub fn error(message: &str) -> JsonResponse<()> {
        JsonResponse {
            success: false,
            data: None,
            error: Some(message.to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            command: None,
        }
    }

    /// Create an error response with command context
    pub fn error_with_command(message: &str, command: &str) -> JsonResponse<()> {
        JsonResponse {
            success: false,
            data: None,
            error: Some(message.to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            command: Some(command.to_string()),
        }
    }
}

/// Formats output for different modes
/// Requirements: 9.1, 9.2, 9.3, 9.4
pub struct OutputFormatter {
    format: OutputFormat,
    verbose: bool,
}

impl OutputFormatter {
    /// Create a new output formatter
    pub fn new(format: OutputFormat, verbose: bool) -> Self {
        Self { format, verbose }
    }

    /// Get the current output format
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// Check if verbose mode is enabled
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// Check if quiet mode is enabled
    pub fn is_quiet(&self) -> bool {
        self.format == OutputFormat::Quiet
    }

    /// Format pairing list
    /// Requirements: 9.1, 9.2
    pub fn format_pairings(&self, pairings: &[StoredPairing]) -> String {
        match self.format {
            OutputFormat::Table => self.pairings_table(pairings),
            OutputFormat::Json => self.to_json_response(&PairingsOutput::from(pairings), "pairings list"),
            OutputFormat::Quiet => String::new(),
        }
    }

    /// Format session info
    /// Requirements: 9.1, 9.2
    pub fn format_session(&self, session: &SessionInitResult) -> String {
        match self.format {
            OutputFormat::Table => self.session_table(session),
            OutputFormat::Json => self.to_json_response(&SessionOutput::from(session), "session start"),
            OutputFormat::Quiet => String::new(),
        }
    }

    /// Format identity info
    /// Requirements: 9.1, 9.2
    pub fn format_identity(&self, info: &IdentityInfo) -> String {
        match self.format {
            OutputFormat::Table => self.identity_table(info),
            OutputFormat::Json => self.to_json_response(&IdentityOutput::from(info), "identity show"),
            OutputFormat::Quiet => String::new(),
        }
    }

    /// Format invite info
    /// Requirements: 9.1, 9.2
    pub fn format_invite(&self, invite: &ParsedInvite) -> String {
        match self.format {
            OutputFormat::Table => self.invite_table(invite),
            OutputFormat::Json => self.to_json_response(&InviteOutput::from(invite), "pair invite"),
            OutputFormat::Quiet => String::new(),
        }
    }

    /// Format single pairing detail
    /// Requirements: 9.1, 9.2
    pub fn format_pairing_detail(&self, pairing: &StoredPairing) -> String {
        match self.format {
            OutputFormat::Table => self.pairing_detail_table(pairing),
            OutputFormat::Json => self.to_json_response(&PairingDetailOutput::from(pairing), "pairings show"),
            OutputFormat::Quiet => String::new(),
        }
    }

    /// Format a generic success result
    /// Requirements: 9.1, 9.4
    pub fn format_success<T: Serialize>(&self, data: &T, command: &str) -> String {
        match self.format {
            OutputFormat::Table => String::new(), // Table format handles success differently
            OutputFormat::Json => self.to_json_response(data, command),
            OutputFormat::Quiet => String::new(),
        }
    }

    /// Format error with exit code context
    /// Requirements: 9.1, 9.6
    pub fn format_error_with_code(&self, error: &dyn std::error::Error, code: ExitCode) -> String {
        match self.format {
            OutputFormat::Table => format!("Error: {error}"),
            OutputFormat::Json => {
                let response = JsonResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(error.to_string()),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    command: None,
                };
                let mut output: serde_json::Value = serde_json::to_value(&response).unwrap();
                output["exit_code"] = serde_json::json!(code as i32);
                output["exit_code_name"] = serde_json::json!(format!("{:?}", code));
                serde_json::to_string_pretty(&output).unwrap()
            }
            OutputFormat::Quiet => String::new(),
        }
    }

    /// Format error
    pub fn format_error(&self, error: &dyn std::error::Error) -> String {
        match self.format {
            OutputFormat::Table => format!("Error: {error}"),
            OutputFormat::Json => self.to_json(&ErrorOutput {
                error: error.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            }),
            OutputFormat::Quiet => String::new(),
        }
    }

    /// Format progress message (only shown in verbose mode)
    /// Requirements: 9.7
    pub fn progress(&self, message: &str) {
        if self.verbose && self.format == OutputFormat::Table {
            eprintln!("... {message}");
        }
    }

    /// Format success message
    pub fn success(&self, message: &str) {
        if self.format == OutputFormat::Table {
            println!("✓ {message}");
        }
    }

    /// Format error message
    pub fn error(&self, message: &str) {
        if self.format == OutputFormat::Table {
            eprintln!("✗ {message}");
        } else if self.format == OutputFormat::Json {
            println!("{}", self.to_json(&ErrorOutput {
                error: message.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            }));
        }
        // Quiet mode: no output, rely on exit code
    }

    /// Format warning message
    pub fn warning(&self, message: &str) {
        if self.format == OutputFormat::Table {
            eprintln!("⚠ {message}");
        } else if self.format == OutputFormat::Json {
            println!("{}", self.to_json(&WarningOutput {
                warning: message.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            }));
        }
        // Quiet mode: no output
    }

    /// Format info message (only in verbose mode)
    /// Requirements: 9.7
    pub fn info(&self, message: &str) {
        if self.verbose {
            match self.format {
                OutputFormat::Table => println!("ℹ {message}"),
                OutputFormat::Json => {
                    println!("{}", self.to_json(&InfoOutput {
                        info: message.to_string(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    }));
                }
                OutputFormat::Quiet => {}
            }
        }
    }

    /// Format debug message (only in debug mode, handled by tracing)
    /// Requirements: 9.8
    pub fn debug(&self, message: &str) {
        tracing::debug!("{}", message);
    }

    fn to_json<T: Serialize>(&self, value: &T) -> String {
        serde_json::to_string_pretty(value).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
    }

    /// Format data with consistent JSON response wrapper
    /// Requirements: 9.4
    fn to_json_response<T: Serialize>(&self, value: &T, command: &str) -> String {
        let response = JsonResponse::success_with_command(value, command);
        serde_json::to_string_pretty(&response).unwrap_or_else(|e| {
            let err_response = JsonResponse::<()>::error(&format!("Serialization error: {e}"));
            serde_json::to_string_pretty(&err_response).unwrap()
        })
    }

    fn pairings_table(&self, pairings: &[StoredPairing]) -> String {
        if pairings.is_empty() {
            return "No pairings found.".to_string();
        }

        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec!["Device ID", "Name", "Permissions", "Paired At", "Sessions"]);

        for p in pairings {
            table.add_row(vec![
                &p.device_id,
                p.device_name.as_deref().unwrap_or("-"),
                &p.permissions.join(", "),
                &format_time(p.paired_at),
                &p.session_count.to_string(),
            ]);
        }

        table.to_string()
    }

    fn session_table(&self, session: &SessionInitResult) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec!["Property", "Value"]);
        table.add_row(vec!["Session ID", &session.session_id]);
        table.add_row(vec!["QUIC Endpoint", &format!("{}:{}", session.quic_host, session.quic_port)]);
        table.add_row(vec!["Cert Fingerprint", &hex::encode(session.cert_fingerprint)]);
        table.add_row(vec!["Capabilities", &session.granted_capabilities.join(", ")]);
        table.to_string()
    }

    fn identity_table(&self, info: &IdentityInfo) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec!["Property", "Value"]);
        table.add_row(vec!["Operator ID", &info.operator_id]);
        table.add_row(vec!["Fingerprint", &info.fingerprint]);
        table.add_row(vec!["Algorithm", &info.key_algorithm]);
        table.add_row(vec!["Created At", &format_time(info.created_at)]);
        table.to_string()
    }

    fn invite_table(&self, invite: &ParsedInvite) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec!["Property", "Value"]);
        table.add_row(vec!["Device ID", &invite.device_id]);
        table.add_row(vec!["Expires At", &format_time(invite.expires_at)]);
        
        // Show time until expiry
        if let Some(duration) = invite.time_until_expiry() {
            let hours = duration.as_secs() / 3600;
            let minutes = (duration.as_secs() % 3600) / 60;
            table.add_row(vec!["Time Until Expiry", &format!("{}h {}m", hours, minutes)]);
        } else {
            table.add_row(vec!["Time Until Expiry", "EXPIRED"]);
        }
        
        table.add_row(vec!["Transport Hints", &invite.transport_hints.join("\n")]);
        table.to_string()
    }

    fn pairing_detail_table(&self, pairing: &StoredPairing) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec!["Property", "Value"]);
        table.add_row(vec!["Device ID", &pairing.device_id]);
        table.add_row(vec!["Device Name", pairing.device_name.as_deref().unwrap_or("-")]);
        table.add_row(vec!["Signing Key", &hex::encode(pairing.device_sign_pub)]);
        table.add_row(vec!["KEX Key", &hex::encode(pairing.device_kex_pub)]);
        table.add_row(vec!["Permissions", &pairing.permissions.join(", ")]);
        table.add_row(vec!["Paired At", &format_time(pairing.paired_at)]);
        table.add_row(vec!["Last Session", &pairing.last_session.map(format_time).unwrap_or_else(|| "Never".to_string())]);
        table.add_row(vec!["Session Count", &pairing.session_count.to_string()]);
        table.to_string()
    }
}

fn format_time(time: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Format time as ISO 8601 for JSON output
/// Requirements: 9.5
fn format_time_iso(time: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.to_rfc3339()
}

// JSON output structures
// Requirements: 9.4 - Consistent JSON schema across all commands

#[derive(Serialize)]
struct PairingsOutput {
    pairings: Vec<PairingJson>,
    count: usize,
}

#[derive(Serialize)]
struct PairingJson {
    device_id: String,
    device_name: Option<String>,
    permissions: Vec<String>,
    paired_at: String,
    paired_at_iso: String,
    last_session: Option<String>,
    last_session_iso: Option<String>,
    session_count: u32,
}

impl From<&[StoredPairing]> for PairingsOutput {
    fn from(pairings: &[StoredPairing]) -> Self {
        Self {
            count: pairings.len(),
            pairings: pairings.iter().map(PairingJson::from).collect(),
        }
    }
}

impl From<&StoredPairing> for PairingJson {
    fn from(p: &StoredPairing) -> Self {
        Self {
            device_id: p.device_id.clone(),
            device_name: p.device_name.clone(),
            permissions: p.permissions.clone(),
            paired_at: format_time(p.paired_at),
            paired_at_iso: format_time_iso(p.paired_at),
            last_session: p.last_session.map(format_time),
            last_session_iso: p.last_session.map(format_time_iso),
            session_count: p.session_count,
        }
    }
}

#[derive(Serialize)]
struct SessionOutput {
    session_id: String,
    quic_endpoint: String,
    quic_host: String,
    quic_port: u16,
    cert_fingerprint: String,
    capabilities: Vec<String>,
}

impl From<&SessionInitResult> for SessionOutput {
    fn from(s: &SessionInitResult) -> Self {
        Self {
            session_id: s.session_id.clone(),
            quic_endpoint: format!("{}:{}", s.quic_host, s.quic_port),
            quic_host: s.quic_host.clone(),
            quic_port: s.quic_port,
            cert_fingerprint: hex::encode(s.cert_fingerprint),
            capabilities: s.granted_capabilities.clone(),
        }
    }
}

#[derive(Serialize)]
struct IdentityOutput {
    operator_id: String,
    fingerprint: String,
    algorithm: String,
    created_at: String,
    created_at_iso: String,
}

impl From<&IdentityInfo> for IdentityOutput {
    fn from(i: &IdentityInfo) -> Self {
        Self {
            operator_id: i.operator_id.clone(),
            fingerprint: i.fingerprint.clone(),
            algorithm: i.key_algorithm.clone(),
            created_at: format_time(i.created_at),
            created_at_iso: format_time_iso(i.created_at),
        }
    }
}

#[derive(Serialize)]
struct InviteOutput {
    device_id: String,
    expires_at: String,
    expires_at_iso: String,
    is_expired: bool,
    transport_hints: Vec<String>,
}

impl From<&ParsedInvite> for InviteOutput {
    fn from(i: &ParsedInvite) -> Self {
        Self {
            device_id: i.device_id.clone(),
            expires_at: format_time(i.expires_at),
            expires_at_iso: format_time_iso(i.expires_at),
            is_expired: i.is_expired(),
            transport_hints: i.transport_hints.clone(),
        }
    }
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
    timestamp: String,
}

#[derive(Serialize)]
struct WarningOutput {
    warning: String,
    timestamp: String,
}

#[derive(Serialize)]
struct InfoOutput {
    info: String,
    timestamp: String,
}

#[derive(Serialize)]
struct PairingDetailOutput {
    device_id: String,
    device_name: Option<String>,
    device_sign_pub: String,
    device_kex_pub: String,
    permissions: Vec<String>,
    paired_at: String,
    paired_at_iso: String,
    last_session: Option<String>,
    last_session_iso: Option<String>,
    session_count: u32,
}

impl From<&StoredPairing> for PairingDetailOutput {
    fn from(p: &StoredPairing) -> Self {
        Self {
            device_id: p.device_id.clone(),
            device_name: p.device_name.clone(),
            device_sign_pub: hex::encode(p.device_sign_pub),
            device_kex_pub: hex::encode(p.device_kex_pub),
            permissions: p.permissions.clone(),
            paired_at: format_time(p.paired_at),
            paired_at_iso: format_time_iso(p.paired_at),
            last_session: p.last_session.map(format_time),
            last_session_iso: p.last_session.map(format_time_iso),
            session_count: p.session_count,
        }
    }
}

/// Simple success message for JSON output
#[derive(Serialize)]
pub struct SuccessMessage {
    pub message: String,
}

impl SuccessMessage {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

/// Frame statistics output
#[derive(Serialize)]
pub struct FrameStatsOutput {
    pub frame_count: u64,
    pub frame_rate: f64,
    pub resolution: String,
    pub bandwidth_kbps: f64,
    pub dropped_frames: u64,
}

/// Transport test result output
#[derive(Serialize)]
pub struct TransportTestOutput {
    pub url: String,
    pub reachable: bool,
    pub latency_ms: Option<u64>,
    pub protocol_version: Option<String>,
    pub error: Option<String>,
}

/// Debug envelope output
#[derive(Serialize)]
pub struct EnvelopeDebugOutput {
    pub version: u32,
    pub msg_type: String,
    pub sender_id: String,
    pub recipient_id: String,
    pub timestamp: String,
    pub payload_size: usize,
    pub signature_valid: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_parsing() {
        assert_eq!(OutputFormat::from_str("table").unwrap(), OutputFormat::Table);
        assert_eq!(OutputFormat::from_str("json").unwrap(), OutputFormat::Json);
        assert_eq!(OutputFormat::from_str("quiet").unwrap(), OutputFormat::Quiet);
        assert_eq!(OutputFormat::from_str("TABLE").unwrap(), OutputFormat::Table);
        assert_eq!(OutputFormat::from_str("JSON").unwrap(), OutputFormat::Json);
        assert!(OutputFormat::from_str("invalid").is_err());
    }

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Table.to_string(), "table");
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Quiet.to_string(), "quiet");
    }

    #[test]
    fn test_json_response_success() {
        let response = JsonResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_json_response_error() {
        let response = JsonResponse::<()>::error("test error");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
    }

    #[test]
    fn test_formatter_quiet_mode() {
        let formatter = OutputFormatter::new(OutputFormat::Quiet, false);
        assert!(formatter.is_quiet());
        
        // Quiet mode should return empty strings
        let pairings: Vec<StoredPairing> = vec![];
        assert_eq!(formatter.format_pairings(&pairings), "");
    }

    #[test]
    fn test_formatter_json_consistency() {
        let formatter = OutputFormatter::new(OutputFormat::Json, false);
        
        // All JSON outputs should be valid JSON
        let pairings: Vec<StoredPairing> = vec![];
        let output = formatter.format_pairings(&pairings);
        assert!(serde_json::from_str::<serde_json::Value>(&output).is_ok());
    }
}
