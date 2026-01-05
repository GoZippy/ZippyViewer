//! Channel multiplexing support.

use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

/// Channel types for multiplexing
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelType {
    Control = 0,
    Frames = 1,
    Clipboard = 2,
    Files = 3,
    Audio = 4,
}

impl ChannelType {
    /// Get priority (lower = higher priority)
    pub fn priority(&self) -> u8 {
        match self {
            ChannelType::Control => 0,   // Highest
            ChannelType::Clipboard => 1,
            ChannelType::Files => 2,
            ChannelType::Audio => 3,
            ChannelType::Frames => 4,    // Lowest (can drop)
        }
    }

    /// Check if channel is lossy (can drop frames)
    pub fn is_lossy(&self) -> bool {
        matches!(self, ChannelType::Frames | ChannelType::Audio)
    }
}

/// Channel state
struct ChannelState {
    send_seq: AtomicU64,
    recv_seq: AtomicU64,
    send_buffer: Mutex<VecDeque<Vec<u8>>>,
    recv_buffer: Mutex<VecDeque<Vec<u8>>>,
    is_open: Mutex<bool>,
}

impl ChannelState {
    fn new() -> Self {
        Self {
            send_seq: AtomicU64::new(0),
            recv_seq: AtomicU64::new(0),
            send_buffer: Mutex::new(VecDeque::new()),
            recv_buffer: Mutex::new(VecDeque::new()),
            is_open: Mutex::new(true),
        }
    }

    fn next_send_seq(&self) -> u64 {
        self.send_seq.fetch_add(1, Ordering::SeqCst)
    }

    fn next_recv_seq(&self) -> u64 {
        self.recv_seq.fetch_add(1, Ordering::SeqCst)
    }
}

/// Multiplexer error
#[derive(Debug, Error)]
pub enum MuxError {
    #[error("Channel not open: {0:?}")]
    ChannelClosed(ChannelType),

    #[error("Channel already open: {0:?}")]
    ChannelAlreadyOpen(ChannelType),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Other error: {0}")]
    Other(String),
}

/// Channel encryption trait
#[async_trait]
pub trait ChannelEncryption: Send + Sync {
    async fn encrypt(
        &self,
        channel: ChannelType,
        seq: u64,
        data: &[u8],
    ) -> Result<Vec<u8>, MuxError>;

    async fn decrypt(
        &self,
        channel: ChannelType,
        seq: u64,
        data: &[u8],
    ) -> Result<Vec<u8>, MuxError>;
}

/// Multiplexer for routing data to correct channels
pub struct Multiplexer {
    channels: parking_lot::Mutex<HashMap<ChannelType, ChannelState>>,
    encryption: parking_lot::Mutex<Option<Box<dyn ChannelEncryption>>>,
}

impl Multiplexer {
    /// Create a new multiplexer
    pub fn new() -> Self {
        Self {
            channels: parking_lot::Mutex::new(HashMap::new()),
            encryption: parking_lot::Mutex::new(None),
        }
    }

    /// Open a channel
    pub fn open_channel(&self, channel: ChannelType) -> Result<(), MuxError> {
        let mut channels = self.channels.lock();
        if channels.contains_key(&channel) {
            return Err(MuxError::ChannelAlreadyOpen(channel));
        }
        channels.insert(channel, ChannelState::new());
        Ok(())
    }

    /// Close a channel
    pub fn close_channel(&self, channel: ChannelType) -> Result<(), MuxError> {
        let mut channels = self.channels.lock();
        if let Some(state) = channels.get(&channel) {
            *state.is_open.lock() = false;
        }
        channels.remove(&channel);
        Ok(())
    }

    /// Send on channel
    pub async fn send(&self, channel: ChannelType, data: &[u8]) -> Result<(), MuxError> {
        let channels = self.channels.lock();
        let state = channels
            .get(&channel)
            .ok_or_else(|| MuxError::ChannelClosed(channel))?;

        if !*state.is_open.lock() {
            return Err(MuxError::ChannelClosed(channel));
        }

        let seq = state.next_send_seq();
        let mut payload = data.to_vec();

        // Apply encryption if available
        if let Some(enc) = self.encryption.lock().as_ref() {
            payload = enc.encrypt(channel, seq, &payload).await?;
        }

        state.send_buffer.lock().push_back(payload);
        Ok(())
    }

