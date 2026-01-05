use axum::Router;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::watch;
use tracing::{info, warn};

use crate::api::AppState;
use crate::auth::AuthConfig;
use crate::config::ServerConfig;
use crate::mailbox::MailboxMap;
use crate::metrics::MailboxMetrics;
use crate::rate_limit::RateLimiter;

pub struct RendezvousServer {
    config: ServerConfig,
    mailboxes: MailboxMap,
    rate_limiter: RateLimiter,
    auth: AuthConfig,
    metrics: Arc<MailboxMetrics>,
    shutdown_tx: watch::Sender<bool>,
}

impl RendezvousServer {
    pub fn new(config: ServerConfig) -> anyhow::Result<Self> {
        config.validate()?;

        let mailboxes = Arc::new(dashmap::DashMap::new());
        let rate_limiter = RateLimiter::new(config.rate_limit.clone());
        let auth = {
            let mut auth = AuthConfig::new(config.auth_mode_enum());
            for token in &config.server_tokens {
                auth.add_server_token(token.clone());
            }
            auth
        };
        let metrics = Arc::new(MailboxMetrics::new()?);
        let (shutdown_tx, _) = watch::channel(false);

        Ok(Self {
            config,
            mailboxes,
            rate_limiter,
            auth,
            metrics,
            shutdown_tx,
        })
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        // Setup allowlist/blocklist
        for ip_str in &self.config.allowlist {
            if let Ok(ip) = ip_str.parse() {
                self.rate_limiter.add_to_allowlist(ip);
            }
        }
        for ip_str in &self.config.blocklist {
            if let Ok(ip) = ip_str.parse() {
                self.rate_limiter.add_to_blocklist(ip);
            }
        }

        // Start eviction task
        let mailboxes = self.mailboxes.clone();
        let config = self.config.clone();
        let metrics = Arc::clone(&self.metrics);
        let shutdown_rx = self.shutdown_tx.subscribe();
        tokio::spawn(Self::eviction_task(mailboxes, config, metrics, shutdown_rx));

        // Create app state
        let state = AppState {
            mailboxes: self.mailboxes.clone(),
            rate_limiter: self.rate_limiter.clone(),
            auth: self.auth.clone(),
            metrics: Arc::clone(&self.metrics),
            config: self.config.clone(),
            shutdown: self.shutdown_tx.subscribe(),
        };

        // Build router
        let app = Router::new()
            .route("/v1/mailbox/:rid_hex", axum::routing::post(crate::api::post_mailbox).get(crate::api::get_mailbox))
            .route("/health", axum::routing::get(crate::api::get_health))
            .route("/metrics", axum::routing::get(crate::api::get_metrics))
            .with_state(state);

        // Handle graceful shutdown
        let shutdown_rx = self.shutdown_tx.subscribe();

        // Start server with or without TLS
        if let (Some(cert_path), Some(key_path)) = (&self.config.tls_cert_path, &self.config.tls_key_path) {
            // TLS mode - Note: Full TLS integration requires hyper server setup
            // For now, we'll log a warning and fall back to HTTP
            // Full TLS support can be added with hyper + tokio-rustls integration
            warn!("TLS configuration provided but full TLS integration pending. Starting in HTTP mode.");
            warn!("To enable TLS, use a reverse proxy (nginx, Caddy) or implement hyper server with tokio-rustls.");
            
            let listener = tokio::net::TcpListener::bind(self.config.bind_addr).await?;
            info!("zrc-rendezvous listening on {} (HTTP - TLS config ignored)", self.config.bind_addr);

            // Setup SIGHUP handler for future certificate reload
            let tls_config = crate::tls::TlsConfig::new(cert_path, key_path)?;
            crate::tls::setup_tls_reload_handler(tls_config).await;

            axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
                .with_graceful_shutdown(Self::shutdown_signal(shutdown_rx))
                .await?;
        } else {
            // Plain HTTP mode
            let listener = tokio::net::TcpListener::bind(self.config.bind_addr).await?;
            info!("zrc-rendezvous listening on {} (HTTP)", self.config.bind_addr);

            axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
                .with_graceful_shutdown(Self::shutdown_signal(shutdown_rx))
                .await?;
        }

        Ok(())
    }

    async fn eviction_task(
        mailboxes: MailboxMap,
        config: ServerConfig,
        metrics: Arc<MailboxMetrics>,
        mut shutdown: watch::Receiver<bool>,
    ) {
        let mut interval = tokio::time::interval(config.eviction_interval());
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let mut total_evicted = 0;
                    let mut idle_removed = 0;
                    
                    let mut to_remove = Vec::new();
                    
                    for mut entry in mailboxes.iter_mut() {
                        let rid = entry.key().clone();
                        let mailbox = entry.value_mut();
                        
                        // Evict expired messages
                        let evicted = mailbox.evict_expired(config.message_ttl());
                        total_evicted += evicted;
                        
                        // Check for idle mailbox removal
                        if mailbox.is_idle(config.idle_mailbox_timeout()) {
                            to_remove.push(rid);
                            idle_removed += 1;
                        }
                    }
                    
                    // Remove idle mailboxes
                    for rid in to_remove {
                        mailboxes.remove(&rid);
                    }
                    
                    // Update metrics
                    if total_evicted > 0 {
                        for _ in 0..total_evicted {
                            metrics.messages_evicted.inc();
                        }
                    }
                    metrics.active_mailboxes.set(mailboxes.len() as f64);
                    let total: usize = mailboxes.iter().map(|e| e.value().queue_length()).sum();
                    metrics.total_messages.set(total as f64);
                    
                    if total_evicted > 0 || idle_removed > 0 {
                        info!("Evicted {} messages, removed {} idle mailboxes", total_evicted, idle_removed);
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
            }
        }
    }

    async fn shutdown_signal(mut shutdown: watch::Receiver<bool>) {
        #[cfg(unix)]
        let mut sigterm = {
            use tokio::signal::unix::{signal, SignalKind};
            signal(SignalKind::terminate()).ok()
        };

        tokio::select! {
            _ = async {
                #[cfg(unix)]
                {
                    if let Some(ref mut sigterm) = sigterm {
                        sigterm.recv().await;
                    }
                }
                #[cfg(not(unix))]
                {
                    std::future::pending::<()>().await;
                }
            } => {
                info!("Received SIGTERM, starting graceful shutdown");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received SIGINT, starting graceful shutdown");
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("Shutdown requested");
                }
            }
        }
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}
