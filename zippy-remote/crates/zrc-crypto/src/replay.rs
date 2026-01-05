//! Replay protection module.
//!
//! Provides replay attack prevention using sliding window bitmap and
//! monotonic counters for nonce/sequence tracking.

use std::sync::atomic::{AtomicU64, Ordering};

/// Error type for replay detection.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReplayError {
    #[error("duplicate packet detected: counter {counter} already seen")]
    DuplicatePacket { counter: u64 },
    #[error("counter {counter} is outside the replay window (window starts at {window_start})")]
    OutsideWindow { counter: u64, window_start: u64 },
}

/// A replay filter using a sliding window bitmap.
///
/// This structure tracks which packet sequence numbers (counters) have been
/// seen within a configurable window. It efficiently handles out-of-order
/// delivery while preventing replay attacks.
///
/// The default window size is 1024 packets, stored as a bitmap of 16 x u64.
#[derive(Debug)]
pub struct ReplayFilter {
    /// Start of the current window (minimum accepted counter)
    window_start: u64,
    /// Bitmap tracking seen packets within the window
    /// Each bit represents one counter value: bitmap[i] bit j = counter (window_start + i*64 + j)
    seen_bitmap: [u64; 16], // 1024 bits = 16 * 64
    /// Window size in packets
    window_size: usize,
}

impl Default for ReplayFilter {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl ReplayFilter {
    /// Create a new replay filter with the specified window size.
    ///
    /// # Arguments
    /// * `window_size` - Maximum number of packets to track (up to 1024)
    pub fn new(window_size: usize) -> Self {
        let window_size = window_size.min(1024); // Cap at 1024
        Self {
            window_start: 0,
            seen_bitmap: [0u64; 16],
            window_size,
        }
    }

    /// Check if a packet is valid (not a replay) and update the filter.
    ///
    /// # Arguments
    /// * `counter` - The packet's sequence number/counter
    ///
    /// # Returns
    /// * `Ok(())` if the packet is valid (not seen before and within window)
    /// * `Err(ReplayError)` if the packet is a replay or outside the window
    pub fn check_and_update(&mut self, counter: u64) -> Result<(), ReplayError> {
        // Case 1: Counter is before the window - reject as too old
        if counter < self.window_start {
            return Err(ReplayError::OutsideWindow {
                counter,
                window_start: self.window_start,
            });
        }

        // Case 2: Counter is within the window - check if already seen
        let offset = counter - self.window_start;
        if offset < self.window_size as u64 {
            let bitmap_idx = (offset / 64) as usize;
            let bit_idx = offset % 64;
            let bit_mask = 1u64 << bit_idx;

            if self.seen_bitmap[bitmap_idx] & bit_mask != 0 {
                return Err(ReplayError::DuplicatePacket { counter });
            }

            // Mark as seen
            self.seen_bitmap[bitmap_idx] |= bit_mask;
            return Ok(());
        }

        // Case 3: Counter is ahead of the window - slide the window
        self.slide_window(counter);

        // Mark the new counter as seen
        // After slide_window, counter is guaranteed to be in the first word (offset < 64)
        let bit_idx = counter % 64;
        self.seen_bitmap[0] |= 1 << bit_idx;

        Ok(())
    }

    /// Slide the window forward so that `new_counter` is within it.
    fn slide_window(&mut self, new_counter: u64) {
        let new_window_start = new_counter - (new_counter % 64);
        let shift_amount = new_window_start.saturating_sub(self.window_start);

        if shift_amount >= (self.window_size as u64) {
            // Complete reset - new counter is way ahead
            self.seen_bitmap = [0u64; 16];
        } else {
            // Shift the bitmap
            let words_to_shift = (shift_amount / 64) as usize;
            let bits_to_shift = (shift_amount % 64) as u32;

            if words_to_shift > 0 {
                // Shift whole words
                for i in 0..(16 - words_to_shift) {
                    self.seen_bitmap[i] = self.seen_bitmap[i + words_to_shift];
                }
                for i in (16 - words_to_shift)..16 {
                    self.seen_bitmap[i] = 0;
                }
            }

            if bits_to_shift > 0 && words_to_shift < 16 {
                // Shift partial bits within words
                let mut carry = 0u64;
                for i in (0..16).rev() {
                    let new_carry = self.seen_bitmap[i] << (64 - bits_to_shift);
                    self.seen_bitmap[i] = (self.seen_bitmap[i] >> bits_to_shift) | carry;
                    carry = new_carry;
                }
            }
        }

        self.window_start = new_window_start;
    }

