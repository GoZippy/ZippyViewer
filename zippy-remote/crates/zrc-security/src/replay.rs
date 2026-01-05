//! Replay attack prevention.
//!
//! Requirements: 3.1, 3.2, 3.3, 3.4, 3.5

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::error::SecurityError;

/// Replay protection using a sliding window bitmap.
///
/// Tracks sequence numbers within a configurable window to detect
/// and reject duplicate or out-of-window packets.
///
/// **Thread Safety:** This struct is not thread-safe. For multi-threaded use,
/// wrap it in `Arc<Mutex<ReplayProtection>>` or use per-thread instances.
///
/// Requirements: 3.1, 3.4, 3.5
pub struct ReplayProtection {
    /// Sliding window bitmap (1024 bits = 16 * u64)
    window: [u64; 16],
    /// Start of the current window (aligned to 64-byte boundary)
    window_start: u64,
    /// Highest sequence number seen
    highest_seq: u64,
    /// Window size in packets
    window_size: u64,
}

impl ReplayProtection {
    /// Create a new replay protection with the specified window size.
    ///
    /// Window size is capped at 1024 packets.
    pub fn new(window_size: u64) -> Self {
        let window_size = window_size.min(1024);
        Self {
            window: [0u64; 16],
            window_start: 0,
            highest_seq: 0,
            window_size,
        }
    }

    /// Check if a sequence number is valid (not replayed) and update the filter.
    ///
    /// Requirements: 3.4, 3.5
    pub fn check_and_update(&mut self, seq: u64) -> Result<(), SecurityError> {
        if seq == 0 {
            return Err(SecurityError::InvalidSequence);
        }

        // Case 1: Sequence is before the window - reject as too old
        if seq < self.window_start() {
            return Err(SecurityError::ReplayDetected { sequence: seq });
        }

        // Case 2: Sequence is within the window - check if already seen
        let offset = seq - self.window_start();
        if offset < self.window_size {
            let bitmap_idx = (offset / 64) as usize;
            let bit_idx = offset % 64;
            let bit_mask = 1u64 << bit_idx;

            if self.window[bitmap_idx] & bit_mask != 0 {
                return Err(SecurityError::ReplayDetected { sequence: seq });
            }

            // Mark as seen
            self.window[bitmap_idx] |= bit_mask;
            return Ok(());
        }

        // Case 3: Sequence is ahead of the window - slide the window
        self.slide_window(seq);

        // Mark the new sequence as seen
        // After slide_window, seq is guaranteed to be in the first word (offset < 64)
        let offset = seq - self.window_start();
        let bitmap_idx = (offset / 64) as usize;
        let bit_idx = offset % 64;
        let bit_mask = 1u64 << bit_idx;
        self.window[bitmap_idx] |= bit_mask;

        Ok(())
    }

    /// Get the current window start position.
    fn window_start(&self) -> u64 {
        self.window_start
    }

    /// Slide the window forward to accommodate a new sequence number.
    fn slide_window(&mut self, new_seq: u64) {
        // Align window start to 64-byte boundary for efficient bitmap operations
        let new_window_start = new_seq - (new_seq % 64);
        let old_window_start = self.window_start();
        let shift_amount = new_window_start.saturating_sub(old_window_start);

        if shift_amount >= self.window_size {
            // Complete reset - new sequence is way ahead
            self.window = [0u64; 16];
            self.window_start = new_window_start;
        } else {
            // Shift the bitmap
            let words_to_shift = (shift_amount / 64) as usize;
            let bits_to_shift = (shift_amount % 64) as u32;

            if words_to_shift > 0 && words_to_shift < 16 {
                // Shift whole words
                for i in 0..(16 - words_to_shift) {
                    self.window[i] = self.window[i + words_to_shift];
                }
                for i in (16 - words_to_shift)..16 {
                    self.window[i] = 0;
                }
            }

            if bits_to_shift > 0 && words_to_shift < 16 {
                // Shift partial bits within words
                let mut carry = 0u64;
                for i in (0..16).rev() {
                    let new_carry = self.window[i] << (64 - bits_to_shift);
                    self.window[i] = (self.window[i] >> bits_to_shift) | carry;
                    carry = new_carry;
                }
            }
        }

        self.highest_seq = new_seq;
        self.window_start = new_window_start;
    }
}

