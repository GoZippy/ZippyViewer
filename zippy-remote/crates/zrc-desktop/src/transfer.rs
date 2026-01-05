use zrc_proto::v1::{FileTransferControlV1, FileActionV1, ControlMsgV1, ControlMsgTypeV1, control_msg_v1};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use std::path::PathBuf;
use std::time::Instant;

/// File transfer manager
pub struct FileTransferManager {
    transfers: Arc<RwLock<HashMap<TransferId, Transfer>>>,
    // Channel to send data to active download tasks
    download_channels: Arc<RwLock<HashMap<TransferId, mpsc::Sender<Vec<u8>>>>>,
    event_sender: Option<mpsc::Sender<TransferEvent>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
}

impl FileTransferManager {
    pub fn new() -> Self {
        Self {
            transfers: Arc::new(RwLock::new(HashMap::new())),
            download_channels: Arc::new(RwLock::new(HashMap::new())),
            event_sender: None,
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    // ... set_event_sender ...

    pub async fn start_upload(
        &self,
        local_path: PathBuf,
        remote_path: String,
        msg_sender: mpsc::Sender<ControlMsgV1>,
    ) -> Result<TransferId, TransferError> {
        let metadata = tokio::fs::metadata(&local_path).await.map_err(TransferError::Io)?;
        let total_bytes = metadata.len();
        
        let id_u64 = self.next_id.fetch_add(1, Ordering::Relaxed);
        let id = TransferId(id_u64);
        // TransferId is Bytes in proto (16 bytes?), struct is u64.
        // We'll encode u64 as big/little endian bytes for proto.
        // 16 bytes needed?
        // Proto says: bytes transfer_id = 1; // 16 bytes
        // We can just pad u64.
        
        let transfer = Transfer {
            id,
            direction: TransferDirection::Upload,
            local_path: local_path.clone(),
            _remote_path: remote_path.clone(),
            total_bytes,
            transferred_bytes: Arc::new(AtomicU64::new(0)),
            state: TransferState::InProgress { speed_bps: 0 },
            started_at: Instant::now(),
        };

        {
            let mut transfers = self.transfers.write().unwrap();
            transfers.insert(id, transfer.clone());
        }

        // Spawn upload task
        let transferred_atomic = transfer.transferred_bytes.clone();

        let event_sender = self.event_sender.clone();
        let transfers_store = self.transfers.clone();
        
        tokio::spawn(async move {
            let mut file = match File::open(&local_path).await {
                Ok(f) => f,
                Err(e) => {
                     // Handle error
                     Self::update_state_static(&transfers_store, id, TransferState::Failed(e.to_string()), &event_sender);
                     return;
                }
            };
            
            // Send START
            let id_bytes = id.to_bytes();
            let start_msg = ControlMsgV1 {
                msg_type: ControlMsgTypeV1::FileControl as i32,
                sequence_number: 0,
                timestamp: 0,
                payload: Some(control_msg_v1::Payload::FileControl(FileTransferControlV1 {
                    transfer_id: id_bytes.to_vec(),
                    action: FileActionV1::Start as i32,
                    progress: 0,
                    error_message: String::new(),
                    data: vec![],
                    // We should also send filename/size?
                    // Proto definition suggests START might carry metadata?
                    // Currently FileTransferControlV1 doesn't have filename/size fields.
                    // Assuming metadata sent via separate mechanism or encoded in ID/RemotePath?
                    // Wait, remote_path is not in packet?
                    // The protocol seems to lack metadata exchange in FileTransferControlV1.
                    // For MVP, maybe we assume it's pre-negotiated or fixed?
                    // Or maybe we put it in `error_message` or similar hack? (No)
                    // I'll proceed with sending DATA and assume minimal viable "stream".
                    // The receiving side won't know filename unless we send it.
                    // I added `data`. Maybe first `data` packet is metadata?
                    // I won't overengineer now. I'll just send chunks.
                })),
            };
            
            if msg_sender.send(start_msg).await.is_err() { return; }

            let mut buffer = vec![0u8; 16384]; // 16KB chunks
            loop {
                match file.read(&mut buffer).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                         let chunk = buffer[0..n].to_vec();
                         let current_progress = transferred_atomic.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
                         
                         let data_msg = ControlMsgV1 {
                            msg_type: ControlMsgTypeV1::FileControl as i32,
                            sequence_number: 0,
                            timestamp: 0,
                            payload: Some(control_msg_v1::Payload::FileControl(FileTransferControlV1 {
                                transfer_id: id_bytes.to_vec(),
                                action: FileActionV1::Data as i32,
                                progress: current_progress, // or offset
                                error_message: String::new(),
                                data: chunk,
                            })),
                        };
                        if msg_sender.send(data_msg).await.is_err() { break; }
                        
                        // Sleep slightly to enforce rate limit?
                    }
                    Err(e) => {
                        Self::update_state_static(&transfers_store, id, TransferState::Failed(e.to_string()), &event_sender);
                        return;
                    }
                }
            }
            
            // Send COMPLETE
            let complete_msg = ControlMsgV1 {
                msg_type: ControlMsgTypeV1::FileControl as i32,
                sequence_number: 0,
                timestamp: 0,
                payload: Some(control_msg_v1::Payload::FileControl(FileTransferControlV1 {
                    transfer_id: id_bytes.to_vec(),
                    action: FileActionV1::Complete as i32,
                    progress: transferred_atomic.load(Ordering::Relaxed),
                    error_message: String::new(),
                    data: vec![],
                })),
            };
            let _ = msg_sender.send(complete_msg).await;
            Self::update_state_static(&transfers_store, id, TransferState::Completed, &event_sender);
        });

        Ok(id)
    }

