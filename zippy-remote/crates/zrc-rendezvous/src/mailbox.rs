use bytes::Bytes;
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Notify;

#[derive(Debug, Clone)]
pub struct Message {
    pub data: Bytes,
    pub sequence: u64,
    pub timestamp: Instant,
}

#[derive(Debug)]
pub struct Mailbox {
    pub messages: VecDeque<Message>,
    pub next_sequence: u64,
    pub last_activity: Instant,
    pub notify: Arc<Notify>,
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Mailbox {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            next_sequence: 1,
            last_activity: Instant::now(),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn post(&mut self, data: Bytes, max_queue_len: usize, max_message_size: usize) -> Result<u64, MailboxError> {
        if data.len() > max_message_size {
            return Err(MailboxError::MessageTooLarge);
        }

        if self.messages.len() >= max_queue_len {
            return Err(MailboxError::QueueFull);
        }

        let sequence = self.next_sequence;
        self.next_sequence += 1;

        let message = Message {
            data,
            sequence,
            timestamp: Instant::now(),
        };

        self.messages.push_back(message);
        self.last_activity = Instant::now();
        self.notify.notify_waiters();

        Ok(sequence)
    }

    pub fn get(&mut self) -> Option<Message> {
        self.last_activity = Instant::now();
        self.messages.pop_front()
    }

    pub fn queue_length(&self) -> usize {
        self.messages.len()
    }

    pub fn evict_expired(&mut self, ttl: Duration) -> usize {
        let now = Instant::now();
        let mut evicted = 0;

        while let Some(front) = self.messages.front() {
            if now.duration_since(front.timestamp) > ttl {
                self.messages.pop_front();
                evicted += 1;
            } else {
                break;
            }
        }

        evicted
    }

    pub fn is_idle(&self, idle_timeout: Duration) -> bool {
        self.messages.is_empty() && Instant::now().duration_since(self.last_activity) > idle_timeout
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MailboxError {
    #[error("message too large")]
    MessageTooLarge,
    #[error("queue full")]
    QueueFull,
}

pub type MailboxMap = Arc<dashmap::DashMap<Vec<u8>, Mailbox>>;
