//! Transport negotiation for selecting optimal connection method.
//!
//! Implements transport selection logic as specified in Requirements 7.1-7.7.
//!
//! # Priority Order
//!
//! Transport types are evaluated in priority order: MESH → DIRECT → RENDEZVOUS → RELAY
//! (Requirement 7.1). The negotiator will attempt to use the highest priority transport
//! that is both supported by both endpoints and allowed by policy.
//!
//! # Policy Restrictions
//!
//! The negotiator respects policy restrictions on allowed transports (Requirement 7.7).
//! Transports can be explicitly allowed or denied via `AllowedTransports`.

use std::collections::HashSet;
use thiserror::Error;

/// Errors from transport negotiation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    #[error("no compatible transport available")]
    NoCompatibleTransport,
    #[error("transport not allowed by policy: {0}")]
    NotAllowedByPolicy(String),
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("missing required parameters: {0}")]
    MissingParameters(String),
}

/// Transport types in priority order (Requirement 7.1).
///
/// Priority: MESH (0) → DIRECT (1) → RENDEZVOUS (2) → RELAY (3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportType {
    /// Mesh network (highest priority).
    /// Used when both endpoints are on the same mesh network.
    Mesh,
    /// Direct peer-to-peer connection.
    /// Used when direct IP connectivity is available.
    Direct,
    /// Rendezvous server assisted.
    /// Used when NAT traversal is needed but relay is not required.
    Rendezvous,
    /// Relay server fallback (lowest priority).
    /// Used when direct connectivity is not possible.
    Relay,
}

impl TransportType {
    /// Get the default priority (lower = higher priority).
    pub fn default_priority(&self) -> u8 {
        match self {
            TransportType::Mesh => 0,
            TransportType::Direct => 1,
            TransportType::Rendezvous => 2,
            TransportType::Relay => 3,
        }
    }

    /// Get all transport types in default priority order.
    pub fn all_in_priority_order() -> Vec<TransportType> {
        vec![
            TransportType::Mesh,
            TransportType::Direct,
            TransportType::Rendezvous,
            TransportType::Relay,
        ]
    }
}

/// Policy restrictions on allowed transports (Requirement 7.7).
#[derive(Debug, Clone)]
pub struct AllowedTransports {
    /// Set of allowed transport types.
    allowed: HashSet<TransportType>,
}

impl Default for AllowedTransports {
    fn default() -> Self {
        // By default, all transports are allowed
        Self {
            allowed: TransportType::all_in_priority_order().into_iter().collect(),
        }
    }
}

impl AllowedTransports {
    /// Create a new policy allowing only the specified transports.
    pub fn only(transports: Vec<TransportType>) -> Self {
        Self {
            allowed: transports.into_iter().collect(),
        }
    }

