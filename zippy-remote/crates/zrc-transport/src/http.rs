//! HTTP mailbox helpers for rendezvous transport.

use std::time::Duration;

/// HTTP client configuration
pub struct HttpClientConfig {
    pub connection_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub max_connections: usize,
    pub enable_proxy: bool,
    pub proxy_url: Option<String>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(10),
            max_connections: 10,
            enable_proxy: false,
            proxy_url: None,
        }
    }
}

impl HttpClientConfig {
    /// Create configuration optimized for long-polling
    pub fn long_poll() -> Self {
        Self {
            connection_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(300), // 5 minutes
            write_timeout: Duration::from_secs(10),
            max_connections: 5,
            enable_proxy: false,
            proxy_url: None,
        }
    }
}

/// Long-poll retry configuration
pub struct LongPollConfig {
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub max_retries: u32,
    pub backoff_multiplier: f64,
}

impl Default for LongPollConfig {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            max_retries: 10,
            backoff_multiplier: 2.0,
        }
    }
}

impl LongPollConfig {
    /// Calculate backoff delay for retry attempt
    pub fn backoff_delay(&self, attempt: u32) -> Duration {
        if attempt >= self.max_retries {
            return self.max_backoff;
        }

        let delay_secs = self.initial_backoff.as_secs_f64()
            * self.backoff_multiplier.powi(attempt as i32);
        let delay = Duration::from_secs_f64(delay_secs.min(self.max_backoff.as_secs_f64()));
        delay
    }
}

/// Request signing utilities (placeholder)
pub struct RequestSigner;

impl RequestSigner {
    /// Sign HTTP request (simplified - real implementation would use actual signing)
    pub fn sign_request(
        method: &str,
        path: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _secret: &[u8],
    ) -> String {
        // In real implementation, this would:
        // 1. Create canonical request string
        // 2. Hash with HMAC using secret
        // 3. Return signature
        format!("signature-{}-{}", method, path)
    }

    /// Verify request signature
    pub fn verify_signature(
        signature: &str,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        secret: &[u8],
    ) -> bool {
        let expected = Self::sign_request(method, path, headers, body, secret);
        signature == expected
    }
}

/// Response parsing utilities
pub struct ResponseParser;

impl ResponseParser {
    /// Parse mailbox response (simplified)
    pub fn parse_mailbox_response(_data: &[u8]) -> Result<MailboxResponse, String> {
        // In real implementation, this would parse JSON or protobuf
        // For now, return a simple response
        Ok(MailboxResponse {
            messages: Vec::new(),
            next_poll_after: Duration::from_secs(1),
        })
    }
}

/// Mailbox response structure
pub struct MailboxResponse {
    pub messages: Vec<Vec<u8>>,
    pub next_poll_after: Duration,
}

/// Proxy configuration
pub struct ProxyConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub no_proxy: Vec<String>,
}

impl ProxyConfig {
    /// Create proxy config from environment variables
    pub fn from_env() -> Self {
        Self {
            http_proxy: std::env::var("HTTP_PROXY").ok(),
            https_proxy: std::env::var("HTTPS_PROXY").ok(),
            no_proxy: std::env::var("NO_PROXY")
                .ok()
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
        }
    }

    /// Check if proxy should be used for URL
    pub fn should_use_proxy(&self, url: &str) -> bool {
        // Check if URL is in no_proxy list
        for no_proxy in &self.no_proxy {
            if url.contains(no_proxy) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_long_poll_config() {
        let config = LongPollConfig::default();
        let delay1 = config.backoff_delay(0);
        let delay2 = config.backoff_delay(1);
        assert!(delay2 > delay1);
    }

    #[test]
    fn test_request_signing() {
        let signature = RequestSigner::sign_request("GET", "/mailbox", &[], b"", b"secret");
        assert!(signature.starts_with("signature-"));
        
        let verified = RequestSigner::verify_signature(
            &signature,
            "GET",
            "/mailbox",
            &[],
            b"",
            b"secret",
        );
        assert!(verified);
    }

    #[test]
    fn test_proxy_config() {
        let config = ProxyConfig::from_env();
        // Just verify it doesn't panic
        let _ = config.should_use_proxy("http://example.com");
    }
}
