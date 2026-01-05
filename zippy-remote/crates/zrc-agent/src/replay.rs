//! Replay protection module for WebRTC media streams.
//!
//! Implements deterministic nonce generation and sliding window replay filter
//! to prevent packet replay attacks.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use zrc_crypto::replay::{ReplayFilter, ReplayError};
use tracing::{debug, warn};

/// Stream ID type (32-bit)
pub type StreamId = u32;

/// Counter type (64-bit)
pub type Counter = u64;

/// Deterministic nonce: stream_id (32-bit) || counter (64-bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Nonce {
    pub stream_id: StreamId,
    pub counter: Counter,
}

impl Nonce {
    pub fn new(stream_id: StreamId, counter: Counter) -> Self {
        Self { stream_id, counter }
    }

    pub fn to_bytes(&self) -> [u8; 12] {
        let mut bytes = [0u8; 12];
        bytes[0..4].copy_from_slice(&self.stream_id.to_be_bytes());
        bytes[4..12].copy_from_slice(&self.counter.to_be_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 12 {
            return None;
        }
        let stream_id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let counter = u64::from_be_bytes([
            bytes[4], bytes[5], bytes[6], bytes[7],
            bytes[8], bytes[9], bytes[10], bytes[11],
        ]);
        Some(Self { stream_id, counter })
    }
}

/// Per-stream counter tracking
pub struct StreamCounter {
    stream_id: StreamId,
    counter: Counter,
}

impl StreamCounter {
    pub fn new(stream_id: StreamId) -> Self {
        Self {
            stream_id,
            counter: 0,
        }
    }

    pub fn next(&mut self) -> Nonce {
        let nonce = Nonce::new(self.stream_id, self.counter);
        self.counter += 1;
        nonce
    }

    pub fn current(&self) -> Counter {
        self.counter
    }
}

/// Replay protection manager for multiple streams
pub struct ReplayProtection {
    counters: Arc<RwLock<HashMap<StreamId, StreamCounter>>>,
    filters: Arc<RwLock<HashMap<StreamId, ReplayFilter>>>,
    window_size: usize,
}

impl ReplayProtection {
    pub fn new(window_size: usize) -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            filters: Arc::new(RwLock::new(HashMap::new())),
            window_size,
        }
    }

    /// Generate next nonce for a stream
    pub async fn next_nonce(&self, stream_id: StreamId) -> Nonce {
        let mut counters = self.counters.write().await;
        let counter = counters
            .entry(stream_id)
            .or_insert_with(|| StreamCounter::new(stream_id));
        counter.next()
    }

    /// Check if a nonce is valid (not a replay)
    pub async fn check_nonce(&self, nonce: Nonce) -> Result<(), ReplayError> {
        let mut filters = self.filters.write().await;
        let filter = filters
            .entry(nonce.stream_id)
            .or_insert_with(|| ReplayFilter::new(self.window_size));
        
        filter.check_and_update(nonce.counter)
    }

    /// Remove a stream's replay protection state
    pub async fn remove_stream(&self, stream_id: StreamId) {
        let mut counters = self.counters.write().await;
        let mut filters = self.filters.write().await;
        counters.remove(&stream_id);
        filters.remove(&stream_id);
        debug!("Removed replay protection for stream {}", stream_id);
    }

    /// Get current counter for a stream
    pub async fn current_counter(&self, stream_id: StreamId) -> Option<Counter> {
        let counters = self.counters.read().await;
        counters.get(&stream_id).map(|c| c.current())
    }
}