    /// Get the current window start position.
    pub fn window_start(&self) -> u64 {
        self.window_start
    }
}

/// A monotonic counter for generating unique sequence numbers.
///
/// Thread-safe and lock-free.
#[derive(Debug)]
pub struct MonotonicCounter {
    value: AtomicU64,
}

impl MonotonicCounter {
    /// Create a new counter starting at the given value.
    pub fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
        }
    }

    /// Increment the counter and return the new value.
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Get the current counter value without incrementing.
    pub fn current(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }
}

impl Default for MonotonicCounter {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Generate a 12-byte nonce from stream_id and counter.
///
/// The nonce format is: stream_id (4 bytes, big-endian) || counter (8 bytes, big-endian)
///
/// This ensures unique nonces for each (stream, counter) pair.
pub fn generate_nonce(stream_id: u32, counter: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[0..4].copy_from_slice(&stream_id.to_be_bytes());
    nonce[4..12].copy_from_slice(&counter.to_be_bytes());
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_filter_accepts_sequential() {
        let mut filter = ReplayFilter::default();

        for i in 0..100 {
            assert!(filter.check_and_update(i).is_ok(), "Failed at counter {}", i);
        }
    }

    #[test]
    fn test_replay_filter_rejects_duplicate() {
        let mut filter = ReplayFilter::default();

        assert!(filter.check_and_update(42).is_ok());
        assert!(matches!(
            filter.check_and_update(42),
            Err(ReplayError::DuplicatePacket { counter: 42 })
        ));
    }

    #[test]
    fn test_replay_filter_handles_out_of_order() {
        let mut filter = ReplayFilter::default();

        // Receive packets out of order within window
        assert!(filter.check_and_update(5).is_ok());
        assert!(filter.check_and_update(3).is_ok());
        assert!(filter.check_and_update(7).is_ok());
        assert!(filter.check_and_update(4).is_ok());
        assert!(filter.check_and_update(6).is_ok());

        // All should be marked as seen now
        assert!(filter.check_and_update(5).is_err());
        assert!(filter.check_and_update(3).is_err());
    }

    #[test]
    fn test_replay_filter_window_slide() {
        let mut filter = ReplayFilter::new(64); // Small window for testing

        // Fill up some of the window
        for i in 0..10 {
            assert!(filter.check_and_update(i).is_ok());
        }

        // Jump ahead - should slide the window
        assert!(filter.check_and_update(100).is_ok());

        // Old packets should now be rejected as outside window
        assert!(matches!(
            filter.check_and_update(5),
            Err(ReplayError::OutsideWindow { .. })
        ));
    }

    #[test]
    fn test_replay_filter_rejects_old_packets() {
        let mut filter = ReplayFilter::new(100);

        // Advance the window
        assert!(filter.check_and_update(1000).is_ok());

        // Very old packet should be rejected
        assert!(matches!(
            filter.check_and_update(10),
            Err(ReplayError::OutsideWindow { .. })
        ));
    }

    #[test]
    fn test_monotonic_counter_increment() {
        let counter = MonotonicCounter::new(0);

        assert_eq!(counter.current(), 0);
        assert_eq!(counter.increment(), 1);
        assert_eq!(counter.increment(), 2);
        assert_eq!(counter.increment(), 3);
        assert_eq!(counter.current(), 3);
    }

    #[test]
    fn test_monotonic_counter_thread_safety() {
        use std::thread;

        let counter = std::sync::Arc::new(MonotonicCounter::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let counter_clone = counter.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    counter_clone.increment();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(counter.current(), 1000);
    }

    #[test]
    fn test_generate_nonce_uniqueness() {
        // Different stream_ids should produce different nonces
        let nonce1 = generate_nonce(1, 100);
        let nonce2 = generate_nonce(2, 100);
        assert_ne!(nonce1, nonce2);

        // Different counters should produce different nonces
        let nonce3 = generate_nonce(1, 101);
        assert_ne!(nonce1, nonce3);

        // Same inputs should produce same nonce
        let nonce4 = generate_nonce(1, 100);
        assert_eq!(nonce1, nonce4);
    }

    #[test]
    fn test_generate_nonce_format() {
        let nonce = generate_nonce(0x01020304, 0x0506070809101112);

        // First 4 bytes should be stream_id (big-endian)
        assert_eq!(&nonce[0..4], &[0x01, 0x02, 0x03, 0x04]);

        // Next 8 bytes should be counter (big-endian)
        assert_eq!(&nonce[4..12], &[0x05, 0x06, 0x07, 0x08, 0x09, 0x10, 0x11, 0x12]);
    }
}