impl Default for ReplayProtection {
    fn default() -> Self {
        Self::new(1024)
    }
}

/// Timestamp-based replay protection for session tickets.
///
/// Validates that tickets are not expired and not from the future.
///
/// Requirements: 3.2, 3.3
pub struct TicketValidator {
    /// Maximum age of a ticket
    max_age: Duration,
    /// Maximum clock skew tolerance (future timestamps)
    max_future: Duration,
}

impl TicketValidator {
    /// Create a new ticket validator.
    ///
    /// # Arguments
    /// * `max_age` - Maximum age of tickets (e.g., 1 hour)
    /// * `max_future` - Maximum clock skew tolerance (e.g., 5 minutes)
    pub fn new(max_age: Duration, max_future: Duration) -> Self {
        Self {
            max_age,
            max_future,
        }
    }

    /// Validate a ticket timestamp.
    ///
    /// Requirements: 3.2, 3.3
    pub fn validate_timestamp(&self, ticket_time: u64) -> Result<(), SecurityError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SecurityError::TicketFromFuture)? // System time before epoch is invalid
            .as_secs();

        if ticket_time > now + self.max_future.as_secs() {
            return Err(SecurityError::TicketFromFuture);
        }

        if now > ticket_time + self.max_age.as_secs() {
            return Err(SecurityError::TicketExpired);
        }

        Ok(())
    }
}

impl Default for TicketValidator {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(3600), // 1 hour
            Duration::from_secs(300),   // 5 minutes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_protection_accepts_sequential() {
        let mut rp = ReplayProtection::new(64);

        for i in 1..=100 {
            assert!(rp.check_and_update(i).is_ok(), "Failed at sequence {}", i);
        }
    }

    #[test]
    fn test_replay_protection_rejects_duplicate() {
        let mut rp = ReplayProtection::new(64);

        assert!(rp.check_and_update(42).is_ok());
        assert!(matches!(
            rp.check_and_update(42),
            Err(SecurityError::ReplayDetected { sequence: 42 })
        ));
    }

    #[test]
    fn test_replay_protection_handles_out_of_order() {
        let mut rp = ReplayProtection::new(64);

        // Receive packets out of order within window
        assert!(rp.check_and_update(5).is_ok());
        assert!(rp.check_and_update(3).is_ok());
        assert!(rp.check_and_update(7).is_ok());
        assert!(rp.check_and_update(4).is_ok());
        assert!(rp.check_and_update(6).is_ok());

        // All should be marked as seen now
        assert!(rp.check_and_update(5).is_err());
        assert!(rp.check_and_update(3).is_err());
    }

    #[test]
    fn test_replay_protection_rejects_zero() {
        let mut rp = ReplayProtection::new(64);
        assert!(matches!(
            rp.check_and_update(0),
            Err(SecurityError::InvalidSequence)
        ));
    }

    #[test]
    fn test_ticket_validator_accepts_valid_timestamp() {
        let validator = TicketValidator::default();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Ticket from 30 minutes ago should be valid
        assert!(validator.validate_timestamp(now - 1800).is_ok());
    }

    #[test]
    fn test_ticket_validator_rejects_expired() {
        let validator = TicketValidator::new(
            Duration::from_secs(3600), // 1 hour
            Duration::from_secs(300),  // 5 minutes
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Ticket from 2 hours ago should be expired
        assert!(matches!(
            validator.validate_timestamp(now - 7200),
            Err(SecurityError::TicketExpired)
        ));
    }

    #[test]
    fn test_ticket_validator_rejects_future() {
        let validator = TicketValidator::new(
            Duration::from_secs(3600), // 1 hour
            Duration::from_secs(300),  // 5 minutes
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Ticket from 10 minutes in future should be rejected
        assert!(matches!(
            validator.validate_timestamp(now + 600),
            Err(SecurityError::TicketFromFuture)
        ));
    }
}
