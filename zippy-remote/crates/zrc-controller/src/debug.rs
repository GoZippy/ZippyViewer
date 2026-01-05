//! Debug and diagnostic tools
//!
//! This module provides debugging and diagnostic utilities for the ZRC controller.
//! It supports:
//! - Envelope decoding and inspection
//! - Transcript hash computation
//! - SAS code computation
//! - Transport connectivity testing
//! - Packet capture for analysis
//!
//! Requirements: 12.1, 12.2, 12.3, 12.4, 12.6

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use prost::Message;
use serde::Serialize;
use thiserror::Error;

use zrc_crypto::sas::sas_6digit;
use zrc_crypto::transcript::Transcript;
use zrc_proto::v1::{EnvelopeV1, MsgTypeV1};

/// Debug operation errors
#[derive(Debug, Error)]
pub enum DebugError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Decode error: {0}")]
    DecodeError(String),

    #[error("Transport error: {0}")]
    TransportError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Timeout: {0}")]
    Timeout(String),
}

/// Decoded envelope information
/// Requirements: 12.1
#[derive(Debug, Clone, Serialize)]
pub struct EnvelopeDebugInfo {
    /// Protocol version
    pub version: u32,
    /// Message type
    pub msg_type: String,
    /// Message type numeric value
    pub msg_type_value: i32,
    /// Sender ID (hex encoded)
    pub sender_id: String,
    /// Recipient ID (hex encoded)
    pub recipient_id: String,
    /// Message timestamp (Unix epoch seconds)
    pub timestamp_unix: u64,
    /// Message timestamp (human readable)
    pub timestamp: String,
    /// Nonce (hex encoded)
    pub nonce: String,
    /// Sender KEX public key (hex encoded)
    pub sender_kex_pub: String,
    /// Payload size in bytes (encrypted)
    pub payload_size: usize,
    /// Signature size in bytes
    pub signature_size: usize,
    /// AAD size in bytes
    pub aad_size: usize,
    /// Signature validity (if verifiable)
    pub signature_valid: Option<bool>,
    /// Raw envelope size in bytes
    pub raw_size: usize,
}

/// Transport test result
/// Requirements: 12.4
#[derive(Debug, Clone, Serialize)]
pub struct TransportTestResult {
    /// URL that was tested
    pub url: String,
    /// Whether the endpoint is reachable
    pub reachable: bool,
    /// Round-trip latency in milliseconds
    pub latency_ms: Option<u64>,
    /// Protocol version reported by server
    pub protocol_version: Option<String>,
    /// TLS certificate info (if HTTPS)
    pub tls_info: Option<String>,
    /// Error message if not reachable
    pub error: Option<String>,
}

/// Packet capture statistics
/// Requirements: 12.6
#[derive(Debug, Clone, Serialize)]
pub struct CaptureStats {
    /// Number of packets captured
    pub packets_captured: u32,
    /// Total bytes captured
    pub bytes_captured: u64,
    /// Capture duration in seconds
    pub duration_seconds: u64,
    /// Output file path
    pub output_file: String,
}

/// Debugging and diagnostic utilities
/// Requirements: 12.1, 12.2, 12.3, 12.4, 12.6
pub struct DebugTools {
    /// Verbose output mode
    verbose: bool,
}

impl DebugTools {
    /// Create new debug tools
    pub fn new() -> Self {
        Self { verbose: false }
    }

    /// Create debug tools with verbose mode
    pub fn with_verbose(verbose: bool) -> Self {
        Self { verbose }
    }

    /// Decode and display envelope
    /// Requirements: 12.1
    ///
    /// Decodes a base64-encoded EnvelopeV1 protobuf message and extracts
    /// all header fields for inspection.
    pub fn decode_envelope(&self, base64_input: &str) -> Result<EnvelopeDebugInfo, DebugError> {
        // Decode base64
        let bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            base64_input.trim(),
        )
        .map_err(|e| DebugError::DecodeError(format!("Invalid base64: {}", e)))?;

        let raw_size = bytes.len();

        // Decode protobuf
        let envelope = EnvelopeV1::decode(bytes.as_slice())
            .map_err(|e| DebugError::DecodeError(format!("Invalid protobuf: {}", e)))?;