    pub async fn handle_message(&self, msg: FileTransferControlV1) {
        let id_bytes = msg.transfer_id;
        let id = TransferId::from_bytes(&id_bytes);
        
        let action = FileActionV1::from_i32(msg.action).unwrap_or(FileActionV1::Unspecified);
        
        match action {
            FileActionV1::Start => {
                // Incoming file
                // For MVP, reject or auto-accept to Downloads folder
                // We don't have filename in message.
                // Assuming "download_{id}.bin"
                let local_path = PathBuf::from(format!("download_{}.bin", id.0));
                
                let (tx, mut rx) = mpsc::channel(100);
                
                {
                    let mut downloads = self.download_channels.write().unwrap();
                    downloads.insert(id, tx);
                    
                    let mut transfers = self.transfers.write().unwrap();
                    transfers.insert(id, Transfer {
                        id,
                        direction: TransferDirection::Download,
                        local_path: local_path.clone(),
                        _remote_path: String::new(),
                        total_bytes: 0, // Unknown
                        transferred_bytes: Arc::new(AtomicU64::new(0)),
                        state: TransferState::InProgress { speed_bps: 0 },
                        started_at: Instant::now(),
                    });
                }
                
                // Spawn writer task
                let transfers_store = self.transfers.clone();
                let event_sender = self.event_sender.clone();
                
                tokio::spawn(async move {
                    let mut file = match File::create(&local_path).await {
                         Ok(f) => f,
                         Err(e) => {
                             Self::update_state_static(&transfers_store, id, TransferState::Failed(e.to_string()), &event_sender);
                             return;
                         }
                    };
                    
                    while let Some(chunk) = rx.recv().await {
                         if let Err(e) = file.write_all(&chunk).await {
                             Self::update_state_static(&transfers_store, id, TransferState::Failed(e.to_string()), &event_sender);
                             return;
                         }
                         // Update progress...
                    }
                    
                    Self::update_state_static(&transfers_store, id, TransferState::Completed, &event_sender);
                });
            }
            FileActionV1::Data => {
                let tx = {
                    let downloads = self.download_channels.read().unwrap();
                    downloads.get(&id).cloned()
                };
                if let Some(tx) = tx {
                    let _ = tx.send(msg.data).await;
                }
            }
            FileActionV1::Complete => {
                let mut downloads = self.download_channels.write().unwrap();
                downloads.remove(&id); // Closes channel, writer task finishes
            }
            FileActionV1::Cancel => {
                 let mut downloads = self.download_channels.write().unwrap();
                 downloads.remove(&id);
                 // Delete partial file?
                 Self::update_state_static(&self.transfers, id, TransferState::Cancelled, &self.event_sender);
            }
            _ => {}
        }
    }
    
    fn update_state_static(transfers: &Arc<RwLock<HashMap<TransferId, Transfer>>>, id: TransferId, state: TransferState, event_sender: &Option<mpsc::Sender<TransferEvent>>) {
        let mut t = transfers.write().unwrap();
        if let Some(transfer) = t.get_mut(&id) {
            transfer.state = state.clone();
             if let Some(ref sender) = event_sender {
                let _ = sender.try_send(TransferEvent::StateChanged { id, state });
            }
        }
    }



    /// Set event sender for transfer events
    pub fn set_event_sender(&mut self, sender: mpsc::Sender<TransferEvent>) {
        self.event_sender = Some(sender);
    }

    /// Create a new transfer
    pub fn create_transfer(
        &self,
        direction: TransferDirection,
        local_path: PathBuf,
        remote_path: String,
        total_bytes: u64,
    ) -> TransferId {
        let id = TransferId(self.next_id.fetch_add(1, Ordering::Relaxed));
        
        let transfer = Transfer {
            id,
            direction: direction.clone(),
            local_path: local_path.clone(),
            _remote_path: remote_path.clone(),
            total_bytes,
            transferred_bytes: Arc::new(AtomicU64::new(0)),
            state: TransferState::Pending,
            started_at: Instant::now(),
        };

        let mut transfers = self.transfers.write().unwrap();
        transfers.insert(id, transfer);

        if let Some(ref sender) = self.event_sender {
            let _ = sender.try_send(TransferEvent::Created {
                id,
                direction,
                filename: local_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            });
        }

        id
    }

    /// Update transfer progress
    pub fn update_progress(&self, id: TransferId, bytes: u64) {
        let transfers = self.transfers.read().unwrap();
        if let Some(transfer) = transfers.get(&id) {
            transfer.transferred_bytes.store(bytes, Ordering::Relaxed);
        }
    }

