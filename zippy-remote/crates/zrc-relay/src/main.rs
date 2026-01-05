//! zrc-relay: QUIC relay server binary

use anyhow::Result;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use zrc_relay::{RelayServer, ServerConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting zrc-relay server");

    // Load configuration
    let config = ServerConfig::load()?;

    // Create and start server
    let server = RelayServer::new(config).await?;
    
    info!("Relay server started, listening for connections");
    
    // Run server
    if let Err(e) = server.run().await {
        error!("Server error: {}", e);
        return Err(e);
    }

    Ok(())
}