    /// Receive from any channel
    pub async fn recv(&self) -> Result<(ChannelType, Vec<u8>), MuxError> {
        let channels = self.channels.lock();
        
        // Try channels in priority order
        let mut channel_list: Vec<_> = channels.keys().copied().collect();
        channel_list.sort_by_key(|c| c.priority());

        for channel in channel_list {
            if let Some(state) = channels.get(&channel) {
                let mut recv_buf = state.recv_buffer.lock();
                if let Some(data) = recv_buf.pop_front() {
                    let seq = state.next_recv_seq();
                    
                    // Apply decryption if available
                    let mut payload = data;
                    if let Some(enc) = self.encryption.lock().as_ref() {
                        payload = enc.decrypt(channel, seq, &payload).await?;
                    }
                    
                    return Ok((channel, payload));
                }
            }
        }

        Err(MuxError::Other("No data available".to_string()))
    }

    /// Set encryption for channels
    pub fn set_encryption(&self, encryption: Box<dyn ChannelEncryption>) {
        *self.encryption.lock() = Some(encryption);
    }

    /// Get send sequence number for a channel (for testing)
    pub fn send_seq(&self, channel: ChannelType) -> Option<u64> {
        let channels = self.channels.lock();
        channels.get(&channel).map(|s| s.send_seq.load(Ordering::SeqCst))
    }

    /// Get receive sequence number for a channel (for testing)
    pub fn recv_seq(&self, channel: ChannelType) -> Option<u64> {
        let channels = self.channels.lock();
        channels.get(&channel).map(|s| s.recv_seq.load(Ordering::SeqCst))
    }

    /// Inject data into receive buffer (for testing)
    pub fn inject_recv(&self, channel: ChannelType, data: Vec<u8>) -> Result<(), MuxError> {
        let channels = self.channels.lock();
        let state = channels
            .get(&channel)
            .ok_or_else(|| MuxError::ChannelClosed(channel))?;
        state.recv_buffer.lock().push_back(data);
        Ok(())
    }
}

impl Default for Multiplexer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_channel_open_close() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mux = Multiplexer::new();
            mux.open_channel(ChannelType::Control).unwrap();
            mux.close_channel(ChannelType::Control).unwrap();
            assert!(mux.send(ChannelType::Control, b"test").await.is_err());
        });
    }

    #[test]
    fn test_send_recv() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mux = Multiplexer::new();
            mux.open_channel(ChannelType::Control).unwrap();
            mux.send(ChannelType::Control, b"hello").await.unwrap();
            
            // Note: send puts data in send_buffer, not recv_buffer
            // In real implementation, this would be handled by the transport layer
            mux.inject_recv(ChannelType::Control, b"hello".to_vec()).unwrap();
            let (channel, data) = mux.recv().await.unwrap();
            assert_eq!(channel, ChannelType::Control);
            assert_eq!(data, b"hello");
        });
    }

    #[test]
    fn prop_sequence_monotonicity() {
        use proptest::prelude::*;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        proptest!(|(channel in prop::sample::select(&[
                ChannelType::Control,
                ChannelType::Frames,
                ChannelType::Clipboard,
                ChannelType::Files,
                ChannelType::Audio,
            ]),
            count in 1..100usize)| {
            let mux = Multiplexer::new();
            mux.open_channel(channel).unwrap();
            
            let mut last_seq = None;
            for _ in 0..count {
                rt.block_on(mux.send(channel, b"test")).unwrap();
                let seq = mux.send_seq(channel).unwrap();
                if let Some(last) = last_seq {
                    prop_assert!(seq > last, "Sequence must be strictly increasing");
                }
                last_seq = Some(seq);
            }
        });
    }
}
