#![cfg(feature = "quic")]

use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use quinn::{ClientConfig, Endpoint, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::AsyncWriteExt;

// Re-export for convenience
pub use quinn::Connection;

#[derive(Debug, thiserror::Error)]
pub enum QuicError {
    #[error("tls/cert error: {0}")]
    Tls(String),
    #[error("quic error: {0}")]
    Quic(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("bad input: {0}")]
    Bad(String),
}

pub struct QuicServer {
    pub endpoint: Arc<Endpoint>,
    pub cert_der: Vec<u8>,
    pub alpn: Vec<u8>,
}

pub struct QuicClient {
    pub endpoint: Endpoint,
    pub alpn: Vec<u8>,
}

pub fn make_self_signed_server_config(alpn: &[u8]) -> Result<(ServerConfig, Vec<u8>), QuicError> {
    let certified_key = rcgen::generate_simple_self_signed(vec!["zrc.local".into()])
        .map_err(|e| QuicError::Tls(e.to_string()))?;

    let cert_der = certified_key.cert.der().to_vec();
    let key_der = certified_key.key_pair.serialize_der();

    let cert_chain = vec![CertificateDer::from(cert_der.clone())];
    let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(key_der));

    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .map_err(|e| QuicError::Tls(e.to_string()))?;

    tls.alpn_protocols = vec![alpn.to_vec()];

    let server_cfg = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(tls)
            .map_err(|e| QuicError::Tls(e.to_string()))?
    ));
    Ok((server_cfg, cert_der))
}

pub fn make_pinned_client_config(server_cert_der: &[u8], alpn: &[u8]) -> Result<ClientConfig, QuicError> {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from(server_cert_der.to_vec()))
        .map_err(|e| QuicError::Tls(e.to_string()))?;

    let mut tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    tls.alpn_protocols = vec![alpn.to_vec()];

    Ok(ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls)
            .map_err(|e| QuicError::Tls(e.to_string()))?
    )))
}

impl QuicServer {
    pub async fn bind(addr: SocketAddr, alpn: &[u8]) -> Result<Self, QuicError> {
        let (server_cfg, cert_der) = make_self_signed_server_config(alpn)?;
        let endpoint = Endpoint::server(server_cfg, addr).map_err(|e| QuicError::Quic(e.to_string()))?;
        Ok(Self {
            endpoint: Arc::new(endpoint),
            cert_der,
            alpn: alpn.to_vec(),
        })
    }

    /// Minimal echo loop: accept connections and echo frames on first bi-stream.
    pub async fn run_echo_loop(self) -> Result<(), QuicError> {
        let endpoint = self.endpoint;
        loop {
            let incoming = endpoint.accept().await;
            let Some(connecting) = incoming else { break };
            tokio::spawn(async move {
                if let Ok(conn) = connecting.await {
                    if let Ok((mut send, mut recv)) = conn.accept_bi().await {
                        while let Ok(Some(frame)) = read_frame(&mut recv).await {
                            let _ = write_frame(&mut send, &frame).await;
                        }
                        let _ = send.finish();
                    }
                }
            });
        }
        Ok(())
    }
}

impl QuicClient {
    pub fn new(bind_addr: SocketAddr, alpn: &[u8], server_cert_der: &[u8]) -> Result<Self, QuicError> {
        let mut endpoint = Endpoint::client(bind_addr).map_err(|e| QuicError::Quic(e.to_string()))?;
        let cfg = make_pinned_client_config(server_cert_der, alpn)?;
        endpoint.set_default_client_config(cfg);
        Ok(Self { endpoint, alpn: alpn.to_vec() })
    }

    pub async fn connect(&self, remote: SocketAddr, sni: &str) -> Result<quinn::Connection, QuicError> {
        let conn = self.endpoint
            .connect(remote, sni)
            .map_err(|e| QuicError::Quic(e.to_string()))?
            .await
            .map_err(|e| QuicError::Quic(e.to_string()))?;
        Ok(conn)
    }
}

/// Length-prefixed frame (u32 BE) helpers.
/// Returns Ok(None) on clean EOF.
pub async fn read_frame(recv: &mut quinn::RecvStream) -> Result<Option<Bytes>, QuicError> {
    let mut len_buf = [0u8; 4];
    match recv.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(quinn::ReadExactError::FinishedEarly(_)) => return Ok(None),
        Err(e) => return Err(QuicError::Io(e.to_string())),
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 16 * 1024 * 1024 {
        return Err(QuicError::Bad("frame too large".into()));
    }
    let mut data = vec![0u8; len];
    recv.read_exact(&mut data).await.map_err(|e| QuicError::Io(e.to_string()))?;
    Ok(Some(Bytes::from(data)))
}

pub async fn write_frame(send: &mut quinn::SendStream, data: &[u8]) -> Result<(), QuicError> {
    let len = data.len() as u32;
    send.write_all(&len.to_be_bytes()).await.map_err(|e| QuicError::Io(e.to_string()))?;
    send.write_all(data).await.map_err(|e| QuicError::Io(e.to_string()))?;
    send.flush().await.map_err(|e| QuicError::Io(e.to_string()))?;
    Ok(())
}