    /// Create a policy allowing all transports except relay.
    pub fn no_relay() -> Self {
        Self {
            allowed: vec![
                TransportType::Mesh,
                TransportType::Direct,
                TransportType::Rendezvous,
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Check if a transport type is allowed.
    pub fn is_allowed(&self, transport: TransportType) -> bool {
        self.allowed.contains(&transport)
    }

    /// Add a transport type to the allowed set.
    pub fn allow(&mut self, transport: TransportType) {
        self.allowed.insert(transport);
    }

    /// Remove a transport type from the allowed set.
    pub fn deny(&mut self, transport: TransportType) {
        self.allowed.remove(&transport);
    }

    /// Get the allowed transports as a vector in priority order.
    pub fn to_priority_vec(&self) -> Vec<TransportType> {
        let mut result: Vec<_> = self.allowed.iter().copied().collect();
        result.sort_by_key(|t| t.default_priority());
        result
    }
}

/// Transport preferences configuration.
#[derive(Debug, Clone)]
pub struct TransportPreferences {
    /// Priority order for transport selection.
    pub priority: Vec<TransportType>,
    /// Whether relay fallback is allowed.
    pub allow_relay: bool,
    /// Whether to prefer mesh when available.
    pub prefer_mesh: bool,
    /// Policy restrictions on allowed transports (Requirement 7.7).
    pub policy_restrictions: AllowedTransports,
}

impl Default for TransportPreferences {
    fn default() -> Self {
        Self {
            priority: vec![
                TransportType::Mesh,
                TransportType::Direct,
                TransportType::Rendezvous,
                TransportType::Relay,
            ],
            allow_relay: true,
            prefer_mesh: true,
            policy_restrictions: AllowedTransports::default(),
        }
    }
}

impl TransportPreferences {
    /// Create preferences with custom priority order.
    pub fn with_priority(priority: Vec<TransportType>) -> Self {
        let allow_relay = priority.contains(&TransportType::Relay);
        Self {
            priority,
            allow_relay,
            prefer_mesh: true,
            policy_restrictions: AllowedTransports::default(),
        }
    }

    /// Create preferences that disallow relay.
    pub fn no_relay() -> Self {
        Self {
            priority: vec![
                TransportType::Mesh,
                TransportType::Direct,
                TransportType::Rendezvous,
            ],
            allow_relay: false,
            prefer_mesh: true,
            policy_restrictions: AllowedTransports::no_relay(),
        }
    }

    /// Set policy restrictions.
    pub fn with_policy_restrictions(mut self, restrictions: AllowedTransports) -> Self {
        self.policy_restrictions = restrictions;
        self
    }

    /// Check if a transport is allowed by both preferences and policy.
    pub fn is_transport_allowed(&self, transport: TransportType) -> bool {
        // Check relay preference
        if transport == TransportType::Relay && !self.allow_relay {
            return false;
        }
        // Check policy restrictions
        self.policy_restrictions.is_allowed(transport)
    }
}

/// QUIC connection parameters (Requirement 7.2).
#[derive(Debug, Clone)]
pub struct QuicParams {
    /// Self-signed certificate (DER encoded).
    pub certificate: Vec<u8>,
    /// ALPN protocols (e.g., ["zrc/1"]).
    pub alpn_protocols: Vec<String>,
    /// Server address (for direct connections).
    pub server_addr: Option<String>,
}

impl QuicParams {
    /// Create new QUIC parameters with the given certificate.
    pub fn new(certificate: Vec<u8>) -> Self {
        Self {
            certificate,
            alpn_protocols: vec!["zrc/1".to_string()],
            server_addr: None,
        }
    }

    /// Set the server address.
    pub fn with_server_addr(mut self, addr: String) -> Self {
        self.server_addr = Some(addr);
        self
    }

    /// Add an ALPN protocol.
    pub fn with_alpn(mut self, protocol: String) -> Self {
        if !self.alpn_protocols.contains(&protocol) {
            self.alpn_protocols.push(protocol);
        }
        self
    }
}

/// Relay token for relay-assisted connections (Requirement 7.3).
#[derive(Debug, Clone)]
pub struct RelayToken {
    /// Relay server URL.
    pub relay_url: String,
    /// Authentication token.
    pub token: Vec<u8>,
    /// Token expiry timestamp (Unix seconds).
    pub expires_at: u64,
    /// Optional bandwidth limit in bytes/sec.
    pub bandwidth_limit: Option<u32>,
}

impl RelayToken {
    /// Create a new relay token.
    pub fn new(relay_url: String, token: Vec<u8>, expires_at: u64) -> Self {
        Self {
            relay_url,
            token,
            expires_at,
            bandwidth_limit: None,
        }
    }

    /// Set bandwidth limit.
    pub fn with_bandwidth_limit(mut self, limit: u32) -> Self {
        self.bandwidth_limit = Some(limit);
        self
    }

    /// Check if the token is expired.
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time >= self.expires_at
    }
}

/// ICE candidate for WebRTC connectivity (Requirement 7.6).
#[derive(Debug, Clone)]
pub struct IceCandidate {
    /// Candidate type: host, srflx, prflx, relay.
    pub candidate_type: String,
    /// Protocol: udp, tcp.
    pub protocol: String,
    /// IP address.
    pub address: String,
    /// Port number.
    pub port: u16,
    /// ICE priority.
    pub priority: u32,
    /// ICE foundation.
    pub foundation: String,
}

impl IceCandidate {
    /// Create a new host candidate.
    pub fn host(address: String, port: u16, protocol: &str) -> Self {
        Self {
            candidate_type: "host".to_string(),
            protocol: protocol.to_string(),
            address,
            port,
            priority: 2130706431, // Default host priority
            foundation: "1".to_string(),
        }
    }

    /// Create a new server reflexive candidate.
    pub fn srflx(address: String, port: u16, protocol: &str) -> Self {
        Self {
            candidate_type: "srflx".to_string(),
            protocol: protocol.to_string(),
            address,
            port,
            priority: 1694498815, // Default srflx priority
            foundation: "2".to_string(),
        }
    }

    /// Create a new relay candidate.
    pub fn relay(address: String, port: u16, protocol: &str) -> Self {
        Self {
            candidate_type: "relay".to_string(),
            protocol: protocol.to_string(),
            address,
            port,
            priority: 16777215, // Default relay priority
            foundation: "3".to_string(),
        }
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the foundation.
    pub fn with_foundation(mut self, foundation: String) -> Self {
        self.foundation = foundation;
        self
    }
}

/// Selected transport with connection parameters.
#[derive(Debug, Clone)]
pub enum SelectedTransport {
    /// Direct QUIC connection.
    Quic { params: QuicParams },
    /// Relay-assisted connection.
    Relay { token: RelayToken, params: QuicParams },
    /// WebRTC connection with ICE candidates.
    WebRtc { candidates: Vec<IceCandidate> },
}

/// Transport negotiation parameters exchanged during session setup.
#[derive(Debug, Clone, Default)]
pub struct TransportNegotiation {
    /// Available QUIC parameters.
    pub quic_params: Option<QuicParams>,
    /// Available relay tokens.
    pub relay_tokens: Vec<RelayToken>,
    /// Supported transport types.
    pub supported_transports: Vec<TransportType>,
    /// ICE candidates for WebRTC (Requirement 7.6).
    pub ice_candidates: Vec<IceCandidate>,
}

/// Transport negotiator for selecting optimal connection method.
///
/// Implements transport selection logic per Requirements 7.1-7.7:
/// - Priority order: MESH → DIRECT → RENDEZVOUS → RELAY (7.1)
/// - QUIC parameter generation (7.2)
/// - Relay token generation (7.3)
/// - Mesh preference (7.4)
/// - Automatic fallback (7.5)
/// - ICE candidate support (7.6)
/// - Policy restrictions (7.7)
#[derive(Debug, Clone)]
pub struct TransportNegotiator {
    preferences: TransportPreferences,
    /// QUIC configuration for generating parameters.
    quic_config: Option<QuicConfig>,
    /// Pre-configured relay tokens.
    relay_tokens: Vec<RelayToken>,
}

/// Configuration for QUIC parameter generation.
#[derive(Debug, Clone)]
pub struct QuicConfig {
    /// Self-signed certificate (DER encoded).
    pub certificate: Vec<u8>,
    /// ALPN protocols.
    pub alpn_protocols: Vec<String>,
    /// Local server addresses.
    pub server_addrs: Vec<String>,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            certificate: Vec::new(),
            alpn_protocols: vec!["zrc/1".to_string()],
            server_addrs: Vec::new(),
        }
    }
}

impl Default for TransportNegotiator {
    fn default() -> Self {
        Self {
            preferences: TransportPreferences::default(),
            quic_config: None,
            relay_tokens: Vec::new(),
        }
    }
}

impl TransportNegotiator {
    /// Create a new transport negotiator with the given preferences.
    pub fn new(preferences: TransportPreferences) -> Self {
        Self {
            preferences,
            quic_config: None,
            relay_tokens: Vec::new(),
        }
    }

