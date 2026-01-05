use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use zrc_agent::*;

#[derive(Parser)]
#[command(name = "zrc-agent")]
#[command(about = "ZRC Host Agent - Remote control daemon")]
struct Args {
    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Run in foreground (for debugging)
    #[arg(short, long)]
    foreground: bool,

    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(format!("zrc_agent={},zrc_core={},zrc_crypto={}", 
            args.log_level, args.log_level, args.log_level))
        .init();

    info!("Starting zrc-agent");

    // Load configuration
    let config = if let Some(config_path) = &args.config {
        config::AgentConfig::load_from_file(config_path)?
    } else {
        config::AgentConfig::load_from_env()
    };

    // Initialize identity manager
    #[cfg(windows)]
    let keystore: Arc<dyn identity::KeyStore> = Arc::new(
        zrc_platform_win::keystore::DpapiKeyStore::new(
            zrc_platform_win::keystore::DpapiScope::CurrentUser
        )
    );

    #[cfg(not(windows))]
    let keystore: Arc<dyn identity::KeyStore> = {
        // TODO: Implement for Linux/macOS
        error!("KeyStore not implemented for this platform");
        return Err(anyhow::anyhow!("KeyStore not available"));
    };

    let identity_mgr = identity::IdentityManager::new(keystore).await?;
    info!("Identity loaded: {}", hex::encode(identity_mgr.device_id()));

    // Initialize service host
    let mut service: Box<dyn service::ServiceHost> = if args.foreground {
        let (service, _rx) = service::ForegroundService::new();
        Box::new(service)
    } else {
        #[cfg(windows)]
        {
            Box::new(service::windows::WindowsServiceHost::new("zrc-agent".to_string())?)
        }
        #[cfg(target_os = "linux")]
        {
            Box::new(service::linux::SystemdServiceHost::new()?)
        }
        #[cfg(target_os = "macos")]
        {
            Box::new(service::macos::LaunchdServiceHost::new()?)
        }
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            let (service, _rx) = service::ForegroundService::new();
            Box::new(service)
        }
    };

    // Start service
    service.start().await?;
    info!("zrc-agent service started");

    // TODO: Initialize and run main agent loop
    // - Pairing manager
    // - Session manager
    // - WebRTC transport
    // - Capture engine
    // - Input injector

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received");

    service.stop().await?;
    info!("zrc-agent stopped");

    Ok(())
}
