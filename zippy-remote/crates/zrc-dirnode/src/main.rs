//! zrc-dirnode binary entry point

use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use zrc_dirnode::{ServerConfig, DirNodeServer};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = ServerConfig::load()?;

    // Create and run server
    let server = DirNodeServer::new(config).await?;
    server.run().await?;

    Ok(())
}