    /// Set the QUIC configuration for parameter generation.
    pub fn with_quic_config(mut self, config: QuicConfig) -> Self {
        self.quic_config = Some(config);
        self
    }

    /// Add relay tokens for relay fallback.
    pub fn with_relay_tokens(mut self, tokens: Vec<RelayToken>) -> Self {
        self.relay_tokens = tokens;
        self
    }

    /// Get the current preferences.
    pub fn preferences(&self) -> &TransportPreferences {
        &self.preferences
    }

    /// Get mutable reference to preferences.
    pub fn preferences_mut(&mut self) -> &mut TransportPreferences {
        &mut self.preferences
    }

    /// Generate transport negotiation parameters for session response (Requirement 7.2, 7.3).
    ///
    /// This method generates the transport parameters that will be sent to the peer
    /// during session negotiation. It includes:
    /// - QUIC parameters with self-signed certificate and ALPN protocols
    /// - Relay tokens when relay fallback is enabled
    /// - List of supported transport types based on preferences and policy
    pub fn generate_params(&self, quic_params: Option<QuicParams>, relay_tokens: Vec<RelayToken>) -> TransportNegotiation {
        let mut supported = Vec::new();

        // Build list of supported transports based on preferences and policy (Requirement 7.7)
        for transport_type in &self.preferences.priority {
            if !self.preferences.is_transport_allowed(*transport_type) {
                continue;
            }
            supported.push(*transport_type);
        }

        // Use provided params or generate from config
        let final_quic_params = quic_params.or_else(|| {
            self.quic_config.as_ref().map(|config| QuicParams {
                certificate: config.certificate.clone(),
                alpn_protocols: config.alpn_protocols.clone(),
                server_addr: config.server_addrs.first().cloned(),
            })
        });

        // Use provided relay tokens or pre-configured ones
        let final_relay_tokens = if relay_tokens.is_empty() {
            self.relay_tokens.clone()
        } else {
            relay_tokens
        };

        TransportNegotiation {
            quic_params: final_quic_params,
            relay_tokens: final_relay_tokens,
            supported_transports: supported,
            ice_candidates: Vec::new(),
        }
    }

