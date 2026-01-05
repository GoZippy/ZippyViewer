use dashmap::DashMap;
use std::{
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RateLimitConfig {
    pub post_limit: u32,
    pub get_limit: u32,
    pub window_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            post_limit: 60,
            get_limit: 120,
            window_secs: 60,
        }
    }
}

#[derive(Debug)]
struct TokenBucket {
    tokens: u32,
    last_refill: Instant,
    limit: u32,
    window: Duration,
}

impl TokenBucket {
    fn new(limit: u32, window: Duration) -> Self {
        Self {
            tokens: limit,
            last_refill: Instant::now(),
            limit,
            window,
        }
    }

    fn check(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);

        // Refill tokens based on elapsed time
        if elapsed >= self.window {
            self.tokens = self.limit;
            self.last_refill = now;
        } else {
            // Proportional refill
            let refill = (self.limit as f64 * elapsed.as_secs_f64() / self.window.as_secs_f64()) as u32;
            self.tokens = (self.tokens + refill).min(self.limit);
            self.last_refill = now;
        }

        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }

    fn retry_after(&self) -> u64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        if elapsed >= self.window {
            0
        } else {
            (self.window.as_secs() - elapsed.as_secs()).max(1)
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<DashMap<IpAddr, Mutex<TokenBucket>>>,
    config: RateLimitConfig,
    allowlist: Arc<DashMap<IpAddr, ()>>,
    blocklist: Arc<DashMap<IpAddr, ()>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            config,
            allowlist: Arc::new(DashMap::new()),
            blocklist: Arc::new(DashMap::new()),
        }
    }

    pub async fn check_post(&self, ip: IpAddr) -> Result<(), u64> {
        if self.blocklist.contains_key(&ip) {
            return Err(0);
        }

        if self.allowlist.contains_key(&ip) {
            return Ok(());
        }

        let window = Duration::from_secs(self.config.window_secs);
        let bucket = self
            .buckets
            .entry(ip)
            .or_insert_with(|| Mutex::new(TokenBucket::new(self.config.post_limit, window)));

        let mut bucket = bucket.lock().await;
        if bucket.check() {
            Ok(())
        } else {
            Err(bucket.retry_after())
        }
    }

    pub async fn check_get(&self, ip: IpAddr) -> Result<(), u64> {
        if self.blocklist.contains_key(&ip) {
            return Err(0);
        }

        if self.allowlist.contains_key(&ip) {
            return Ok(());
        }

        let window = Duration::from_secs(self.config.window_secs);
        let bucket = self
            .buckets
            .entry(ip)
            .or_insert_with(|| Mutex::new(TokenBucket::new(self.config.get_limit, window)));

        let mut bucket = bucket.lock().await;
        if bucket.check() {
            Ok(())
        } else {
            Err(bucket.retry_after())
        }
    }

    pub fn add_to_allowlist(&self, ip: IpAddr) {
        self.allowlist.insert(ip, ());
    }

    pub fn add_to_blocklist(&self, ip: IpAddr) {
        self.blocklist.insert(ip, ());
    }

    pub fn remove_from_allowlist(&self, ip: IpAddr) {
        self.allowlist.remove(&ip);
    }

    pub fn remove_from_blocklist(&self, ip: IpAddr) {
        self.blocklist.remove(&ip);
    }
}
