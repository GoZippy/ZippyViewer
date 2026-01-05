//! Directory node server

use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use tracing::info;
use axum::Router;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use crate::config::ServerConfig;
use crate::store::{SqliteStore, RecordStore};
use crate::records::RecordManager;
use crate::access::AccessController;
use crate::discovery::DiscoveryManager;
use crate::search_protection::SearchProtection;
use crate::api::{ApiState, create_router};

/// Directory node server
pub struct DirNodeServer {
    config: ServerConfig,
    record_mgr: Arc<RecordManager>,
    access_ctrl: Arc<AccessController>,
    discovery_mgr: Arc<DiscoveryManager>,
    protection: Arc<SearchProtection>,
}

impl DirNodeServer {
    /// Create new directory node server
    pub async fn new(config: ServerConfig) -> Result<Self> {
        // Create SQLite store
        let store = Arc::new(SqliteStore::new(&config.database_path).await?);
        
        // Create record manager
        let record_config = config.record_config();
        let record_mgr = Arc::new(RecordManager::new(store.clone(), record_config));

        // Create access controller
        let mut access_ctrl = AccessController::new(config.access_mode());
        for token in &config.admin_tokens {
            access_ctrl.add_admin_token(token.clone());
        }
        let access_ctrl = Arc::new(access_ctrl);

        // Create discovery manager
        let discovery_config = config.discovery_config();
        let discovery_mgr = Arc::new(DiscoveryManager::new(discovery_config));

        // Create search protection
        let protection = Arc::new(SearchProtection::new(config.rate_limit_per_minute));

        Ok(Self {
            config,
            record_mgr,
            access_ctrl,
            discovery_mgr,
            protection,
        })
    }

    /// Run the directory node server
    pub async fn run(&self) -> Result<()> {
        info!("Starting directory node on {}", self.config.listen_addr);

        // Create API router
        let api_state = ApiState {
            record_mgr: self.record_mgr.clone(),
            access_ctrl: self.access_ctrl.clone(),
            discovery_mgr: self.discovery_mgr.clone(),
            protection: self.protection.clone(),
        };

        let mut app = create_router(api_state)
            .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

        // Add web UI if enabled
        #[cfg(feature = "web-ui")]
        {
            if self.config.web_ui_enabled {
                use crate::web_ui;
                let web_ui = web_ui::create_router(
                    self.discovery_mgr.clone(),
                    self.access_ctrl.clone(),
                );
                app = app.merge(web_ui);
                info!("Web UI enabled at /ui");
            }
        }

        // Start cleanup task
        let record_mgr = self.record_mgr.clone();
        let discovery_mgr = self.discovery_mgr.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3600));
            loop {
                interval.tick().await;
                record_mgr.cleanup_expired().await;
                discovery_mgr.cleanup_expired();
            }
        });

        // Start HTTP server
        let listener = tokio::net::TcpListener::bind(&self.config.listen_addr).await?;
        info!("HTTP server listening on {}", self.config.listen_addr);
        
        axum::serve(listener, app).await?;

        Ok(())
    }
}