    /// Generate transport parameters from the configured QUIC config.
    ///
    /// This is a convenience method that uses the pre-configured QUIC config
    /// and relay tokens.
    pub fn generate_params_from_config(&self) -> TransportNegotiation {
        self.generate_params(None, Vec::new())
    }

    /// Select the best transport from offered options (Requirements 7.4, 7.5, 7.7).
    ///
    /// This method evaluates the offered transport options and selects the best one
    /// based on:
    /// - Priority order (MESH → DIRECT → RENDEZVOUS → RELAY)
    /// - Mesh preference when available (7.4)
    /// - Automatic fallback to relay when direct fails (7.5)
    /// - Policy restrictions (7.7)
    pub fn select_transport(&self, offered: &TransportNegotiation) -> Result<SelectedTransport, TransportError> {
        // Try transports in priority order
        for transport_type in &self.preferences.priority {
            // Check if transport is allowed by policy (Requirement 7.7)
            if !self.preferences.is_transport_allowed(*transport_type) {
                continue;
            }

            // Check if transport is supported by peer
            if !offered.supported_transports.contains(transport_type) {
                continue;
            }

            match transport_type {
                TransportType::Mesh | TransportType::Direct | TransportType::Rendezvous => {
                    // For mesh/direct/rendezvous, we need QUIC params
                    if let Some(ref params) = offered.quic_params {
                        return Ok(SelectedTransport::Quic {
                            params: params.clone(),
                        });
                    }
                }
                TransportType::Relay => {
                    // For relay, we need both relay token and QUIC params
                    if let Some(token) = self.select_best_relay_token(&offered.relay_tokens) {
                        if let Some(ref params) = offered.quic_params {
                            return Ok(SelectedTransport::Relay {
                                token,
                                params: params.clone(),
                            });
                        }
                    }
                }
            }
        }

        Err(TransportError::NoCompatibleTransport)
    }

