//! ZRC Controller CLI entry point

use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use zrc_controller::{Cli, Config, ExitCode};
use zrc_controller::config::CliOverrides;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Create default config on first run (Requirement 10.7)
    if let Err(e) = Config::create_default_if_missing() {
        eprintln!("Warning: Could not create default config: {e}");
    }

    // Load config from custom path or default (Requirements 10.1, 10.6)
    let config = match Config::load_from(cli.config.as_deref()) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Warning: Config error: {e}");
            eprintln!("Using default configuration.");
            Config::default()
        }
    };

    // Build CLI overrides (Requirement 10.5)
    let overrides = CliOverrides {
        output_format: Some(cli.output.to_string()),
        verbose: if cli.verbose { Some(true) } else { None },
        debug: if cli.debug { Some(true) } else { None },
        transport: cli.transport.clone(),
        rendezvous_urls: if cli.rendezvous_urls.is_empty() {
            None
        } else {
            Some(cli.rendezvous_urls.clone())
        },
        relay_urls: if cli.relay_urls.is_empty() {
            None
        } else {
            Some(cli.relay_urls.clone())
        },
        mesh_nodes: if cli.mesh_nodes.is_empty() {
            None
        } else {
            Some(cli.mesh_nodes.clone())
        },
    };

    // Apply CLI overrides to config
    let config = config.with_overrides(&overrides);

    // Initialize logging based on config (with CLI override)
    let filter = if cli.debug {
        EnvFilter::new("debug")
    } else if cli.verbose {
        EnvFilter::new("info")
    } else {
        EnvFilter::try_new(&config.logging.level).unwrap_or_else(|_| EnvFilter::new("warn"))
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    // Execute command with resolved config
    match cli.execute_with_config(config).await {
        Ok(code) => code.to_exit_code(),
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::GeneralError.to_exit_code()
        }
    }
}