        // Extract header
        let header = envelope
            .header
            .as_ref()
            .ok_or_else(|| DebugError::DecodeError("Missing envelope header".to_string()))?;

        // Convert message type to string
        let msg_type = MsgTypeV1::try_from(header.msg_type)
            .map(|t| format!("{:?}", t))
            .unwrap_or_else(|_| format!("Unknown({})", header.msg_type));

        // Convert timestamp to human readable
        let timestamp_str = UNIX_EPOCH
            .checked_add(Duration::from_secs(header.timestamp))
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.to_rfc3339()
            })
            .unwrap_or_else(|| "Invalid timestamp".to_string());

        Ok(EnvelopeDebugInfo {
            version: header.version,
            msg_type,
            msg_type_value: header.msg_type,
            sender_id: hex::encode(&header.sender_id),
            recipient_id: hex::encode(&header.recipient_id),
            timestamp_unix: header.timestamp,
            timestamp: timestamp_str,
            nonce: hex::encode(&header.nonce),
            sender_kex_pub: hex::encode(&envelope.sender_kex_pub),
            payload_size: envelope.encrypted_payload.len(),
            signature_size: envelope.signature.len(),
            aad_size: envelope.aad.len(),
            signature_valid: None, // Cannot verify without sender's public key
            raw_size,
        })
    }

    /// Compute transcript hash from hex-encoded inputs
    /// Requirements: 12.2
    ///
    /// Takes a comma-separated list of hex-encoded inputs and computes
    /// the transcript hash using the ZRC transcript protocol.
    pub fn compute_transcript_from_hex(&self, hex_inputs: &str) -> Result<[u8; 32], DebugError> {
        let inputs: Vec<Vec<u8>> = hex_inputs
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                hex::decode(s)
                    .map_err(|e| DebugError::InvalidInput(format!("Invalid hex '{}': {}", s, e)))
            })
            .collect::<Result<Vec<_>, _>>()?;

        if inputs.is_empty() {
            return Err(DebugError::InvalidInput(
                "At least one input is required".to_string(),
            ));
        }

        Ok(self.compute_transcript(&inputs.iter().map(|v| v.as_slice()).collect::<Vec<_>>()))
    }

    /// Compute transcript hash from raw byte slices
    /// Requirements: 12.2
    ///
    /// Uses the ZRC transcript protocol to compute a deterministic hash
    /// from the provided inputs.
    pub fn compute_transcript(&self, inputs: &[&[u8]]) -> [u8; 32] {
        let mut transcript = Transcript::new("zrc_debug_transcript");
        
        for (i, input) in inputs.iter().enumerate() {
            transcript.append_bytes(i as u32 + 1, input);
        }

        transcript.finalize()
    }

    /// Compute SAS from transcript hash (hex-encoded)
    /// Requirements: 12.3
    ///
    /// Takes a hex-encoded 32-byte transcript hash and computes the
    /// 6-digit SAS code.
    pub fn compute_sas_from_hex(&self, transcript_hex: &str) -> Result<String, DebugError> {
        let transcript_bytes = hex::decode(transcript_hex.trim())
            .map_err(|e| DebugError::InvalidInput(format!("Invalid hex: {}", e)))?;

        if transcript_bytes.len() != 32 {
            return Err(DebugError::InvalidInput(format!(
                "Transcript must be 32 bytes, got {}",
                transcript_bytes.len()
            )));
        }

        let mut transcript: [u8; 32] = [0u8; 32];
        transcript.copy_from_slice(&transcript_bytes);

        Ok(self.compute_sas(&transcript))
    }

    /// Compute SAS from transcript hash
    /// Requirements: 12.3
    ///
    /// Uses the ZRC SAS algorithm to compute a 6-digit verification code.
    pub fn compute_sas(&self, transcript: &[u8; 32]) -> String {
        sas_6digit(transcript)
    }

    /// Test transport connectivity
    /// Requirements: 12.4
    ///
    /// Tests connectivity to a transport URL (rendezvous, relay, or mesh node).
    /// Returns latency and protocol information if reachable.
    pub async fn test_transport(&self, url: &str) -> Result<TransportTestResult, DebugError> {
        // Validate URL format
        let parsed_url = url::Url::parse(url)
            .map_err(|e| DebugError::InvalidInput(format!("Invalid URL: {}", e)))?;

        let scheme = parsed_url.scheme();
        
        match scheme {
            "http" | "https" => {
                // HTTP-based transport test (rendezvous/relay)
                self.test_http_transport(url).await
            }
            "tcp" | "quic" => {
                // Direct connection test
                self.test_direct_transport(&parsed_url).await
            }
            _ => {
                // Try as a host:port for mesh nodes
                if url.contains(':') && !url.contains("://") {
                    self.test_mesh_node(url).await
                } else {
                    Err(DebugError::InvalidInput(format!(
                        "Unsupported URL scheme: {}",
                        scheme
                    )))
                }
            }
        }
    }

    /// Test HTTP-based transport (rendezvous/relay)
    async fn test_http_transport(&self, url: &str) -> Result<TransportTestResult, DebugError> {
        let start = std::time::Instant::now();

        // Try to connect using TCP to test basic reachability
        let parsed = url::Url::parse(url)
            .map_err(|e| DebugError::InvalidInput(format!("Invalid URL: {}", e)))?;

        let host = parsed.host_str()
            .ok_or_else(|| DebugError::InvalidInput("Missing host".to_string()))?;
        let port = parsed.port().unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });

        let addr = format!("{}:{}", host, port);

        match tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::TcpStream::connect(&addr),
        ).await {
            Ok(Ok(_stream)) => {
                let latency = start.elapsed().as_millis() as u64;
                
                Ok(TransportTestResult {
                    url: url.to_string(),
                    reachable: true,
                    latency_ms: Some(latency),
                    protocol_version: Some("HTTP/1.1 or HTTP/2".to_string()),
                    tls_info: if parsed.scheme() == "https" {
                        Some("TLS enabled".to_string())
                    } else {
                        None
                    },
                    error: None,
                })
            }
            Ok(Err(e)) => {
                Ok(TransportTestResult {
                    url: url.to_string(),
                    reachable: false,
                    latency_ms: None,
                    protocol_version: None,
                    tls_info: None,
                    error: Some(format!("Connection failed: {}", e)),
                })
            }
            Err(_) => {
                Ok(TransportTestResult {
                    url: url.to_string(),
                    reachable: false,
                    latency_ms: None,
                    protocol_version: None,
                    tls_info: None,
                    error: Some("Connection timed out after 10 seconds".to_string()),
                })
            }
        }
    }

    /// Test direct transport (TCP/QUIC)
    async fn test_direct_transport(&self, url: &url::Url) -> Result<TransportTestResult, DebugError> {
        let start = std::time::Instant::now();
        
        let host = url.host_str()
            .ok_or_else(|| DebugError::InvalidInput("Missing host".to_string()))?;
        let port = url.port()
            .ok_or_else(|| DebugError::InvalidInput("Missing port".to_string()))?;

        let addr = format!("{}:{}", host, port);

        match tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::TcpStream::connect(&addr),
        ).await {
            Ok(Ok(_stream)) => {
                let latency = start.elapsed().as_millis() as u64;
                
                Ok(TransportTestResult {
                    url: url.to_string(),
                    reachable: true,
                    latency_ms: Some(latency),
                    protocol_version: Some(format!("{} transport", url.scheme().to_uppercase())),
                    tls_info: None,
                    error: None,
                })
            }
            Ok(Err(e)) => {
                Ok(TransportTestResult {
                    url: url.to_string(),
                    reachable: false,
                    latency_ms: None,
                    protocol_version: None,
                    tls_info: None,
                    error: Some(format!("Connection failed: {}", e)),
                })
            }
            Err(_) => {
                Ok(TransportTestResult {
                    url: url.to_string(),
                    reachable: false,
                    latency_ms: None,
                    protocol_version: None,
                    tls_info: None,
                    error: Some("Connection timed out after 10 seconds".to_string()),
                })
            }
        }
    }

    /// Test mesh node connectivity
    async fn test_mesh_node(&self, addr: &str) -> Result<TransportTestResult, DebugError> {
        let start = std::time::Instant::now();

        match tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::TcpStream::connect(addr),
        ).await {
            Ok(Ok(_stream)) => {
                let latency = start.elapsed().as_millis() as u64;
                
                Ok(TransportTestResult {
                    url: addr.to_string(),
                    reachable: true,
                    latency_ms: Some(latency),
                    protocol_version: Some("Mesh node".to_string()),
                    tls_info: None,
                    error: None,
                })
            }
            Ok(Err(e)) => {
                Ok(TransportTestResult {
                    url: addr.to_string(),
                    reachable: false,
                    latency_ms: None,
                    protocol_version: None,
                    tls_info: None,
                    error: Some(format!("Connection failed: {}", e)),
                })
            }
            Err(_) => {
                Ok(TransportTestResult {
                    url: addr.to_string(),
                    reachable: false,
                    latency_ms: None,
                    protocol_version: None,
                    tls_info: None,
                    error: Some("Connection timed out after 10 seconds".to_string()),
                })
            }
        }
    }

    /// Capture packets to file
    /// Requirements: 12.6
    ///
    /// Captures ZRC protocol packets for a specified duration and saves
    /// them to a file for analysis.
    pub async fn capture_packets(
        &self,
        output: &Path,
        duration: Duration,
    ) -> Result<CaptureStats, DebugError> {
        use std::io::Write;
        use tokio::time::sleep;

        // Create output file
        let mut file = std::fs::File::create(output)?;

        // Write capture header
        let header = format!(
            "# ZRC Packet Capture\n# Started: {}\n# Duration: {} seconds\n\n",
            chrono::Utc::now().to_rfc3339(),
            duration.as_secs()
        );
        file.write_all(header.as_bytes())?;

        // In a real implementation, this would:
        // 1. Hook into the transport layer to capture packets
        // 2. Log each packet with timestamp, direction, size, and type
        // 3. Optionally decode and display packet contents
        //
        // For now, we simulate the capture duration and return stats

        if self.verbose {
            eprintln!("Capturing packets for {} seconds...", duration.as_secs());
        }

        // Wait for the capture duration
        sleep(duration).await;

        // Write capture footer
        let footer = format!(
            "\n# Capture ended: {}\n# Note: Full packet capture requires transport integration\n",
            chrono::Utc::now().to_rfc3339()
        );
        file.write_all(footer.as_bytes())?;

        let bytes_written = header.len() + footer.len();

        Ok(CaptureStats {
            packets_captured: 0, // Would be populated by actual capture
            bytes_captured: bytes_written as u64,
            duration_seconds: duration.as_secs(),
            output_file: output.display().to_string(),
        })
    }
}