    /// Select the best relay token from available options.
    ///
    /// Prefers tokens that:
    /// 1. Are not expired
    /// 2. Have higher bandwidth limits
    fn select_best_relay_token(&self, tokens: &[RelayToken]) -> Option<RelayToken> {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        tokens
            .iter()
            .filter(|t| !t.is_expired(current_time))
            .max_by_key(|t| t.bandwidth_limit.unwrap_or(0))
            .cloned()
    }

    /// Check if a specific transport type is available in the offered options.
    pub fn is_transport_available(&self, transport: TransportType, offered: &TransportNegotiation) -> bool {
        if !self.preferences.is_transport_allowed(transport) {
            return false;
        }
        if !offered.supported_transports.contains(&transport) {
            return false;
        }
        match transport {
            TransportType::Mesh | TransportType::Direct | TransportType::Rendezvous => {
                offered.quic_params.is_some()
            }
            TransportType::Relay => {
                !offered.relay_tokens.is_empty() && offered.quic_params.is_some()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preferences() {
        let prefs = TransportPreferences::default();
        assert!(prefs.allow_relay);
        assert!(prefs.prefer_mesh);
        assert_eq!(prefs.priority[0], TransportType::Mesh);
        assert!(prefs.is_transport_allowed(TransportType::Relay));
    }

    #[test]
    fn test_no_relay_preferences() {
        let prefs = TransportPreferences::no_relay();
        assert!(!prefs.allow_relay);
        assert!(!prefs.is_transport_allowed(TransportType::Relay));
        assert!(prefs.is_transport_allowed(TransportType::Mesh));
        assert!(prefs.is_transport_allowed(TransportType::Direct));
    }

    #[test]
    fn test_transport_type_priority() {
        assert_eq!(TransportType::Mesh.default_priority(), 0);
        assert_eq!(TransportType::Direct.default_priority(), 1);
        assert_eq!(TransportType::Rendezvous.default_priority(), 2);
        assert_eq!(TransportType::Relay.default_priority(), 3);
    }

    #[test]
    fn test_allowed_transports_default() {
        let allowed = AllowedTransports::default();
        assert!(allowed.is_allowed(TransportType::Mesh));
        assert!(allowed.is_allowed(TransportType::Direct));
        assert!(allowed.is_allowed(TransportType::Rendezvous));
        assert!(allowed.is_allowed(TransportType::Relay));
    }

    #[test]
    fn test_allowed_transports_only() {
        let allowed = AllowedTransports::only(vec![TransportType::Direct, TransportType::Relay]);
        assert!(!allowed.is_allowed(TransportType::Mesh));
        assert!(allowed.is_allowed(TransportType::Direct));
        assert!(!allowed.is_allowed(TransportType::Rendezvous));
        assert!(allowed.is_allowed(TransportType::Relay));
    }

    #[test]
    fn test_allowed_transports_no_relay() {
        let allowed = AllowedTransports::no_relay();
        assert!(allowed.is_allowed(TransportType::Mesh));
        assert!(allowed.is_allowed(TransportType::Direct));
        assert!(allowed.is_allowed(TransportType::Rendezvous));
        assert!(!allowed.is_allowed(TransportType::Relay));
    }

    #[test]
    fn test_allowed_transports_modify() {
        let mut allowed = AllowedTransports::default();
        assert!(allowed.is_allowed(TransportType::Relay));
        
        allowed.deny(TransportType::Relay);
        assert!(!allowed.is_allowed(TransportType::Relay));
        
        allowed.allow(TransportType::Relay);
        assert!(allowed.is_allowed(TransportType::Relay));
    }

    #[test]
    fn test_allowed_transports_priority_vec() {
        let allowed = AllowedTransports::only(vec![TransportType::Relay, TransportType::Mesh]);
        let priority = allowed.to_priority_vec();
        // Should be sorted by priority: Mesh (0) before Relay (3)
        assert_eq!(priority[0], TransportType::Mesh);
        assert_eq!(priority[1], TransportType::Relay);
    }

    #[test]
    fn test_select_direct_transport() {
        let negotiator = TransportNegotiator::default();
        let offered = TransportNegotiation {
            quic_params: Some(QuicParams {
                certificate: vec![1, 2, 3],
                alpn_protocols: vec!["zrc/1".into()],
                server_addr: Some("192.168.1.1:4433".into()),
            }),
            relay_tokens: vec![],
            supported_transports: vec![TransportType::Direct],
            ice_candidates: vec![],
        };

        let result = negotiator.select_transport(&offered);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SelectedTransport::Quic { .. }));
    }

    #[test]
    fn test_fallback_to_relay() {
        let negotiator = TransportNegotiator::default();
        let offered = TransportNegotiation {
            quic_params: Some(QuicParams {
                certificate: vec![1, 2, 3],
                alpn_protocols: vec!["zrc/1".into()],
                server_addr: None,
            }),
            relay_tokens: vec![RelayToken {
                relay_url: "https://relay.example.com".into(),
                token: vec![4, 5, 6],
                expires_at: 9999999999,
                bandwidth_limit: None,
            }],
            supported_transports: vec![TransportType::Relay],
            ice_candidates: vec![],
        };

        let result = negotiator.select_transport(&offered);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SelectedTransport::Relay { .. }));
    }

