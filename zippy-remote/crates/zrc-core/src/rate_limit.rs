//! Rate limiting for protection against brute-force and DoS attacks.
//!
//! Implements rate limiting as specified in Requirements 10.1-10.7.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{warn, info};

/// Errors from rate limiting.
#[derive(Debug, Clone, Error)]
pub enum RateLimitError {
    #[error("rate limit exceeded, retry after {retry_after_secs} seconds")]
    RateLimited {
        #[allow(dead_code)]
        source_id: String,
        retry_after_secs: u64,
    },
}

/// Configuration for rate limiting.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum pairing attempts per minute per source.
    pub pairing_attempts_per_minute: u32,
    /// Maximum session requests per minute per source.
    pub session_requests_per_minute: u32,
    /// Window duration for rate limiting.
    pub window_duration: Duration,
    /// Base backoff duration for exponential backoff.
    pub base_backoff: Duration,
    /// Maximum backoff duration.
    pub max_backoff: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            pairing_attempts_per_minute: 3,
            session_requests_per_minute: 10,
            window_duration: Duration::from_secs(60),
            base_backoff: Duration::from_secs(5),
            max_backoff: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Request type for rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestType {
    Pairing,
    Session,
}


/// Tracking data for a single source.
#[derive(Debug, Clone)]
struct SourceTracker {
    /// Request timestamps within the current window.
    requests: Vec<Instant>,
    /// Number of consecutive violations.
    violations: u32,
    /// When the backoff expires (if in backoff).
    backoff_until: Option<Instant>,
}

impl Default for SourceTracker {
    fn default() -> Self {
        Self {
            requests: Vec::new(),
            violations: 0,
            backoff_until: None,
        }
    }
}

