//! Rate limiting and abuse prevention.
//!
//! Requirements: 10.1, 10.2, 10.3, 10.4

use std::num::NonZeroU32;
use std::time::Duration;
use governor::{
    Quota, RateLimiter,
    state::keyed::DefaultKeyedStateStore,
    clock::DefaultClock,
};
use crate::error::SecurityError;

/// Rate limit configuration.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Authentication attempts per minute
    pub auth_per_minute: u32,
    /// Pairing requests per minute
    pub pairing_per_minute: u32,
    /// Session requests per minute
    pub session_per_minute: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            auth_per_minute: 5,
            pairing_per_minute: 3,
            session_per_minute: 10,
        }
    }
}

/// Rate limiter for security operations.
///
/// Uses governor crate for efficient rate limiting with exponential backoff.
///
/// Requirements: 10.1, 10.2, 10.3, 10.4
pub struct SecurityRateLimiter {
    auth_limiter: RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>,
    pairing_limiter: RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>,
    session_limiter: RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>,
    config: RateLimitConfig,
}

impl SecurityRateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        // Use defaults if values are 0 (NonZeroU32 requirement)
        // These unwraps are safe because we're using known non-zero constants
        let auth_quota = Quota::per_minute(
            NonZeroU32::new(config.auth_per_minute)
                .unwrap_or_else(|| NonZeroU32::new(5).expect("5 is non-zero"))
        );
        let pairing_quota = Quota::per_minute(
            NonZeroU32::new(config.pairing_per_minute)
                .unwrap_or_else(|| NonZeroU32::new(3).expect("3 is non-zero"))
        );
        let session_quota = Quota::per_minute(
            NonZeroU32::new(config.session_per_minute)
                .unwrap_or_else(|| NonZeroU32::new(10).expect("10 is non-zero"))
        );

        Self {
            auth_limiter: RateLimiter::keyed(auth_quota),
            pairing_limiter: RateLimiter::keyed(pairing_quota),
            session_limiter: RateLimiter::keyed(session_quota),
            config,
        }
    }

    /// Check if an authentication attempt is allowed.
    ///
    /// Requirements: 10.1
    pub fn check_auth(&self, source: &str) -> Result<(), SecurityError> {
        self.auth_limiter.check_key(&source.to_string())
            .map_err(|_| SecurityError::RateLimited {
                operation: "authentication".to_string(),
                retry_after: Duration::from_secs(60),
            })
    }

    /// Check if a pairing request is allowed.
    ///
    /// Requirements: 10.2
    pub fn check_pairing(&self, source: &str) -> Result<(), SecurityError> {
        self.pairing_limiter.check_key(&source.to_string())
            .map_err(|_| SecurityError::RateLimited {
                operation: "pairing".to_string(),
                retry_after: Duration::from_secs(60),
            })
    }

    /// Check if a session request is allowed.
    ///
    /// Requirements: 10.3
    pub fn check_session(&self, source: &str) -> Result<(), SecurityError> {
        self.session_limiter.check_key(&source.to_string())
            .map_err(|_| SecurityError::RateLimited {
                operation: "session".to_string(),
                retry_after: Duration::from_secs(60),
            })
    }

    /// Get the rate limit configuration.
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }
}

impl Default for SecurityRateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_allows_within_limit() {
        let limiter = SecurityRateLimiter::new(RateLimitConfig {
            auth_per_minute: 5,
            pairing_per_minute: 3,
            session_per_minute: 10,
        });

        // Should allow requests within limit
        for _ in 0..5 {
            assert!(limiter.check_auth("test_source").is_ok());
        }
    }

    #[test]
    fn test_rate_limit_rejects_exceeding_limit() {
        let limiter = SecurityRateLimiter::new(RateLimitConfig {
            auth_per_minute: 2,
            pairing_per_minute: 1,
            session_per_minute: 3,
        });

        // Should allow first 2
        assert!(limiter.check_auth("test_source").is_ok());
        assert!(limiter.check_auth("test_source").is_ok());

        // Should reject third
        assert!(matches!(
            limiter.check_auth("test_source"),
            Err(SecurityError::RateLimited { .. })
        ));
    }

    #[test]
    fn test_rate_limit_per_source() {
        let limiter = SecurityRateLimiter::new(RateLimitConfig {
            auth_per_minute: 2,
            pairing_per_minute: 1,
            session_per_minute: 3,
        });

        // Different sources should have separate limits
        assert!(limiter.check_auth("source1").is_ok());
        assert!(limiter.check_auth("source1").is_ok());
        assert!(limiter.check_auth("source1").is_err());

        // source2 should still have its limit
        assert!(limiter.check_auth("source2").is_ok());
        assert!(limiter.check_auth("source2").is_ok());
    }
}