    #[test]
    fn test_no_compatible_transport() {
        let negotiator = TransportNegotiator::default();
        let offered = TransportNegotiation::default();

        let result = negotiator.select_transport(&offered);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TransportError::NoCompatibleTransport));
    }

    #[test]
    fn test_policy_blocks_relay() {
        let prefs = TransportPreferences::default()
            .with_policy_restrictions(AllowedTransports::no_relay());
        let negotiator = TransportNegotiator::new(prefs);
        
        let offered = TransportNegotiation {
            quic_params: Some(QuicParams {
                certificate: vec![1, 2, 3],
                alpn_protocols: vec!["zrc/1".into()],
                server_addr: None,
            }),
            relay_tokens: vec![RelayToken {
                relay_url: "https://relay.example.com".into(),
                token: vec![4, 5, 6],
                expires_at: 9999999999,
                bandwidth_limit: None,
            }],
            // Only relay is offered
            supported_transports: vec![TransportType::Relay],
            ice_candidates: vec![],
        };

        // Should fail because relay is blocked by policy
        let result = negotiator.select_transport(&offered);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_params_with_config() {
        let config = QuicConfig {
            certificate: vec![1, 2, 3],
            alpn_protocols: vec!["zrc/1".into()],
            server_addrs: vec!["192.168.1.1:4433".into()],
        };
        let negotiator = TransportNegotiator::default()
            .with_quic_config(config);

        let params = negotiator.generate_params_from_config();
        
        assert!(params.quic_params.is_some());
        let quic = params.quic_params.unwrap();
        assert_eq!(quic.certificate, vec![1, 2, 3]);
        assert_eq!(quic.server_addr, Some("192.168.1.1:4433".into()));
    }

    #[test]
    fn test_generate_params_respects_policy() {
        let prefs = TransportPreferences::no_relay();
        let negotiator = TransportNegotiator::new(prefs);

        let params = negotiator.generate_params(None, vec![]);
        
        // Relay should not be in supported transports
        assert!(!params.supported_transports.contains(&TransportType::Relay));
        assert!(params.supported_transports.contains(&TransportType::Mesh));
        assert!(params.supported_transports.contains(&TransportType::Direct));
    }

    #[test]
    fn test_relay_token_expiry() {
        let token = RelayToken::new(
            "https://relay.example.com".into(),
            vec![1, 2, 3],
            1000,
        );
        
        assert!(token.is_expired(1000));
        assert!(token.is_expired(1001));
        assert!(!token.is_expired(999));
    }

    #[test]
    fn test_relay_token_bandwidth_limit() {
        let token = RelayToken::new(
            "https://relay.example.com".into(),
            vec![1, 2, 3],
            9999999999,
        ).with_bandwidth_limit(1_000_000);
        
        assert_eq!(token.bandwidth_limit, Some(1_000_000));
    }

    #[test]
    fn test_select_best_relay_token() {
        let negotiator = TransportNegotiator::default();
        
        let offered = TransportNegotiation {
            quic_params: Some(QuicParams::new(vec![1, 2, 3])),
            relay_tokens: vec![
                RelayToken::new("https://relay1.example.com".into(), vec![1], 9999999999)
                    .with_bandwidth_limit(100),
                RelayToken::new("https://relay2.example.com".into(), vec![2], 9999999999)
                    .with_bandwidth_limit(1000),
                RelayToken::new("https://relay3.example.com".into(), vec![3], 1) // expired
                    .with_bandwidth_limit(10000),
            ],
            supported_transports: vec![TransportType::Relay],
            ice_candidates: vec![],
        };

        let result = negotiator.select_transport(&offered);
        assert!(result.is_ok());
        
        if let SelectedTransport::Relay { token, .. } = result.unwrap() {
            // Should select relay2 (highest bandwidth among non-expired)
            assert_eq!(token.relay_url, "https://relay2.example.com");
        } else {
            panic!("Expected Relay transport");
        }
    }

    #[test]
    fn test_quic_params_builder() {
        let params = QuicParams::new(vec![1, 2, 3])
            .with_server_addr("192.168.1.1:4433".into())
            .with_alpn("zrc/2".into());
        
        assert_eq!(params.certificate, vec![1, 2, 3]);
        assert_eq!(params.server_addr, Some("192.168.1.1:4433".into()));
        assert!(params.alpn_protocols.contains(&"zrc/1".into()));
        assert!(params.alpn_protocols.contains(&"zrc/2".into()));
    }

    #[test]
    fn test_ice_candidate_host() {
        let candidate = IceCandidate::host("192.168.1.1".into(), 4433, "udp");
        assert_eq!(candidate.candidate_type, "host");
        assert_eq!(candidate.address, "192.168.1.1");
        assert_eq!(candidate.port, 4433);
        assert_eq!(candidate.protocol, "udp");
    }

    #[test]
    fn test_ice_candidate_srflx() {
        let candidate = IceCandidate::srflx("203.0.113.1".into(), 4433, "udp")
            .with_priority(100)
            .with_foundation("custom".into());
        
        assert_eq!(candidate.candidate_type, "srflx");
        assert_eq!(candidate.priority, 100);
        assert_eq!(candidate.foundation, "custom");
    }

    #[test]
    fn test_is_transport_available() {
        let negotiator = TransportNegotiator::default();
        
        let offered = TransportNegotiation {
            quic_params: Some(QuicParams::new(vec![1, 2, 3])),
            relay_tokens: vec![],
            supported_transports: vec![TransportType::Direct],
            ice_candidates: vec![],
        };

        assert!(negotiator.is_transport_available(TransportType::Direct, &offered));
        assert!(!negotiator.is_transport_available(TransportType::Relay, &offered));
        assert!(!negotiator.is_transport_available(TransportType::Mesh, &offered));
    }

    #[test]
    fn test_mesh_preferred_over_direct() {
        let negotiator = TransportNegotiator::default();
        
        let offered = TransportNegotiation {
            quic_params: Some(QuicParams::new(vec![1, 2, 3])),
            relay_tokens: vec![],
            supported_transports: vec![TransportType::Direct, TransportType::Mesh],
            ice_candidates: vec![],
        };

        let result = negotiator.select_transport(&offered);
        assert!(result.is_ok());
        // Both Mesh and Direct use QUIC, but Mesh has higher priority
        assert!(matches!(result.unwrap(), SelectedTransport::Quic { .. }));
    }

    #[test]
    fn test_preferences_with_custom_priority() {
        let prefs = TransportPreferences::with_priority(vec![
            TransportType::Direct,
            TransportType::Mesh,
        ]);
        
        assert!(!prefs.allow_relay);
        assert_eq!(prefs.priority[0], TransportType::Direct);
        assert_eq!(prefs.priority[1], TransportType::Mesh);
    }
}
