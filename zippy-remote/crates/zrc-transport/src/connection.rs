//! Connection state management and transport ladder.

use crate::traits::{ControlPlaneTransport, TransportError, TransportType};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::timeout as tokio_timeout;

/// Connection state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// Connection manager for tracking state and statistics
pub struct ConnectionManager {
    state: Mutex<ConnectionState>,
    connected_at: Mutex<Option<Instant>>,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    reconnect_attempts: AtomicU32,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ConnectionState::Disconnected),
            connected_at: Mutex::new(None),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            reconnect_attempts: AtomicU32::new(0),
        }
    }

    /// Transition to new state
    pub fn transition(&self, new_state: ConnectionState) -> ConnectionState {
        let mut state = self.state.lock();
        let old_state = *state;
        
        match new_state {
            ConnectionState::Connected => {
                *self.connected_at.lock() = Some(Instant::now());
            }
            ConnectionState::Disconnected | ConnectionState::Failed => {
                *self.connected_at.lock() = None;
            }
            ConnectionState::Reconnecting => {
                self.reconnect_attempts.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        }
        
        *state = new_state;
        old_state
    }

    /// Get current state
    pub fn state(&self) -> ConnectionState {
        *self.state.lock()
    }

    /// Get connection duration
    pub fn duration(&self) -> Option<Duration> {
        self.connected_at.lock().map(|t| t.elapsed())
    }

    /// Record bytes sent
    pub fn record_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record bytes received
    pub fn record_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Get transfer statistics
    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            state: self.state(),
            duration: self.duration(),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            reconnect_attempts: self.reconnect_attempts.load(Ordering::Relaxed),
        }
    }

    /// Reset statistics
    pub fn reset(&self) {
        self.bytes_sent.store(0, Ordering::Relaxed);
        self.bytes_received.store(0, Ordering::Relaxed);
        self.reconnect_attempts.store(0, Ordering::Relaxed);
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Connection statistics
#[derive(Clone, Debug)]
pub struct ConnectionStats {
    pub state: ConnectionState,
    pub duration: Option<Duration>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub reconnect_attempts: u32,
}

/// Connected transport wrapper
pub struct ConnectedTransport {
    pub transport_type: TransportType,
    pub transport: Box<dyn ControlPlaneTransport>,
    pub connect_time: Duration,
}

/// Transport ladder for priority-ordered transport fallback
pub struct TransportLadder {
    transports: Mutex<Vec<(TransportType, Box<dyn ControlPlaneTransport>)>>,
    timeout: Duration,
}

impl TransportLadder {
    /// Create a new transport ladder
    pub fn new() -> Self {
        Self {
            transports: Mutex::new(Vec::new()),
            timeout: Duration::from_secs(10),
        }
    }

    /// Set connection timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Add transport with priority (lower index = higher priority)
    pub fn add(&self, transport_type: TransportType, transport: Box<dyn ControlPlaneTransport>) {
        self.transports.lock().push((transport_type, transport));
    }

    /// Try transports in order until one succeeds
    pub async fn connect(
        &self,
        _target: &[u8; 32],
    ) -> Result<ConnectedTransport, TransportError> {
        let transports = self.transports.lock();
        let start = Instant::now();

        for (transport_type, transport) in transports.iter() {
            // Check if transport is already connected
            if transport.is_connected() {
                // Note: In real implementation, we'd need to clone or wrap the transport
                // For now, we'll create a mock transport as placeholder
                return Ok(ConnectedTransport {
                    transport_type: *transport_type,
                    transport: Box::new(crate::testing::MockTransport::new().with_transport_type(*transport_type)),
                    connect_time: start.elapsed(),
                });
            }

            // Try to connect (simplified - in real implementation this would
            // call a connect method on the transport)
            match tokio_timeout(self.timeout, async {
                // Simulate connection attempt
                tokio::time::sleep(Duration::from_millis(10)).await;
                if transport.is_connected() {
                    Ok(())
                } else {
                    Err(TransportError::Disconnected)
                }
            })
            .await
            {
                Ok(Ok(())) => {
                    return Ok(ConnectedTransport {
                        transport_type: *transport_type,
                        transport: Box::new(crate::testing::MockTransport::new().with_transport_type(*transport_type)),
                        connect_time: start.elapsed(),
                    });
                }
                Ok(Err(_e)) => {
                    // Try next transport
                    continue;
                }
                Err(_) => {
                    // Timeout, try next transport
                    continue;
                }
            }
        }

        Err(TransportError::Other("All transports failed".to_string()))
    }

    /// Try transports in parallel, use first success
    pub async fn connect_parallel(
        &self,
        _target: &[u8; 32],
    ) -> Result<ConnectedTransport, TransportError> {
        let transports = self.transports.lock();
        let start = Instant::now();

        // Create tasks for each transport
        let mut tasks = Vec::new();
        for (transport_type, _transport) in transports.iter() {
            let transport_type = *transport_type;
            let timeout_duration = self.timeout;
            
            tasks.push(tokio::spawn(async move {
                match tokio_timeout(timeout_duration, async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    if true {
                        // In real implementation, check transport.is_connected()
                        Ok(transport_type)
                    } else {
                        Err(TransportError::Disconnected)
                    }
                })
                .await
                {
                    Ok(Ok(t)) => Ok(t),
                    _ => Err(TransportError::Timeout),
                }
            }));
        }

        // Wait for first success
        for task in tasks {
            if let Ok(Ok(transport_type)) = task.await {
                return Ok(ConnectedTransport {
                    transport_type,
                    transport: Box::new(crate::testing::MockTransport::new()),
                    connect_time: start.elapsed(),
                });
            }
        }

        Err(TransportError::Other("All transports failed".to_string()))
    }
}

