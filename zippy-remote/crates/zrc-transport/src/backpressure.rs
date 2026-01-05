//! Backpressure handling for flow control.

use crate::mux::ChannelType;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::VecDeque;
use thiserror::Error;

/// Drop policy for handling buffer overflow
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropPolicy {
    /// Block until buffer available
    Block,
    /// Drop oldest frames
    DropOldest,
    /// Drop newest frames
    DropNewest,
    /// Drop based on priority (lower priority first)
    DropByPriority,
}

/// Backpressure error
#[derive(Debug, Error)]
pub enum BackpressureError {
    #[error("Buffer full and blocking policy")]
    BufferFull,

    #[error("Frame dropped due to backpressure")]
    FrameDropped,

    #[error("Other error: {0}")]
    Other(String),
}

/// Frame entry in buffer
struct FrameEntry {
    size: usize,
    channel: ChannelType,
}

/// Backpressure handler for managing send buffer limits
pub struct BackpressureHandler {
    send_buffer_limit: usize,
    current_buffer: Mutex<usize>,
    drop_policy: DropPolicy,
    dropped_count: AtomicU64,
    frame_queue: Mutex<VecDeque<FrameEntry>>,
}

impl BackpressureHandler {
    /// Create a new backpressure handler
    pub fn new(limit: usize, policy: DropPolicy) -> Self {
        Self {
            send_buffer_limit: limit,
            current_buffer: Mutex::new(0),
            drop_policy: policy,
            dropped_count: AtomicU64::new(0),
            frame_queue: Mutex::new(VecDeque::new()),
        }
    }

    /// Check if send is allowed
    pub fn can_send(&self, size: usize) -> bool {
        let current = *self.current_buffer.lock();
        current + size <= self.send_buffer_limit
    }

    /// Reserve buffer space
    pub async fn reserve(
        &self,
        size: usize,
        channel: ChannelType,
    ) -> Result<(), BackpressureError> {
        let mut current = self.current_buffer.lock();

        // Check if we have space
        if *current + size <= self.send_buffer_limit {
            *current += size;
            self.frame_queue.lock().push_back(FrameEntry { size, channel });
            return Ok(());
        }

        // Handle overflow based on policy
        match self.drop_policy {
            DropPolicy::Block => {
                return Err(BackpressureError::BufferFull);
            }
            DropPolicy::DropOldest => {
                // Remove oldest frames until we have space
                let mut queue = self.frame_queue.lock();
                while *current + size > self.send_buffer_limit {
                    if let Some(entry) = queue.pop_front() {
                        *current -= entry.size;
                        self.dropped_count.fetch_add(1, Ordering::Relaxed);
                    } else {
                        break;
                    }
                }
                if *current + size <= self.send_buffer_limit {
                    *current += size;
                    queue.push_back(FrameEntry { size, channel });
                    Ok(())
                } else {
                    Err(BackpressureError::FrameDropped)
                }
            }
            DropPolicy::DropNewest => {
                // Just drop this frame
                self.dropped_count.fetch_add(1, Ordering::Relaxed);
                Err(BackpressureError::FrameDropped)
            }
            DropPolicy::DropByPriority => {
                // Remove lowest priority frames first
                let mut queue = self.frame_queue.lock();
                let mut entries: Vec<_> = queue.drain(..).collect();
                
                // Sort by priority (higher priority = lower number)
                entries.sort_by_key(|e| e.channel.priority());
                
                // Remove from lowest priority until we have space
                while *current + size > self.send_buffer_limit {
                    if let Some(entry) = entries.pop() {
                        *current -= entry.size;
                        self.dropped_count.fetch_add(1, Ordering::Relaxed);
                    } else {
                        break;
                    }
                }
                
                // Rebuild queue
                for entry in entries {
                    queue.push_back(entry);
                }
                
                if *current + size <= self.send_buffer_limit {
                    *current += size;
                    queue.push_back(FrameEntry { size, channel });
                    Ok(())
                } else {
                    Err(BackpressureError::FrameDropped)
                }
            }
        }
    }

    /// Release buffer space
    pub fn release(&self, size: usize) {
        let mut current = self.current_buffer.lock();
        if *current >= size {
            *current -= size;
        } else {
            *current = 0;
        }
        
        // Remove from queue if present
        let mut queue = self.frame_queue.lock();
        if let Some(entry) = queue.front() {
            if entry.size == size {
                queue.pop_front();
            }
        }
    }

    /// Get dropped frame count
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }

    /// Get current buffer usage
    pub fn current_usage(&self) -> usize {
        *self.current_buffer.lock()
    }

    /// Get buffer limit
    pub fn limit(&self) -> usize {
        self.send_buffer_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_can_send() {
        let handler = BackpressureHandler::new(1000, DropPolicy::Block);
        assert!(handler.can_send(500));
        assert!(!handler.can_send(1500));
    }

    #[test]
    fn test_reserve_release() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let handler = BackpressureHandler::new(1000, DropPolicy::Block);
            handler.reserve(500, ChannelType::Control).await.unwrap();
            assert_eq!(handler.current_usage(), 500);
            handler.release(500);
            assert_eq!(handler.current_usage(), 0);
        });
    }

    #[test]
    fn test_drop_oldest() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let handler = BackpressureHandler::new(1000, DropPolicy::DropOldest);
            handler.reserve(600, ChannelType::Control).await.unwrap();
            handler.reserve(600, ChannelType::Frames).await.unwrap();
            // Should drop oldest to make room
            assert!(handler.dropped_count() > 0);
        });
    }

    #[test]
    fn prop_backpressure_enforcement() {
        use proptest::prelude::*;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        proptest!(|(limit in 1000..10000usize,
                     frame_sizes in prop::collection::vec(100..2000usize, 1..50),
                     policy in prop::sample::select(&[
                         DropPolicy::Block,
                         DropPolicy::DropOldest,
                         DropPolicy::DropNewest,
                         DropPolicy::DropByPriority,
                     ]))| {
            let handler = BackpressureHandler::new(limit, policy);
            let mut total_used = 0;
            
            for size in frame_sizes {
                match rt.block_on(handler.reserve(size, ChannelType::Frames)) {
                    Ok(()) => {
                        total_used += size;
                        if policy == DropPolicy::Block {
                            prop_assert!(total_used <= limit, "Block policy must not exceed limit");
                        }
                    }
                    Err(BackpressureError::BufferFull) => {
                        prop_assert_eq!(policy, DropPolicy::Block, "Only block policy returns BufferFull");
                    }
                    Err(BackpressureError::FrameDropped) => {
                        // Dropped frames are acceptable for non-block policies
                    }
                    _ => {}
                }
            }
            
            // After all operations, usage should not exceed limit
            prop_assert!(handler.current_usage() <= limit);
        });
    }
}
