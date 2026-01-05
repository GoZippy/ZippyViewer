//! Security controls for relay server

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use dashmap::DashMap;
use thiserror::Error;
use tracing::{warn, info};

/// Rate limiter for IP addresses
struct IpRateLimiter {
    requests: DashMap<IpAddr, (u32, Instant)>,
    limit: u32,
    window: Duration,
}

impl IpRateLimiter {
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

/// IP filter (allowlist/blocklist)
pub struct IpFilter {
    allowlist: DashMap<IpAddr, ()>,
    blocklist: DashMap<IpAddr, ()>,
    allowlist_enabled: Arc<Mutex<bool>>,
    blocklist_enabled: Arc<Mutex<bool>>,
}

impl IpFilter {
    pub fn new() -> Self {
        Self {
            allowlist: DashMap::new(),
            blocklist: DashMap::new(),
            allowlist_enabled: Arc::new(Mutex::new(false)),
            blocklist_enabled: Arc::new(Mutex::new(false)),
        }
    }

    pub fn allow(&self, ip: IpAddr) {
        self.allowlist.insert(ip, ());
        *self.allowlist_enabled.lock().unwrap() = true;
    }

    pub fn block(&self, ip: IpAddr) {
        self.blocklist.insert(ip, ());
        *self.blocklist_enabled.lock().unwrap() = true;
    }

    pub fn remove_allow(&self, ip: IpAddr) {
        self.allowlist.remove(&ip);
        if self.allowlist.is_empty() {
            *self.allowlist_enabled.lock().unwrap() = false;
        }
    }

    pub fn remove_block(&self, ip: IpAddr) {
        self.blocklist.remove(&ip);
        if self.blocklist.is_empty() {
            *self.blocklist_enabled.lock().unwrap() = false;
        }
    }

    pub fn is_allowed(&self, ip: IpAddr) -> bool {
        // Check blocklist first
        if *self.blocklist_enabled.lock().unwrap() && self.blocklist.contains_key(&ip) {
            return false;
        }

        // Check allowlist if enabled
        if *self.allowlist_enabled.lock().unwrap() {
            return self.allowlist.contains_key(&ip);
        }

        // Default: allow if not blocked
        true
    }
}

impl Default for IpFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Amplification attack detection
struct AmplificationDetector {
    ratios: DashMap<IpAddr, (u64, u64)>, // (sent, received)
    threshold: f64, // ratio threshold (sent/received)
}

impl AmplificationDetector {
    fn new(threshold: f64) -> Self {
        Self {
            ratios: DashMap::new(),
            threshold,
        }
    }

    fn record(&self, ip: IpAddr, sent: u64, received: u64) -> bool {
        let mut entry = self.ratios.entry(ip).or_insert((0, 0));
        entry.0 += sent;
        entry.1 += received;

        // Check if ratio exceeds threshold
        if entry.1 > 0 {
            let ratio = entry.0 as f64 / entry.1 as f64;
            if ratio > self.threshold {
                warn!(
                    ip = %ip,
                    ratio = ratio,
                    "Amplification attack detected"
                );
                return true;
            }
        }

        false
    }

    fn reset(&self, ip: IpAddr) {
        self.ratios.remove(&ip);
    }
}

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("IP blocked")]
    IpBlocked,
    #[error("IP not allowed")]
    IpNotAllowed,
    #[error("Amplification attack detected")]
    AmplificationAttack,
}

/// External abuse detection callback
pub type AbuseDetectionCallback = Box<dyn Fn(IpAddr, &str) -> bool + Send + Sync>;

/// Security controls manager
pub struct SecurityControls {
    allocation_rate_limiter: Arc<IpRateLimiter>,
    connection_rate_limiter: Arc<IpRateLimiter>,
    ip_filter: Arc<IpFilter>,
    amplification_detector: Arc<AmplificationDetector>,
    abuse_callback: Arc<Mutex<Option<Arc<AbuseDetectionCallback>>>>,
}