/// Rate limiter for protecting against abuse.
pub struct RateLimiter {
    config: RateLimitConfig,
    /// Tracking data per (source, request_type).
    trackers: RwLock<HashMap<(String, RequestType), SourceTracker>>,
    /// Allowlisted sources (bypass rate limiting).
    allowlist: RwLock<HashSet<String>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            trackers: RwLock::new(HashMap::new()),
            allowlist: RwLock::new(HashSet::new()),
        }
    }

    /// Add a source to the allowlist.
    pub async fn add_to_allowlist(&self, source: String) {
        info!(source = %source, "Added source to rate limit allowlist");
        self.allowlist.write().await.insert(source);
    }

    /// Remove a source from the allowlist.
    pub async fn remove_from_allowlist(&self, source: &str) {
        info!(source = %source, "Removed source from rate limit allowlist");
        self.allowlist.write().await.remove(source);
    }

    /// Check if a source is allowlisted.
    pub async fn is_allowlisted(&self, source: &str) -> bool {
        self.allowlist.read().await.contains(source)
    }

    /// Get the limit for a request type.
    fn get_limit(&self, request_type: RequestType) -> u32 {
        match request_type {
            RequestType::Pairing => self.config.pairing_attempts_per_minute,
            RequestType::Session => self.config.session_requests_per_minute,
        }
    }

    /// Calculate backoff duration based on violation count.
    fn calculate_backoff(&self, violations: u32) -> Duration {
        let backoff = self.config.base_backoff.as_secs_f64() * 2.0_f64.powi(violations as i32);
        let backoff_secs = backoff.min(self.config.max_backoff.as_secs_f64());
        Duration::from_secs_f64(backoff_secs)
    }


    /// Check if a request is allowed under rate limiting.
    ///
    /// Returns `Ok(())` if allowed, `Err(RateLimitError)` if rate limited.
    pub async fn check_rate_limit(
        &self,
        source: &str,
        request_type: RequestType,
    ) -> Result<(), RateLimitError> {
        // Allowlisted sources bypass rate limiting
        if self.is_allowlisted(source).await {
            return Ok(());
        }

        let now = Instant::now();
        let key = (source.to_string(), request_type);
        let limit = self.get_limit(request_type);

        let mut trackers = self.trackers.write().await;
        let tracker = trackers.entry(key.clone()).or_default();

        // Check if in backoff period
        if let Some(backoff_until) = tracker.backoff_until {
            if now < backoff_until {
                let retry_after = backoff_until.duration_since(now).as_secs();
                
                // Log blocked request during backoff (Requirement 10.7)
                warn!(
                    source = %source,
                    request_type = ?request_type,
                    retry_after_secs = retry_after,
                    "Request blocked during backoff period"
                );
                
                return Err(RateLimitError::RateLimited {
                    source_id: source.to_string(),
                    retry_after_secs: retry_after,
                });
            }
            // Backoff expired, clear it
            tracker.backoff_until = None;
        }

        // Clean up old requests outside the window
        let window_start = now - self.config.window_duration;
        tracker.requests.retain(|&t| t > window_start);

        // Check if limit exceeded
        if tracker.requests.len() >= limit as usize {
            tracker.violations += 1;
            let backoff = self.calculate_backoff(tracker.violations);
            tracker.backoff_until = Some(now + backoff);

            // Log rate limit violation for security monitoring (Requirement 10.7)
            warn!(
                source = %source,
                request_type = ?request_type,
                violations = tracker.violations,
                backoff_secs = backoff.as_secs(),
                "Rate limit exceeded"
            );

            return Err(RateLimitError::RateLimited {
                source_id: source.to_string(),
                retry_after_secs: backoff.as_secs(),
            });
        }

        // Record this request
        tracker.requests.push(now);

        // Reset violations on successful request
        if tracker.violations > 0 {
            tracker.violations = 0;
        }

        Ok(())
    }

    /// Reset rate limiting for a source.
    pub async fn reset(&self, source: &str, request_type: RequestType) {
        let key = (source.to_string(), request_type);
        self.trackers.write().await.remove(&key);
    }

    /// Reset all rate limiting data.
    pub async fn reset_all(&self) {
        self.trackers.write().await.clear();
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_allows_under_limit() {
        let limiter = RateLimiter::default();
        let source = "test_operator";

        // Should allow up to 3 pairing attempts
        for _ in 0..3 {
            assert!(limiter.check_rate_limit(source, RequestType::Pairing).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_rate_limit_blocks_over_limit() {
        let config = RateLimitConfig {
            pairing_attempts_per_minute: 2,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let source = "test_operator";

        // First 2 should succeed
        assert!(limiter.check_rate_limit(source, RequestType::Pairing).await.is_ok());
        assert!(limiter.check_rate_limit(source, RequestType::Pairing).await.is_ok());

        // Third should fail
        let result = limiter.check_rate_limit(source, RequestType::Pairing).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_allowlist_bypasses_limit() {
        let config = RateLimitConfig {
            pairing_attempts_per_minute: 1,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let source = "trusted_operator";

        limiter.add_to_allowlist(source.to_string()).await;

        // Should allow unlimited requests for allowlisted source
        for _ in 0..10 {
            assert!(limiter.check_rate_limit(source, RequestType::Pairing).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_reset_clears_tracking() {
        let config = RateLimitConfig {
            pairing_attempts_per_minute: 1,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let source = "test_operator";

        // Use up the limit
        assert!(limiter.check_rate_limit(source, RequestType::Pairing).await.is_ok());
        assert!(limiter.check_rate_limit(source, RequestType::Pairing).await.is_err());

        // Reset
        limiter.reset(source, RequestType::Pairing).await;

        // Should allow again
        assert!(limiter.check_rate_limit(source, RequestType::Pairing).await.is_ok());
    }

    #[tokio::test]
    async fn test_different_request_types_tracked_separately() {
        let config = RateLimitConfig {
            pairing_attempts_per_minute: 1,
            session_requests_per_minute: 1,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let source = "test_operator";

        // Use up pairing limit
        assert!(limiter.check_rate_limit(source, RequestType::Pairing).await.is_ok());
        assert!(limiter.check_rate_limit(source, RequestType::Pairing).await.is_err());

        // Session should still be allowed
        assert!(limiter.check_rate_limit(source, RequestType::Session).await.is_ok());
    }
}
