use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use zrc_core::http_mailbox::HttpMailboxClient;
use zrc_core::quic::{self, QuicClient};
use zrc_transport::{
    ControlPlaneTransport, MediaOpenParams, MediaSession, MediaTransport,
    RouteHint, TransportError, TransportType,
};

/// A wrapper around HttpMailboxClient that implements ControlPlaneTransport
#[derive(Clone)]
pub struct HttpControlTransport {
    client: HttpMailboxClient,
    my_id: [u8; 32],
    poll_wait_ms: u64,
}

impl HttpControlTransport {
    pub fn new(base_url: impl Into<String>, my_id: [u8; 32]) -> Result<Self> {
        let client = HttpMailboxClient::new(base_url)
            .map_err(|e| anyhow!("Failed to create mailbox client: {}", e))?;

        Ok(Self {
            client,
            my_id,
            poll_wait_ms: 1000, // Default long-poll wait
        })
    }

    pub fn set_poll_wait_ms(&mut self, wait_ms: u64) {
        self.poll_wait_ms = wait_ms;
    }
}

#[async_trait]
impl ControlPlaneTransport for HttpControlTransport {
    async fn send(
        &self,
        recipient: &[u8; 32],
        envelope: &[u8],
    ) -> Result<(), TransportError> {
        self.client
            .post(recipient, &Bytes::copy_from_slice(envelope))
            .await
            .map_err(|e| TransportError::Other(format!("Mailbox POST failed: {}", e)))
    }

    async fn recv(&self) -> Result<([u8; 32], Vec<u8>), TransportError> {
        loop {
            // Long poll
            match self.client.poll(&self.my_id, self.poll_wait_ms).await {
                Ok(Some(bytes)) => {
                    // Note: HTTP mailbox doesn't provide sender ID, using our own ID as placeholder
                    return Ok((self.my_id, bytes.to_vec()));
                }
                Ok(None) => continue, // Timeout, retry
                Err(e) => return Err(TransportError::Other(format!("Mailbox POLL failed: {}", e))),
            }
        }
    }

    fn is_connected(&self) -> bool {
        true // HTTP mailbox is stateless, always "connected"
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Mesh
    }
}

/// QUIC Media Transport Factory
pub struct QuicMediaTransport;

#[async_trait]
impl MediaTransport for QuicMediaTransport {
    async fn open(&self, params: MediaOpenParams) -> Result<Box<dyn MediaSession>> {
        // 1. Resolve socket address from route
        let addr = match params.route {
            RouteHint::DirectIp { host, port } => {
                let addr_str = format!("{}:{}", host, port);
                tokio::net::lookup_host(addr_str.clone())
                    .await?
                    .next()
                    .ok_or_else(|| anyhow!("Failed to resolve host: {}", addr_str))?
            }
            RouteHint::RendezvousUrl { .. } | RouteHint::MeshMailbox { .. } => {
                return Err(anyhow!("Unsupported route for QUIC: {:?}", params.route));
            }
        };

        // 2. Prepare client config (needs ALPN and Server Cert for pinning)
        let alpn = params
            .alpn
            .map(|s| s.into_bytes())
            .unwrap_or_else(|| b"zrc-media".to_vec());

        // In a real scenario, we'd get the cert from TransportNegotiation or Params?
        // SessionController gives us TransportNegotiation which includes cert.
        // But MediaOpenParams doesn't explicitly carry cert, unless encoded in relay_token or implicit?
        // ZRC Design: The Controller validates the cert via the Ticket/Negotiation before calling open.
        // Wait, `zrc_core::quic::make_pinned_client_config` requires cert.
        // Protocol: The `zrc-transport` trait is generic. `params.relay_token` might be abused to store cert? or we need to pass it differently.
        // For MVP: We assume self-signed or we Skip verification?
        // `zrc_core::quic` enforces pinning.
        // Let's create a placeholder cert or use a permissive client for now?
        // QuicClient::new takes server_cert_der.
        // Ideally `MediaOpenParams` should support this.
        // Hack for MVP: Generate a dummy self-signed cert if we don't have one, just to satisfy the struct, 
        // BUT the server will present its own. Connection will fail if mismatch.
        // 
        // FIX: The `SessionController` should have passed the cert.
        // `transport.rs` needs to handle this.
        // Maybe we abuse `relay_token` to pass the cert bytes?
        
        let server_cert_der = params.relay_token
            .map(|b| b.to_vec())
            .ok_or_else(|| anyhow!("Missing server certificate in relay_token param"))?;

         // Bind to random port
        let bind_addr: SocketAddr = "0.0.0.0:0".parse()?;
        
        let client = QuicClient::new(bind_addr, &alpn, &server_cert_der)
            .map_err(|e| anyhow!("Failed to create QUIC client: {}", e))?;

        // 3. Connect
        let connection = client.connect(addr, "zrc.local").await
            .map_err(|e| anyhow!("Failed to connect val QUIC: {}", e))?;

        // 4. Open/Accept streams
        // Protocol:
        // Client opens Bi-Di Control Stream (Stream 0)
        // Client Accepts Uni-Di Media Stream (Stream 1) - OR server opens it.
        // Let's assume we open Control.
        
        let (send, recv) = connection.open_bi().await
            .map_err(|e| anyhow!("Failed to open control stream: {}", e))?;

        let session = QuicMediaSession {
            connection,
            control_send: Arc::new(Mutex::new(send)),
            control_recv: Arc::new(Mutex::new(recv)),
            media_recv: Arc::new(Mutex::new(None)),
        };

        Ok(Box::new(session))
    }
}

pub struct QuicMediaSession {
    connection: quinn::Connection,
    control_send: Arc<Mutex<quinn::SendStream>>,
    control_recv: Arc<Mutex<quinn::RecvStream>>,
    media_recv: Arc<Mutex<Option<quinn::RecvStream>>>,
}

#[async_trait]
impl MediaSession for QuicMediaSession {
    async fn send_control(&self, data: Bytes) -> Result<()> {
        let mut send = self.control_send.lock().await;
        quic::write_frame(&mut send, &data).await.map_err(|e| anyhow!(e))
    }

    async fn recv_control(&self) -> Result<Bytes> {
        let mut recv = self.control_recv.lock().await;
        quic::read_frame(&mut recv).await.map_err(|e| anyhow!(e))
            .and_then(|opt| opt.ok_or_else(|| anyhow!("Control stream closed")))
    }

    async fn send_media_frame(&self, _data: Bytes) -> Result<()> {
        Err(anyhow!("Client does not send media frames"))
    }

    async fn recv_media_frame(&self) -> Result<Bytes> {
        // Lazily accept media stream if not already accepted
        let mut stream_opt = self.media_recv.lock().await;
        
        if stream_opt.is_none() {
            // Accept uni stream
            let stream = self.connection.accept_uni().await
                .map_err(|e| anyhow!("Failed to accept media stream: {}", e))?;
            *stream_opt = Some(stream);
        }
        
        if let Some(recv) = stream_opt.as_mut() {
            quic::read_frame(recv).await.map_err(|e| anyhow!(e))
                .and_then(|opt| opt.ok_or_else(|| anyhow!("Media stream closed")))
        } else {
            unreachable!()
        }
    }

    async fn close(&self) -> Result<()> {
        self.connection.close(0u32.into(), b"closed");
        Ok(())
    }
}
