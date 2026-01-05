//! Testing utilities for transport implementations.

use crate::traits::{ControlPlaneTransport, TransportError, TransportType};
use async_trait::async_trait;
use parking_lot::Mutex;
use rand::Rng;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Mock transport for testing
pub struct MockTransport {
    sent: Mutex<Vec<(/* recipient */ [u8; 32], /* data */ Vec<u8>)>>,
    recv_queue: Mutex<VecDeque<(/* sender */ [u8; 32], /* data */ Vec<u8>)>>,
    connected: AtomicBool,
    latency: Duration,
    packet_loss: f64,
    transport_type: TransportType,
}

impl MockTransport {
    /// Create a new mock transport
    pub fn new() -> Self {
        Self {
            sent: Mutex::new(Vec::new()),
            recv_queue: Mutex::new(VecDeque::new()),
            connected: AtomicBool::new(true),
            latency: Duration::ZERO,
            packet_loss: 0.0,
            transport_type: TransportType::Direct,
        }
    }

    /// Configure simulated latency
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// Configure simulated packet loss (0.0 - 1.0)
    pub fn with_packet_loss(mut self, loss: f64) -> Self {
        self.packet_loss = loss.clamp(0.0, 1.0);
        self
    }

    /// Set transport type
    pub fn with_transport_type(mut self, transport_type: TransportType) -> Self {
        self.transport_type = transport_type;
        self
    }

    /// Inject message to receive queue
    pub fn inject_recv(&self, sender: [u8; 32], data: Vec<u8>) {
        self.recv_queue.lock().push_back((sender, data));
    }

    /// Get sent messages
    pub fn get_sent(&self) -> Vec<([u8; 32], Vec<u8>)> {
        self.sent.lock().clone()
    }

    /// Clear sent messages
    pub fn clear_sent(&self) {
        self.sent.lock().clear();
    }

    /// Simulate disconnect
    pub fn disconnect(&self) {
        self.connected.store(false, Ordering::Relaxed);
    }

    /// Simulate connect
    pub fn connect(&self) {
        self.connected.store(true, Ordering::Relaxed);
    }
}

#[async_trait]
impl ControlPlaneTransport for MockTransport {
    async fn send(
        &self,
        recipient: &[u8; 32],
        envelope: &[u8],
    ) -> Result<(), TransportError> {
        if !self.is_connected() {
            return Err(TransportError::Disconnected);
        }

        // Simulate packet loss
        let should_drop = {
            let mut rng = rand::thread_rng();
            rng.gen::<f64>() < self.packet_loss
        };
        if should_drop {
            return Err(TransportError::Other("Packet lost".to_string()));
        }

        // Simulate latency
        if !self.latency.is_zero() {
            sleep(self.latency).await;
        }

        self.sent.lock().push((*recipient, envelope.to_vec()));
        Ok(())
    }

    async fn recv(&self) -> Result<([u8; 32], Vec<u8>), TransportError> {
        if !self.is_connected() {
            return Err(TransportError::Disconnected);
        }

        // Simulate latency
        if !self.latency.is_zero() {
            sleep(self.latency).await;
        }

        loop {
            if let Some((sender, data)) = self.recv_queue.lock().pop_front() {
                return Ok((sender, data));
            }
            // Wait a bit before checking again
            sleep(Duration::from_millis(10)).await;
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn transport_type(&self) -> TransportType {
        self.transport_type
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

/// Loopback transport for local testing
pub struct LoopbackTransport {
    local_id: [u8; 32],
    peer: Arc<LoopbackPeer>,
}

struct LoopbackPeer {
    #[allow(dead_code)]
    id: [u8; 32],
    recv_queue: Mutex<VecDeque<([u8; 32], Vec<u8>)>>,
    connected: AtomicBool,
}

impl LoopbackTransport {
    /// Create connected pair
    pub fn pair() -> (Self, Self) {
        let id1 = [1u8; 32];
        let id2 = [2u8; 32];

        let peer1 = Arc::new(LoopbackPeer {
            id: id1,
            recv_queue: Mutex::new(VecDeque::new()),
            connected: AtomicBool::new(true),
        });

        let peer2 = Arc::new(LoopbackPeer {
            id: id2,
            recv_queue: Mutex::new(VecDeque::new()),
            connected: AtomicBool::new(true),
        });

        // Cross-link peers
        let transport1 = Self {
            local_id: id1,
            peer: peer2.clone(),
        };

        let transport2 = Self {
            local_id: id2,
            peer: peer1.clone(),
        };

        (transport1, transport2)
    }
}

#[async_trait]
impl ControlPlaneTransport for LoopbackTransport {
    async fn send(
        &self,
        _recipient: &[u8; 32],
        envelope: &[u8],
    ) -> Result<(), TransportError> {
        if !self.peer.connected.load(Ordering::Relaxed) {
            return Err(TransportError::Disconnected);
        }

        self.peer
            .recv_queue
            .lock()
            .push_back((self.local_id, envelope.to_vec()));
        Ok(())
    }

    async fn recv(&self) -> Result<([u8; 32], Vec<u8>), TransportError> {
        if !self.peer.connected.load(Ordering::Relaxed) {
            return Err(TransportError::Disconnected);
        }

        loop {
            if let Some((sender, data)) = self.peer.recv_queue.lock().pop_front() {
                return Ok((sender, data));
            }
            sleep(Duration::from_millis(10)).await;
        }
    }

    fn is_connected(&self) -> bool {
        self.peer.connected.load(Ordering::Relaxed)
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Direct
    }
}

/// Transport recording for deterministic testing
pub struct TransportRecorder {
    events: Mutex<Vec<TransportEvent>>,
}

#[derive(Clone, Debug)]
pub enum TransportEvent {
    Send { recipient: [u8; 32], data: Vec<u8> },
    Recv { sender: [u8; 32], data: Vec<u8> },
    Disconnect,
    Connect,
}

impl TransportRecorder {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn record(&self, event: TransportEvent) {
        self.events.lock().push(event);
    }

    pub fn get_events(&self) -> Vec<TransportEvent> {
        self.events.lock().clone()
    }

    pub fn clear(&self) {
        self.events.lock().clear();
    }
}

impl Default for TransportRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_transport() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let transport = MockTransport::new();
            let recipient = [1u8; 32];
            transport.send(&recipient, b"hello").await.unwrap();

            let sent = transport.get_sent();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0].1, b"hello");
        });
    }

    #[test]
    fn test_loopback_transport() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let (transport1, transport2) = LoopbackTransport::pair();
            let recipient = [2u8; 32];
            
            transport1.send(&recipient, b"hello").await.unwrap();
            let (_sender, data) = transport2.recv().await.unwrap();
            assert_eq!(data, b"hello");
        });
    }
}
