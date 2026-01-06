use rustls::pki_types::PrivateKeyDer;
use rustls::ServerConfig;
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

#[derive(Clone)]
pub struct TlsConfig {
    inner: Arc<RwLock<Arc<ServerConfig>>>,
    cert_path: std::path::PathBuf,
    key_path: std::path::PathBuf,
}

impl TlsConfig {
    pub fn new(cert_path: impl AsRef<Path>, key_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let cert_path = cert_path.as_ref().to_path_buf();
        let key_path = key_path.as_ref().to_path_buf();

        let config = Self::load_config(&cert_path, &key_path)?;
        let inner = Arc::new(RwLock::new(Arc::new(config)));

        Ok(Self {
            inner,
            cert_path,
            key_path,
        })
    }

    fn load_config(cert_path: &Path, key_path: &Path) -> anyhow::Result<ServerConfig> {
        // Load certificate
        let cert_file = File::open(cert_path)
            .map_err(|e| anyhow::anyhow!("Failed to open certificate file: {}", e))?;
        let mut cert_reader = BufReader::new(cert_file);
        let certs = certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate: {}", e))?;

        if certs.is_empty() {
            anyhow::bail!("No certificates found in certificate file");
        }

        // Load private key
        let key_file = File::open(key_path)
            .map_err(|e| anyhow::anyhow!("Failed to open key file: {}", e))?;
        let mut key_reader = BufReader::new(key_file);
        let mut keys = pkcs8_private_keys(&mut key_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to parse private key: {}", e))?;

        if keys.is_empty() {
            anyhow::bail!("No private keys found in key file");
        }

        let key = PrivateKeyDer::Pkcs8(keys.remove(0));

        // Build server config with TLS 1.2+ only
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                certs.into_iter().collect(),
                key,
            )
            .map_err(|e| anyhow::anyhow!("Failed to build TLS config: {}", e))?;

        // Enforce TLS 1.2+ (rustls 0.23 defaults to TLS 1.2+)
        // The default configuration already enforces TLS 1.2 minimum

        Ok(config)
    }

    pub async fn reload(&self) -> anyhow::Result<()> {
        info!("Reloading TLS certificate and key");
        match Self::load_config(&self.cert_path, &self.key_path) {
            Ok(config) => {
                *self.inner.write().await = Arc::new(config);
                info!("TLS certificate reloaded successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to reload TLS certificate: {}", e);
                Err(e)
            }
        }
    }

    pub async fn get(&self) -> Arc<ServerConfig> {
        self.inner.read().await.clone()
    }
}

pub async fn setup_tls_reload_handler(_tls_config: TlsConfig) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sighup = match signal(SignalKind::hangup()) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!("Failed to register SIGHUP handler: {}", e);
                None
            }
        };

        if let Some(ref mut sighup) = sighup {
            let tls_config = _tls_config;
            tokio::spawn(async move {
                loop {
                    if sighup.recv().await.is_some() {
                        if let Err(e) = tls_config.reload().await {
                            error!("Failed to reload TLS certificate on SIGHUP: {}", e);
                        }
                    }
                }
            });
        }
    }
}