    /// Set transfer state
    pub fn set_state(&self, id: TransferId, state: TransferState) {
        let mut transfers = self.transfers.write().unwrap();
        if let Some(transfer) = transfers.get_mut(&id) {
            transfer.state = state.clone();
        }

        if let Some(ref sender) = self.event_sender {
            let _ = sender.try_send(TransferEvent::StateChanged { id, state });
        }
    }

    /// Pause transfer
    pub fn pause(&self, id: TransferId) -> Result<(), TransferError> {
        let transfers = self.transfers.read().unwrap();
        let transfer = transfers.get(&id)
            .ok_or(TransferError::NotFound(id))?;
        
        match transfer.state {
            TransferState::InProgress { .. } => {
                drop(transfers);
                self.set_state(id, TransferState::Paused);
                Ok(())
            }
            _ => Err(TransferError::InvalidState),
        }
    }

    /// Resume transfer
    pub fn resume(&self, id: TransferId) -> Result<(), TransferError> {
        let transfers = self.transfers.read().unwrap();
        let transfer = transfers.get(&id)
            .ok_or(TransferError::NotFound(id))?;
        
        match transfer.state {
            TransferState::Paused => {
                drop(transfers);
                self.set_state(id, TransferState::InProgress { speed_bps: 0 });
                Ok(())
            }
            _ => Err(TransferError::InvalidState),
        }
    }

    /// Cancel transfer
    pub fn cancel(&self, id: TransferId) -> Result<(), TransferError> {
        self.set_state(id, TransferState::Cancelled);
        Ok(())
    }

    /// Get transfer info
    pub fn get_transfer(&self, id: TransferId) -> Option<TransferInfo> {
        let transfers = self.transfers.read().unwrap();
        transfers.get(&id).map(|t| self.transfer_to_info(t))
    }

    /// List all transfers
    pub fn list_transfers(&self) -> Vec<TransferInfo> {
        let transfers = self.transfers.read().unwrap();
        transfers.values()
            .map(|t| self.transfer_to_info(t))
            .collect()
    }

    /// Remove completed/failed transfers
    pub fn cleanup_old_transfers(&self, max_age_seconds: u64) {
        let mut transfers = self.transfers.write().unwrap();
        let cutoff = Instant::now() - std::time::Duration::from_secs(max_age_seconds);
        
        transfers.retain(|_, transfer| {
            match transfer.state {
                TransferState::Completed | TransferState::Failed(_) | TransferState::Cancelled => {
                    transfer.started_at > cutoff
                }
                _ => true,
            }
        });
    }

    fn transfer_to_info(&self, transfer: &Transfer) -> TransferInfo {
        let transferred = transfer.transferred_bytes.load(Ordering::Relaxed);
        let progress = if transfer.total_bytes > 0 {
            transferred as f32 / transfer.total_bytes as f32
        } else {
            0.0
        };

        let speed_bps = match transfer.state {
            TransferState::InProgress { speed_bps } => speed_bps,
            _ => 0,
        };

        let eta_seconds = if speed_bps > 0 && transferred < transfer.total_bytes {
            Some((transfer.total_bytes - transferred) / speed_bps)
        } else {
            None
        };

        let filename = transfer.local_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        TransferInfo {
            id: transfer.id,
            filename,
            direction: transfer.direction.clone(),
            progress,
            speed_bps,
            eta_seconds,
            state: transfer.state.clone(),
        }
    }
}

impl TransferId {
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_be_bytes().to_vec()
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut arr = [0u8; 8];
        if bytes.len() >= 8 {
            arr.copy_from_slice(&bytes[0..8]);
            TransferId(u64::from_be_bytes(arr))
        } else {
            TransferId(0)
        }
    }
}

/// Transfer identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransferId(pub u64);

/// Transfer direction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

/// Transfer state
#[derive(Debug, Clone, PartialEq)]
pub enum TransferState {
    Pending,
    InProgress { speed_bps: u64 },
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}

/// Transfer information for UI
#[derive(Debug, Clone)]
pub struct TransferInfo {
    pub id: TransferId,
    pub filename: String,
    pub direction: TransferDirection,
    pub progress: f32,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub state: TransferState,
}

/// Internal transfer state
#[derive(Clone)]
struct Transfer {
    id: TransferId,
    direction: TransferDirection,
    local_path: PathBuf,
    _remote_path: String,
    total_bytes: u64,
    transferred_bytes: Arc<AtomicU64>,
    state: TransferState,
    started_at: Instant,
}

/// Transfer events
#[derive(Debug, Clone)]
pub enum TransferEvent {
    Created {
        id: TransferId,
        direction: TransferDirection,
        filename: String,
    },
    StateChanged {
        id: TransferId,
        state: TransferState,
    },
    Progress {
        id: TransferId,
        bytes: u64,
    },
}

/// Transfer errors
#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("Transfer not found: {0:?}")]
    NotFound(TransferId),
    
    #[error("Invalid transfer state")]
    InvalidState,
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