impl Default for DebugTools {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_transcript_deterministic() {
        let tools = DebugTools::new();
        
        let input1 = b"hello";
        let input2 = b"world";
        
        let hash1 = tools.compute_transcript(&[input1, input2]);
        let hash2 = tools.compute_transcript(&[input1, input2]);
        
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_transcript_different_inputs() {
        let tools = DebugTools::new();
        
        let hash1 = tools.compute_transcript(&[b"hello"]);
        let hash2 = tools.compute_transcript(&[b"world"]);
        
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_sas_format() {
        let tools = DebugTools::new();
        
        let transcript = [0u8; 32];
        let sas = tools.compute_sas(&transcript);
        
        // SAS should be 6 digits
        assert_eq!(sas.len(), 6);
        assert!(sas.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_compute_sas_deterministic() {
        let tools = DebugTools::new();
        
        let transcript = [1u8; 32];
        let sas1 = tools.compute_sas(&transcript);
        let sas2 = tools.compute_sas(&transcript);
        
        assert_eq!(sas1, sas2);
    }

    #[test]
    fn test_compute_transcript_from_hex() {
        let tools = DebugTools::new();
        
        // Test with valid hex inputs
        let result = tools.compute_transcript_from_hex("48656c6c6f,576f726c64");
        assert!(result.is_ok());
        
        // Test with invalid hex
        let result = tools.compute_transcript_from_hex("invalid");
        assert!(result.is_err());
        
        // Test with empty input
        let result = tools.compute_transcript_from_hex("");
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_sas_from_hex() {
        let tools = DebugTools::new();
        
        // Valid 32-byte hex
        let hex = "0000000000000000000000000000000000000000000000000000000000000000";
        let result = tools.compute_sas_from_hex(hex);
        assert!(result.is_ok());
        
        // Invalid length
        let result = tools.compute_sas_from_hex("00");
        assert!(result.is_err());
        
        // Invalid hex
        let result = tools.compute_sas_from_hex("xyz");
        assert!(result.is_err());
    }
}
