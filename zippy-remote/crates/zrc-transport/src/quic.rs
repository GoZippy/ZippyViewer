//! QUIC transport helpers.

use std::time::Duration;

/// ALPN protocol identifier for ZRC
pub const ZRC_ALPN: &[u8] = b"zrc/1";

/// QUIC stream ID mapping utilities
pub struct StreamMapper;

impl StreamMapper {
    /// Map channel type to stream ID
    /// Uses odd stream IDs for client-initiated, even for server-initiated
    pub fn channel_to_stream_id(channel: crate::mux::ChannelType, is_client: bool) -> u64 {
        let base = channel as u64;
        if is_client {
            base * 2 + 1
        } else {
            base * 2
        }
    }

    /// Map stream ID to channel type
    pub fn stream_id_to_channel(stream_id: u64) -> Option<crate::mux::ChannelType> {
        let channel_num = stream_id / 2;
        match channel_num {
            0 => Some(crate::mux::ChannelType::Control),
            1 => Some(crate::mux::ChannelType::Frames),
            2 => Some(crate::mux::ChannelType::Clipboard),
            3 => Some(crate::mux::ChannelType::Files),
            4 => Some(crate::mux::ChannelType::Audio),
            _ => None,
        }
    }
}

/// QUIC configuration helpers
pub struct QuicConfig {
    pub max_idle_timeout: Duration,
    pub keep_alive_interval: Duration,
    pub initial_rtt: Duration,
    pub max_udp_payload_size: usize,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            max_idle_timeout: Duration::from_secs(30),
            keep_alive_interval: Duration::from_secs(10),
            initial_rtt: Duration::from_millis(100),
            max_udp_payload_size: 1200,
        }
    }
}

impl QuicConfig {
    /// Create configuration optimized for low latency
    pub fn low_latency() -> Self {
        Self {
            max_idle_timeout: Duration::from_secs(10),
            keep_alive_interval: Duration::from_secs(5),
            initial_rtt: Duration::from_millis(50),
            max_udp_payload_size: 1200,
        }
    }

    /// Create configuration optimized for throughput
    pub fn high_throughput() -> Self {
        Self {
            max_idle_timeout: Duration::from_secs(60),
            keep_alive_interval: Duration::from_secs(30),
            initial_rtt: Duration::from_millis(200),
            max_udp_payload_size: 1400,
        }
    }
}

/// Certificate pinning verification
pub struct CertificatePinner;

impl CertificatePinner {
    /// Verify certificate pin (simplified - real implementation would use actual cert pinning)
    pub fn verify_pin(cert_der: &[u8], expected_pin: &[u8]) -> bool {
        // In real implementation, this would:
        // 1. Extract public key from certificate
        // 2. Hash the public key
        // 3. Compare with expected pin
        // For now, return true as placeholder
        !cert_der.is_empty() && !expected_pin.is_empty()
    }
}

/// QUIC error mapping
pub fn map_quic_error(error: &str) -> crate::traits::TransportError {
    if error.contains("timeout") || error.contains("timed out") {
        crate::traits::TransportError::Timeout
    } else if error.contains("connection") || error.contains("disconnect") {
        crate::traits::TransportError::Disconnected
    } else {
        crate::traits::TransportError::Other(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_mapping() {
        let stream_id = StreamMapper::channel_to_stream_id(crate::mux::ChannelType::Control, true);
        assert_eq!(stream_id, 1);
        
        let channel = StreamMapper::stream_id_to_channel(1);
        assert_eq!(channel, Some(crate::mux::ChannelType::Control));
    }

    #[test]
    fn test_quic_config() {
        let config = QuicConfig::default();
        assert_eq!(config.max_idle_timeout, Duration::from_secs(30));
        
        let low_latency = QuicConfig::low_latency();
        assert!(low_latency.max_idle_timeout < config.max_idle_timeout);
    }
}
