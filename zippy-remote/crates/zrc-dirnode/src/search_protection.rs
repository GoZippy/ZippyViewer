//! Search protection and enumeration prevention

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
pub enum ProtectionError {
    #[error("Rate limit exceeded")]
    RateLimited,
    #[error("Enumeration attempt detected")]
    EnumerationDetected,
    #[error("IP blocked")]
    IpBlocked,
}

/// Rate limiter for lookup requests
struct RateLimiter {
    requests: DashMap<IpAddr, (u32, Instant)>,
    limit: u32,
    window: Duration,
}

impl RateLimiter {
    fn new(limit: u32, window: Duration) -> Self {
        Self {
            requests: DashMap::new(),
            limit,
            window,
        }
    }

    fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        
        // Clean up old entries
        self.requests.retain(|_ip, (_count, time)| {
            now.duration_since(*time) < self.window
        });

        // Check or increment
        let mut entry = self.requests.entry(ip).or_insert((0, now));
        if entry.0 >= self.limit {
            false
        } else {
            entry.0 += 1;
            true
        }
    }

    fn reset(&self, ip: IpAddr) {
        self.requests.remove(&ip);
    }
}

/// Enumeration pattern detector
struct EnumerationDetector {
    patterns: DashMap<IpAddr, Vec<Instant>>,
    threshold: usize,
    window: Duration,
}

impl EnumerationDetector {
    fn new(threshold: usize, window: Duration) -> Self {
        Self {
            patterns: DashMap::new(),
            threshold,
            window,
        }
    }

    fn record_lookup(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.patterns.entry(ip).or_insert_with(Vec::new);
        
        // Clean old entries
        entry.retain(|&time| now.duration_since(time) < self.window);
        
        // Add current lookup
        entry.push(now);
        
        // Check if threshold exceeded
        if entry.len() >= self.threshold {
            warn!("Enumeration attempt detected from {}", ip);
            true
        } else {
            false
        }
    }

    fn reset(&self, ip: IpAddr) {
        self.patterns.remove(&ip);
    }
}

/// IP blocklist
struct IpBlocklist {
    blocked: DashMap<IpAddr, Instant>,
    block_duration: Duration,
}

impl IpBlocklist {
    fn new(block_duration: Duration) -> Self {
        Self {
            blocked: DashMap::new(),
            block_duration,
        }
    }

    fn is_blocked(&self, ip: IpAddr) -> bool {
        if let Some(blocked_at_ref) = self.blocked.get(&ip) {
            let now = Instant::now();
            let blocked_at = *blocked_at_ref.value();
            if now.duration_since(blocked_at) < self.block_duration {
                return true;
            } else {
                // Block expired, remove it
                drop(blocked_at_ref);
                self.blocked.remove(&ip);
            }
        }
        false
    }

    fn block(&self, ip: IpAddr) {
        self.blocked.insert(ip, Instant::now());
    }
}

/// Search protection manager
pub struct SearchProtection {
    rate_limiter: Arc<RateLimiter>,
    enumeration_detector: Arc<EnumerationDetector>,
    blocklist: Arc<IpBlocklist>,
}

impl SearchProtection {
    pub fn new(rate_limit_per_minute: u32) -> Self {
        Self {
            rate_limiter: Arc::new(RateLimiter::new(
                rate_limit_per_minute,
                Duration::from_secs(60),
            )),
            enumeration_detector: Arc::new(EnumerationDetector::new(
                100, // 100 lookups in window = enumeration
                Duration::from_secs(300), // 5 minute window
            )),
            blocklist: Arc::new(IpBlocklist::new(
                Duration::from_secs(3600), // Block for 1 hour
            )),
        }
    }

    /// Check if lookup is allowed (rate limiting + enumeration detection)
    pub fn check_lookup(&self, ip: IpAddr) -> Result<(), ProtectionError> {
        // Check if IP is blocked
        if self.blocklist.is_blocked(ip) {
            return Err(ProtectionError::IpBlocked);
        }

        // Check rate limit
        if !self.rate_limiter.check(ip) {
            return Err(ProtectionError::RateLimited);
        }

        // Record lookup for enumeration detection
        if self.enumeration_detector.record_lookup(ip) {
            // Block IP temporarily
            self.blocklist.block(ip);
            return Err(ProtectionError::EnumerationDetected);
        }

        Ok(())
    }

    /// Reset rate limits for an IP (for testing/admin)
    pub fn reset(&self, ip: IpAddr) {
        self.rate_limiter.reset(ip);
        self.enumeration_detector.reset(ip);
    }
}
