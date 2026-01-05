//! QUIC relay server

use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Duration;
use anyhow::{Result, Context};
use tracing::{info, error, warn, debug};
use quinn::{Endpoint, ServerConfig as QuinnServerConfig, Connection as QuinnConnection};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls_pemfile::{certs, pkcs8_private_keys};
use axum::{
    extract::State,
    http::StatusCode,
    response::Response,
    routing::{get, Router},
};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use crate::config::ServerConfig;
use crate::allocation::AllocationManager;
use crate::admin::AdminApi;
use crate::bandwidth::BandwidthLimiter;
use crate::forwarder::Forwarder;
use crate::ha::{HAManager, HAConfig};
use crate::metrics::AllocationMetrics;
use crate::security::SecurityControls;
use crate::token::TokenVerifier;

/// QUIC relay server
pub struct RelayServer {
    config: ServerConfig,
    allocation_mgr: Arc<AllocationManager>,
    bandwidth_limiter: Arc<BandwidthLimiter>,
    forwarder: Arc<Forwarder>,
    metrics: Arc<AllocationMetrics>,
    token_verifier: Arc<TokenVerifier>,
    security: Arc<SecurityControls>,
    endpoint: Arc<Endpoint>,
    ha_manager: Option<HAManager>,
}

impl RelayServer {
    /// Create new relay server
    pub async fn new(config: ServerConfig) -> Result<Self> {
        let allocation_config = config.to_allocation_config();
        let allocation_mgr = Arc::new(AllocationManager::new(allocation_config));
        let bandwidth_limiter = Arc::new(BandwidthLimiter::new(config.global_bandwidth_limit));
        let forwarder = Arc::new(Forwarder::new(
            allocation_mgr.clone(),
            bandwidth_limiter.clone(),
        ));
        let metrics = Arc::new(AllocationMetrics::new()?);
        let token_verifier = Arc::new(TokenVerifier::new());
        let security = Arc::new(SecurityControls::new());

        // Setup High Availability if configured
        let ha_manager = if config.enable_state_sharing || config.instance_id.is_some() {
            let ha_config = HAConfig {
                instance_id: config.instance_id.clone()
                    .unwrap_or_else(|| format!("relay-{}", uuid::Uuid::new_v4().to_string()[..8].to_string())),
                region: config.region.clone(),
                redis_url: config.redis_url.clone(),
                state_sync_interval_secs: config.state_sync_interval_secs,
                enable_state_sharing: config.enable_state_sharing,
            };
            Some(HAManager::new(ha_config, allocation_mgr.clone())?)
        } else {
            None
        };

        // Load TLS certificate and key
        let cert_chain = load_cert_chain(&config.quic_cert_path)?;
        let key = load_private_key(&config.quic_key_path)?;

        // Configure TLS
        let mut tls = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key)
            .context("Failed to create TLS config")?;

        // Set ALPN
        tls.alpn_protocols = vec![b"zrc-relay".to_vec()];