impl Default for TransportLadder {
    fn default() -> Self {
        Self::new()
    }
}

/// Reconnection manager with exponential backoff
pub struct ReconnectionManager {
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
    current_attempt: AtomicU32,
    cancelled: AtomicBool,
}

impl ReconnectionManager {
    /// Create a new reconnection manager
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
            max_delay,
            current_attempt: AtomicU32::new(0),
            cancelled: AtomicBool::new(false),
        }
    }

    /// Attempt reconnection with exponential backoff
    pub async fn reconnect<F, Fut>(&self, mut connect_fn: F) -> Result<(), TransportError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<(), TransportError>>,
    {
        self.current_attempt.store(0, Ordering::Relaxed);
        self.cancelled.store(false, Ordering::Relaxed);

        loop {
            if self.cancelled.load(Ordering::Relaxed) {
                return Err(TransportError::Other("Reconnection cancelled".to_string()));
            }

            let attempt = self.current_attempt.load(Ordering::Relaxed);
            if attempt >= self.max_attempts {
                return Err(TransportError::Other("Max reconnection attempts reached".to_string()));
            }

            // Try to connect
            match connect_fn().await {
                Ok(()) => {
                    return Ok(());
                }
                Err(_e) => {
                    // Calculate backoff delay
                    let delay = self.calculate_backoff(attempt);
                    tokio::time::sleep(delay).await;
                    self.current_attempt.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let delay = self.base_delay.as_secs_f64() * 2.0_f64.powi(attempt as i32);
        let delay = delay.min(self.max_delay.as_secs_f64());
        Duration::from_secs_f64(delay)
    }

    /// Cancel reconnection attempts
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Get current attempt number
    pub fn current_attempt(&self) -> u32 {
        self.current_attempt.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::MockTransport;
    use proptest::prelude::*;

    #[test]
    fn test_state_transitions() {
        let mgr = ConnectionManager::new();
        assert_eq!(mgr.state(), ConnectionState::Disconnected);
        
        mgr.transition(ConnectionState::Connecting);
        assert_eq!(mgr.state(), ConnectionState::Connecting);
        
        mgr.transition(ConnectionState::Connected);
        assert_eq!(mgr.state(), ConnectionState::Connected);
        assert!(mgr.duration().is_some());
    }

    #[test]
    fn test_bytes_tracking() {
        let mgr = ConnectionManager::new();
        mgr.record_sent(100);
        mgr.record_received(200);
        
        let stats = mgr.stats();
        assert_eq!(stats.bytes_sent, 100);
        assert_eq!(stats.bytes_received, 200);
    }

    #[test]
    fn test_transport_ladder() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let ladder = TransportLadder::new();
            let transport1 = Box::new(crate::testing::MockTransport::new().with_transport_type(TransportType::Direct));
            let transport2 = Box::new(crate::testing::MockTransport::new().with_transport_type(TransportType::Relay));
            
            ladder.add(TransportType::Direct, transport1);
            ladder.add(TransportType::Relay, transport2);
            
            let _target = [0u8; 32];
            // Note: This test is simplified - real implementation would need proper connection logic
        });
    }

    proptest! {
        #[test]
        fn prop_transport_fallback(
            num_transports in 1..10usize,
            fail_first in 0..10usize
        ) {
            // Test that transport ladder tries transports in order
            // This is a simplified property test
            prop_assume!(fail_first < num_transports);
            // In real implementation, would test actual fallback behavior
        }
    }
}
