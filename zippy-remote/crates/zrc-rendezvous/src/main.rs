use zrc_rendezvous::RendezvousServer;
use zrc_rendezvous::config::ServerConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load configuration
    let config = if let Ok(path) = std::env::var("ZRC_CONFIG_PATH") {
        ServerConfig::from_toml(path)?
    } else {
        ServerConfig::from_env()?
    };

    // Create and start server
    let server = RendezvousServer::new(config)?;
    server.start().await?;

    Ok(())
}