        // Create QUIC server config
        let server_cfg = QuinnServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(tls)
                .context("Failed to create QUIC server config")?
        ));

        // Create QUIC endpoint
        let endpoint = Arc::new(
            Endpoint::server(server_cfg, config.listen_addr)
                .context("Failed to create QUIC endpoint")?
        );

        Ok(Self {
            config,
            allocation_mgr,
            bandwidth_limiter,
            forwarder,
            metrics,
            token_verifier,
            security,
            endpoint,
            ha_manager,
        })
    }

    /// Graceful shutdown with allocation migration
    async fn graceful_shutdown(&self) -> Result<()> {
        info!("Starting graceful shutdown...");
        
        // Stop accepting new connections
        // The endpoint will stop accepting when we drop it, but we can also close it explicitly
        // For now, we'll just wait for existing connections to close
        
        // Give existing allocations time to migrate or close
        let shutdown_timeout = Duration::from_secs(30);
        let start = std::time::Instant::now();
        
        loop {
            let active = self.allocation_mgr.count();
            if active == 0 {
                info!("All allocations closed, shutdown complete");
                break;
            }
            
            if start.elapsed() > shutdown_timeout {
                warn!("Shutdown timeout reached, {} allocations still active", active);
                break;
            }
            
            info!("Waiting for {} allocations to close...", active);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        
        Ok(())
    }

    /// Run the relay server
    pub async fn run(&self) -> Result<()> {
        info!("Starting relay server on {}", self.config.listen_addr);

        // Start HA state sync if enabled
        if let Some(ref ha_manager) = self.ha_manager {
            ha_manager.start_sync();
            info!("High Availability enabled - Instance ID: {}, Region: {:?}",
                ha_manager.instance_id(),
                ha_manager.region());
        }

        // Start HTTP server for health/metrics/admin endpoints
        let metrics_state = self.metrics.clone();
        let allocation_mgr_state = self.allocation_mgr.clone();
        let max_allocations = self.config.max_allocations;
        let mut health_router = Router::new()
            .route("/health", get(health_handler))
            .route("/ready", get(move |State((metrics, allocation_mgr)): State<(Arc<AllocationMetrics>, Arc<AllocationManager>)>| async move {
                ready_handler(State((metrics, allocation_mgr)), max_allocations).await
            }))
            .route("/metrics", get(move |State((metrics, allocation_mgr)): State<(Arc<AllocationMetrics>, Arc<AllocationManager>)>| async move {
                metrics_handler(State(metrics)).await
            }))
            .with_state((metrics_state, allocation_mgr_state))
            .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

        // Add admin API if configured
        if let Some(admin_addr) = self.config.admin_addr {
            if let Some(admin_token) = &self.config.admin_token {
                let admin_api = AdminApi::new(
                    self.allocation_mgr.clone(),
                    self.metrics.clone(),
                    self.security.clone(),
                    admin_token.clone(),
                );
                health_router = health_router.merge(admin_api.router());
                info!("Admin API enabled on {}", admin_addr);
            } else {
                warn!("Admin API address configured but no token provided, disabling admin API");
            }
        }

        let http_addr = SocketAddr::from(([127, 0, 0, 1], 8080));
        let listener = tokio::net::TcpListener::bind(&http_addr).await?;
        
        info!("HTTP server for health/metrics listening on {}", http_addr);
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, health_router).await {
                tracing::error!("HTTP server error: {}", e);
            }
        });

        // Start metrics update task
        let metrics_update = self.metrics.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                metrics_update.update_rate_calc();
            }
        });

        // Start expiration cleanup task
        let allocation_mgr = self.allocation_mgr.clone();
        let metrics = self.metrics.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                allocation_mgr.expire_stale();
                metrics.set_active_allocations(allocation_mgr.count());
            }
        });

        // Start QUIC connection handler
        let endpoint = self.endpoint.clone();
        let allocation_mgr = self.allocation_mgr.clone();
        let forwarder = self.forwarder.clone();
        let token_verifier = self.token_verifier.clone();
        let security = self.security.clone();
        let metrics = self.metrics.clone();
        let keepalive_interval = self.config.keepalive_interval_secs;

        tokio::spawn(async move {
            loop {
                let incoming = endpoint.accept().await;
                let Some(connecting) = incoming else { break };
                
                let allocation_mgr = allocation_mgr.clone();
                let forwarder = forwarder.clone();
                let token_verifier = token_verifier.clone();
                let metrics = metrics.clone();

                let security = security.clone();
                let metrics_clone = metrics.clone();
                tokio::spawn(async move {
                    match connecting.await {
                        Ok(conn) => {
                            // Check connection rate limit
                            if let Err(e) = security.check_connection_rate_limit(conn.remote_address()) {
                                warn!("Connection rate limit exceeded: {}", e);
                                metrics_clone.record_rate_limit_drop();
                                return;
                            }

                            // Check IP filter
                            if let Err(e) = security.check_ip_filter(conn.remote_address()) {
                                warn!("IP filter blocked: {}", e);
                                metrics_clone.record_error();
                                return;
                            }

                            if let Err(e) = handle_connection(
                                conn,
                                allocation_mgr,
                                forwarder,
                                token_verifier,
                                security,
                                metrics_clone.clone(),
                                keepalive_interval,
                            ).await {
                                error!("Connection error: {}", e);
                                metrics_clone.record_error();
                            }
                        }
                        Err(e) => {
                            warn!("Connection failed: {}", e);
                            metrics_clone.record_error();
                        }
                    }
                });
            }
        });

        // Setup graceful shutdown
        let shutdown_signal = async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install CTRL+C handler");
        };

        // Wait for shutdown signal
        shutdown_signal.await;
        info!("Shutdown signal received, starting graceful shutdown");

        // Graceful shutdown with allocation migration
        self.graceful_shutdown().await?;

        info!("Shutting down relay server");
        Ok(())
    }
}