impl SecurityControls {
    pub fn new() -> Self {
        Self {
            allocation_rate_limiter: Arc::new(IpRateLimiter::new(
                10, // 10 per minute
                Duration::from_secs(60),
            )),
            connection_rate_limiter: Arc::new(IpRateLimiter::new(
                30, // 30 per minute
                Duration::from_secs(60),
            )),
            ip_filter: Arc::new(IpFilter::new()),
            amplification_detector: Arc::new(AmplificationDetector::new(10.0)), // 10:1 ratio
            abuse_callback: Arc::new(Mutex::new(None)),
        }
    }

    /// Set external abuse detection callback
    /// The callback receives (IP address, event_type) and returns true if abuse detected
    pub fn set_abuse_callback<F>(&self, callback: F)
    where
        F: Fn(IpAddr, &str) -> bool + Send + Sync + 'static,
    {
        *self.abuse_callback.lock().unwrap() = Some(Arc::new(Box::new(callback)));
    }

    /// Check with external abuse detection system
    pub fn check_external_abuse(&self, ip: IpAddr, event_type: &str) -> bool {
        if let Some(ref callback) = *self.abuse_callback.lock().unwrap() {
            callback(ip, event_type)
        } else {
            false
        }
    }

    /// Check allocation request rate limit
    pub fn check_allocation_rate_limit(&self, addr: SocketAddr) -> Result<(), SecurityError> {
        if !self.allocation_rate_limiter.check(addr.ip()) {
            warn!("Allocation rate limit exceeded for {}", addr.ip());
            return Err(SecurityError::RateLimitExceeded);
        }
        Ok(())
    }

    /// Check connection rate limit
    pub fn check_connection_rate_limit(&self, addr: SocketAddr) -> Result<(), SecurityError> {
        if !self.connection_rate_limiter.check(addr.ip()) {
            warn!("Connection rate limit exceeded for {}", addr.ip());
            // Check external abuse detection
            if self.check_external_abuse(addr.ip(), "connection_rate_limit") {
                warn!("External abuse detection confirmed abuse for {}", addr.ip());
            }
            return Err(SecurityError::RateLimitExceeded);
        }
        Ok(())
    }

    /// Check IP filter
    pub fn check_ip_filter(&self, addr: SocketAddr) -> Result<(), SecurityError> {
        if !self.ip_filter.is_allowed(addr.ip()) {
            let is_blocked = *self.ip_filter.blocklist_enabled.lock().unwrap() 
                && self.ip_filter.blocklist.contains_key(&addr.ip());
            if is_blocked {
                warn!("Blocked IP attempted connection: {}", addr.ip());
                return Err(SecurityError::IpBlocked);
            } else {
                warn!("IP not in allowlist: {}", addr.ip());
                return Err(SecurityError::IpNotAllowed);
            }
        }
        Ok(())
    }

    /// Check for amplification attack
    pub fn check_amplification(
        &self,
        addr: SocketAddr,
        sent_bytes: u64,
        received_bytes: u64,
    ) -> Result<(), SecurityError> {
        if self.amplification_detector.record(addr.ip(), sent_bytes, received_bytes) {
            return Err(SecurityError::AmplificationAttack);
        }
        Ok(())
    }

    /// Get IP filter (for admin API)
    /// Note: Admin API will need to use interior mutability (Mutex) for modifications
    pub fn ip_filter(&self) -> Arc<IpFilter> {
        self.ip_filter.clone()
    }

    /// Reset rate limits for an IP (for testing/admin)
    pub fn reset_rate_limits(&self, ip: IpAddr) {
        self.allocation_rate_limiter.reset(ip);
        self.connection_rate_limiter.reset(ip);
        self.amplification_detector.reset(ip);
    }
}

impl Default for SecurityControls {
    fn default() -> Self {
        Self::new()
    }
}
