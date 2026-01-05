//! Bandwidth limiting and quota management

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use dashmap::DashMap;
use crate::allocation::AllocationId;

/// Token bucket for rate limiting
struct TokenBucket {
    capacity: u64,
    tokens: AtomicU64,
    refill_rate: u64, // tokens per second
    last_refill: Arc<Mutex<Instant>>,
}

impl TokenBucket {
    fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            capacity,
            tokens: AtomicU64::new(capacity),
            refill_rate,
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    fn check_and_consume(&self, tokens: u64) -> bool {
        let now = Instant::now();
        let mut last_refill = self.last_refill.lock().unwrap();
        let elapsed = now.duration_since(*last_refill);
        
        // Refill tokens based on elapsed time
        if elapsed.as_secs() > 0 || elapsed.subsec_nanos() > 0 {
            let refill = (elapsed.as_secs() as u64 * self.refill_rate) +
                (elapsed.subsec_nanos() as u64 * self.refill_rate / 1_000_000_000);
            
            let current = self.tokens.load(Ordering::Relaxed);
            let new_tokens = (current + refill).min(self.capacity);
            self.tokens.store(new_tokens, Ordering::Relaxed);
            *last_refill = now;
        }

        // Try to consume tokens
        let current = self.tokens.load(Ordering::Relaxed);
        if current >= tokens {
            self.tokens.fetch_sub(tokens, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}

/// Token tier for limit configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenTier {
    Free,
    Paid,
    Unlimited,
}

/// Tiered limit configuration
#[derive(Debug, Clone)]
pub struct TieredLimits {
    pub free_bandwidth: u32,
    pub free_quota: u64,
    pub paid_bandwidth: u32,
    pub paid_quota: u64,
    pub unlimited_bandwidth: Option<u32>, // None means truly unlimited
    pub unlimited_quota: Option<u64>,
}

impl Default for TieredLimits {
    fn default() -> Self {
        Self {
            free_bandwidth: 5 * 1024 * 1024,      // 5 Mbps
            free_quota: 512 * 1024 * 1024,        // 512 MB
            paid_bandwidth: 50 * 1024 * 1024,     // 50 Mbps
            paid_quota: 10 * 1024 * 1024 * 1024,  // 10 GB
            unlimited_bandwidth: None,
            unlimited_quota: None,
        }
    }
}

/// Bandwidth limiter
pub struct BandwidthLimiter {
    buckets: DashMap<AllocationId, Arc<TokenBucket>>,
    global_bucket: Option<Arc<TokenBucket>>,
    tiered_limits: TieredLimits,
    allocation_tiers: DashMap<AllocationId, TokenTier>,
}

impl BandwidthLimiter {
    pub fn new(global_limit: Option<u64>) -> Self {
        let global_bucket = global_limit.map(|limit| {
            Arc::new(TokenBucket::new(limit, limit)) // Full capacity, refill at limit rate
        });

        Self {
            buckets: DashMap::new(),
            global_bucket,
            tiered_limits: TieredLimits::default(),
            allocation_tiers: DashMap::new(),
        }
    }

    /// Set tiered limits configuration
    pub fn set_tiered_limits(&mut self, limits: TieredLimits) {
        self.tiered_limits = limits;
    }

    /// Set tier for an allocation
    pub fn set_allocation_tier(&self, allocation_id: &AllocationId, tier: TokenTier) {
        self.allocation_tiers.insert(*allocation_id, tier);
    }

    /// Get bandwidth limit for a tier
    fn get_bandwidth_limit(&self, tier: TokenTier) -> Option<u32> {
        match tier {
            TokenTier::Free => Some(self.tiered_limits.free_bandwidth),
            TokenTier::Paid => Some(self.tiered_limits.paid_bandwidth),
            TokenTier::Unlimited => self.tiered_limits.unlimited_bandwidth,
        }
    }

    /// Check if transfer is allowed for an allocation
    pub fn check(&self, allocation_id: &AllocationId, bytes: usize, default_bandwidth_limit: u32) -> bool {
        // Check global limit first
        if let Some(ref global) = self.global_bucket {
            if !global.check_and_consume(bytes as u64) {
                return false;
            }
        }

        // Get tier-specific limit if available
        let bandwidth_limit = self.allocation_tiers
            .get(allocation_id)
            .and_then(|tier| self.get_bandwidth_limit(*tier.value()))
            .unwrap_or(default_bandwidth_limit);

        // Check per-allocation limit
        let bucket = self.buckets
            .entry(*allocation_id)
            .or_insert_with(|| {
                Arc::new(TokenBucket::new(
                    bandwidth_limit as u64,
                    bandwidth_limit as u64,
                ))
            })
            .clone();

        bucket.check_and_consume(bytes as u64)
    }

    /// Consume bandwidth tokens (called after successful transfer)
    pub fn consume(&self, allocation_id: &AllocationId, bytes: usize) {
        // Tokens already consumed in check()
        // This method exists for potential future use
        let _ = (allocation_id, bytes);
    }

    /// Remove allocation's bucket when allocation is terminated
    pub fn remove(&self, allocation_id: &AllocationId) {
        self.buckets.remove(allocation_id);
        self.allocation_tiers.remove(allocation_id);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::time::Duration;

    /// Property 2: Bandwidth Enforcement
    /// Validates: Requirements 4.1, 4.5
    /// 
    /// Property: Over any time period, the total bytes consumed cannot
    /// exceed (bandwidth_limit * time_elapsed + initial_capacity).
    #[test]
    fn prop_bandwidth_enforcement() {
        proptest!(|(
            capacity in 1000u64..=10_000_000u64,
            refill_rate in 1000u64..=100_000_000u64,
            num_requests in 1usize..=1000usize,
            request_sizes in prop::collection::vec(1u64..=100_000u64, 1..=1000),
        )| {
            let bucket = TokenBucket::new(capacity, refill_rate);
            let start = Instant::now();
            let mut total_consumed = 0u64;

            // Simulate requests over time
            for size in request_sizes.iter().take(num_requests) {
                if bucket.check_and_consume(*size) {
                    total_consumed += size;
                }
                // Small delay to allow refill
                std::thread::sleep(Duration::from_millis(1));
            }

            let elapsed = start.elapsed();
            let max_allowed = capacity + (refill_rate * elapsed.as_secs()) +
                (refill_rate * elapsed.subsec_nanos() as u64 / 1_000_000_000);

            // Total consumed should not exceed theoretical maximum
            prop_assert!(total_consumed <= max_allowed,
                "total_consumed={} max_allowed={}", total_consumed, max_allowed);
        });
    }
}