/// Load certificate chain from file
fn load_cert_chain(path: &std::path::Path) -> Result<Vec<CertificateDer<'static>>> {
    let cert_file = std::fs::read(path)
        .with_context(|| format!("Failed to read certificate file: {:?}", path))?;
    let certs = certs(&mut cert_file.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificates")?;
    Ok(certs.into_iter().map(CertificateDer::from).collect())
}

/// Load private key from file
fn load_private_key(path: &std::path::Path) -> Result<PrivateKeyDer<'static>> {
    let key_file = std::fs::read(path)
        .with_context(|| format!("Failed to read key file: {:?}", path))?;
    let mut keys = pkcs8_private_keys(&mut key_file.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse private key")?;
    
    if keys.is_empty() {
        anyhow::bail!("No private keys found in file");
    }
    
    Ok(PrivateKeyDer::from(PrivatePkcs8KeyDer::from(keys.remove(0))))
}

/// Handle incoming QUIC connection
async fn handle_connection(
    conn: QuinnConnection,
    allocation_mgr: Arc<AllocationManager>,
    forwarder: Arc<Forwarder>,
    token_verifier: Arc<TokenVerifier>,
    security: Arc<SecurityControls>,
    metrics: Arc<AllocationMetrics>,
    keepalive_interval: u64,
) -> Result<()> {
    let mut remote_addr = conn.remote_address();
    debug!("New connection from {}", remote_addr);

    // TODO: Receive token from connection
    // For now, this is a placeholder
    // The actual protocol would:
    // 1. Receive token on first datagram or stream
    // 2. Validate token
    // 3. Create or associate allocation
    // 4. Forward datagrams/streams

    // Start keepalive task
    let mut keepalive_conn = conn.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(keepalive_interval));
        loop {
            interval.tick().await;
            // Send keepalive (QUIC handles this automatically, but we can send a ping)
            if keepalive_conn.close_reason().is_some() {
                break;
            }
        }
    });

    // Monitor connection migration (address changes)
    let conn_for_migration = conn.clone();
    tokio::spawn(async move {
        let mut last_addr = conn_for_migration.remote_address();
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if conn_for_migration.close_reason().is_some() {
                break;
            }
            let current_addr = conn_for_migration.remote_address();
            if current_addr != last_addr {
                info!(
                    "Connection migrated from {} to {}",
                    last_addr, current_addr
                );
                last_addr = current_addr;
            }
        }
    });

    // Handle datagrams
    let mut received_bytes = 0u64;
    let mut sent_bytes = 0u64;

    loop {
        // Check for address change (connection migration)
        let current_addr = conn.remote_address();
        if current_addr != remote_addr {
            info!(
                "Connection migration detected: {} -> {}",
                remote_addr, current_addr
            );
            remote_addr = current_addr;
        }

        match conn.read_datagram().await {
            Ok(data) => {
                received_bytes += data.len() as u64;
                debug!("Received datagram: {} bytes", data.len());
                
                // Check for amplification attack
                if let Err(e) = security.check_amplification(
                    remote_addr,
                    sent_bytes,
                    received_bytes,
                ) {
                    warn!("Amplification attack detected: {}", e);
                    metrics.record_error();
                    break;
                }

                // TODO: Parse token, validate, forward
                // For now, just record metrics
                metrics.record_forward(data.len());
            }
            Err(quinn::ConnectionError::ApplicationClosed(_)) => {
                debug!("Connection closed by application");
                break;
            }
            Err(e) => {
                warn!("Datagram read error: {}", e);
                break;
            }
        }
    }

    debug!("Connection closed: {}", remote_addr);
    Ok(())
}

/// Health check handler
async fn health_handler() -> StatusCode {
    StatusCode::OK
}

/// Readiness probe handler
/// Returns 200 if server is ready to accept connections, 503 otherwise
async fn ready_handler(
    State((_metrics, allocation_mgr)): State<(Arc<AllocationMetrics>, Arc<AllocationManager>)>,
    max_allocations: usize,
) -> StatusCode {
    // Check if we're at capacity
    let current_allocations = allocation_mgr.count();
    
    if current_allocations >= max_allocations {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    
    StatusCode::OK
}

/// Metrics export handler
async fn metrics_handler(
    State(metrics): State<Arc<AllocationMetrics>>,
) -> Result<Response<String>, StatusCode> {
    match metrics.export() {
        Ok(body) => Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; version=0.0.4")
            .body(body)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
