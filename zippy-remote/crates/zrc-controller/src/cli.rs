//! CLI command definitions and argument parsing

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::output::OutputFormat;
use crate::ExitCode;

/// ZRC Controller CLI - Remote control client
#[derive(Parser, Debug)]
#[command(name = "zrc-controller")]
#[command(version, about = "ZRC Controller CLI - Remote control client")]
pub struct Cli {
    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,

    /// Output format
    #[arg(long, default_value = "table", global = true)]
    pub output: OutputFormat,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Debug mode (protocol-level tracing)
    #[arg(long, global = true)]
    pub debug: bool,

    /// Config file path
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Transport preference: auto, mesh, rendezvous, direct, relay
    /// Requirements: 8.1, 8.2
    #[arg(long, global = true)]
    pub transport: Option<String>,

    /// Rendezvous server URL (can be specified multiple times)
    /// Requirements: 8.4
    #[arg(long = "rendezvous-url", global = true)]
    pub rendezvous_urls: Vec<String>,

    /// Relay server URL (can be specified multiple times)
    /// Requirements: 8.5
    #[arg(long = "relay-url", global = true)]
    pub relay_urls: Vec<String>,

    /// Mesh node address (can be specified multiple times)
    /// Requirements: 8.6
    #[arg(long = "mesh-node", global = true)]
    pub mesh_nodes: Vec<String>,
}

impl Cli {
    /// Execute the CLI command
    pub async fn execute(self) -> anyhow::Result<ExitCode> {
        // Load default config for backward compatibility
        let config = crate::config::Config::load_default().unwrap_or_default();
        self.execute_with_config(config).await
    }

    /// Execute the CLI command with a pre-loaded configuration
    /// Requirements: 10.5 - CLI arguments override config values
    pub async fn execute_with_config(self, config: crate::config::Config) -> anyhow::Result<ExitCode> {
        // Build transport options from CLI flags
        let transport_opts = TransportOptions {
            preference: self.transport.clone(),
            rendezvous_urls: self.rendezvous_urls.clone(),
            relay_urls: self.relay_urls.clone(),
            mesh_nodes: self.mesh_nodes.clone(),
        };

        match self.command {
            Commands::Pair(args) => args.execute(&self.output, self.verbose, &transport_opts).await,
            Commands::Session(args) => args.execute(&self.output, self.verbose, &transport_opts).await,
            Commands::Input(args) => args.execute(&self.output, self.verbose).await,
            Commands::Pairings(args) => args.execute(&self.output, self.verbose).await,
            Commands::Identity(args) => args.execute(&self.output, self.verbose).await,
            Commands::Frames(args) => args.execute(&self.output, self.verbose).await,
            Commands::Debug(args) => args.execute(&self.output, self.verbose, &transport_opts).await,
        }
    }
}

/// Transport configuration options from CLI flags
/// Requirements: 8.1-8.6
#[derive(Debug, Clone, Default)]
pub struct TransportOptions {
    /// Transport preference (auto, mesh, rendezvous, direct, relay)
    pub preference: Option<String>,
    /// Rendezvous server URLs
    pub rendezvous_urls: Vec<String>,
    /// Relay server URLs
    pub relay_urls: Vec<String>,
    /// Mesh node addresses
    pub mesh_nodes: Vec<String>,
}

impl TransportOptions {
    /// Merge CLI options with config, CLI takes precedence
    /// Requirements: 10.5
    pub fn merge_with_config(&self, config: &crate::config::TransportConfig) -> ResolvedTransport {
        // CLI flags override config values
        let preference = self.preference.clone()
            .unwrap_or_else(|| config.default.clone());
        
        let rendezvous_urls = if self.rendezvous_urls.is_empty() {
            config.rendezvous_urls.clone()
        } else {
            self.rendezvous_urls.clone()
        };

        let relay_urls = if self.relay_urls.is_empty() {
            config.relay_urls.clone()
        } else {
            self.relay_urls.clone()
        };

        let mesh_nodes = if self.mesh_nodes.is_empty() {
            config.mesh_nodes.clone()
        } else {
            self.mesh_nodes.clone()
        };

        ResolvedTransport {
            preference,
            rendezvous_urls,
            relay_urls,
            mesh_nodes,
            timeout_seconds: config.timeout_seconds,
        }
    }
}

/// Resolved transport configuration after merging CLI and config
#[derive(Debug, Clone)]
pub struct ResolvedTransport {
    /// Transport preference
    pub preference: String,
    /// Rendezvous server URLs
    pub rendezvous_urls: Vec<String>,
    /// Relay server URLs
    pub relay_urls: Vec<String>,
    /// Mesh node addresses
    pub mesh_nodes: Vec<String>,
    /// Connection timeout in seconds
    pub timeout_seconds: u64,
}

/// Available commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Pair with a device
    Pair(PairArgs),
    /// Manage sessions
    Session(SessionArgs),
    /// Send input commands
    Input(InputArgs),
    /// Manage pairings
    Pairings(PairingsArgs),
    /// Manage operator identity
    Identity(IdentityArgs),
    /// Receive and display frames
    Frames(FramesArgs),
    /// Debug and diagnostic tools
    Debug(DebugArgs),
}

/// Arguments for the pair command
#[derive(Parser, Debug)]
pub struct PairArgs {
    /// Import invite from base64, file, or QR image
    #[arg(long)]
    pub invite: Option<String>,

    /// Device ID to pair with
    #[arg(long)]
    pub device: Option<String>,

    /// Requested permissions (comma-separated)
    #[arg(long)]
    pub permissions: Option<String>,

    /// Dry run (validate only, don't store)
    #[arg(long)]
    pub dry_run: bool,

    /// Transport preference
    #[arg(long, default_value = "auto")]
    pub transport: String,
}

impl PairArgs {
    pub async fn execute(self, output: &OutputFormat, verbose: bool, transport_opts: &TransportOptions) -> anyhow::Result<ExitCode> {
        use crate::config::Config;
        use crate::identity::IdentityManager;
        use crate::output::OutputFormatter;
        use crate::pairing::{InviteSource, PairingClient, TransportClient, TransportPreference};
        use std::path::PathBuf;

        let formatter = OutputFormatter::new(*output, verbose);
        
        // Load config and identity
        let config = Config::load_default().unwrap_or_default();
        let identity = IdentityManager::init(&config.identity).await?;
        let identity = std::sync::Arc::new(identity);

        // Merge CLI transport options with config (CLI takes precedence)
        let resolved = transport_opts.merge_with_config(&config.transport);

        // Create transport client with resolved URLs
        let transport = TransportClient::with_urls(
            resolved.rendezvous_urls.clone(),
            resolved.relay_urls.clone(),
            resolved.mesh_nodes.clone(),
        );

        // Create pairing client
        let mut client = PairingClient::with_config(identity, transport, None);

        // Set transport preference from resolved config or command-specific override
        // Command-specific --transport flag takes precedence over global --transport
        let transport_pref: TransportPreference = if self.transport != "auto" {
            // Command-specific override
            self.transport.parse().unwrap_or_default()
        } else {
            // Use resolved preference (from global CLI or config)
            resolved.preference.parse().unwrap_or_default()
        };
        client.set_transport_preference(transport_pref);

        if verbose {
            formatter.progress(&format!("Using transport: {:?}", transport_pref));
            if !resolved.rendezvous_urls.is_empty() {
                formatter.progress(&format!("Rendezvous URLs: {:?}", resolved.rendezvous_urls));
            }
            if !resolved.relay_urls.is_empty() {
                formatter.progress(&format!("Relay URLs: {:?}", resolved.relay_urls));
            }
            if !resolved.mesh_nodes.is_empty() {
                formatter.progress(&format!("Mesh nodes: {:?}", resolved.mesh_nodes));
            }
        }

        // Handle invite import
        if let Some(invite_str) = self.invite {
            formatter.progress("Importing invite...");

            // Determine the source type
            let source = if std::path::Path::new(&invite_str).exists() {
                // Check if it's an image file (QR code)
                let path = PathBuf::from(&invite_str);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp") {
                    InviteSource::QrImage(path)
                } else {
                    InviteSource::File(path)
                }
            } else {
                // Assume base64-encoded string
                InviteSource::Base64(invite_str)
            };

            match client.import_invite(source) {
                Ok(parsed) => {
                    if self.dry_run {
                        formatter.success("Invite validated successfully (dry run)");
                    } else {
                        formatter.success("Invite imported successfully");
                    }

                    // Display invite details
                    println!("{}", formatter.format_invite(&parsed));

                    if parsed.is_expired() {
                        eprintln!("Warning: This invite has expired!");
                        return Ok(ExitCode::InvalidInput);
                    }

                    if self.dry_run {
                        return Ok(ExitCode::Success);
                    }

                    // Store invite for subsequent pairing
                    // The invite is now stored in the client state
                    Ok(ExitCode::Success)
                }
                Err(e) => {
                    formatter.error(&format!("Failed to import invite: {e}"));
                    Ok(ExitCode::InvalidInput)
                }
            }
        } else if let Some(device_id) = self.device {
            // Pairing with a device requires the invite secret
            // For now, we need the invite to be imported first
            formatter.progress(&format!("Initiating pairing with device {}...", device_id));
            
            // In a full implementation, we would:
            // 1. Look up the stored invite for this device
            // 2. Prompt for the invite secret (or read from secure storage)
            // 3. Generate and send the pair request
            // 4. Wait for receipt
            // 5. Display SAS and prompt for confirmation
            // 6. Store the pairing
            
            eprintln!("Note: Full pairing flow requires invite secret.");
            eprintln!("Use --invite to import an invite first, then pair with --device.");
            
            Ok(ExitCode::Success)
        } else {
            eprintln!("Error: Either --invite or --device must be specified");
            Ok(ExitCode::InvalidInput)
        }
    }

    /// Execute the full pairing flow (for use when invite secret is available)
    #[allow(dead_code)]
    async fn execute_pairing_flow(
        client: &mut crate::pairing::PairingClient,
        invite_secret: &[u8; 32],
        permissions: u32,
        formatter: &crate::output::OutputFormatter,
    ) -> anyhow::Result<ExitCode> {
        use std::io::{self, Write};

        // Generate and send pair request
        formatter.progress("Sending pair request...");
        let _request = client.send_pair_request(invite_secret, permissions).await?;
        formatter.success("Pair request sent");

        // Wait for receipt
        formatter.progress("Waiting for device response...");
        let receipt = client.wait_for_receipt().await?;
        formatter.success("Received pair receipt");

        // Handle receipt and get SAS
        let sas = client.handle_receipt(receipt)?;
        
        // Display SAS for verification
        println!("\n╔════════════════════════════════════════╗");
        println!("║     SAS Verification Code              ║");
        println!("║                                        ║");
        println!("║           {:^6}                       ║", sas);
        println!("║                                        ║");
        println!("║  Verify this code matches the device   ║");
        println!("╚════════════════════════════════════════╝\n");

        // Prompt for confirmation
        eprint!("Does the code match? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().eq_ignore_ascii_case("y") {
            // Confirm SAS and complete pairing
            formatter.progress("Confirming pairing...");
            let result = client.confirm_sas().await?;
            
            formatter.success(&format!(
                "Pairing complete! Device: {}, Permissions: {:?}",
                result.device_id,
                result.permissions_granted
            ));
            
            Ok(ExitCode::Success)
        } else {
            // Reject SAS
            client.reject_sas()?;
            formatter.error("SAS verification rejected - pairing cancelled");
            Ok(ExitCode::AuthenticationFailed)
        }
    }
}

/// Arguments for the session command
#[derive(Parser, Debug)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub action: SessionAction,
}

impl SessionArgs {
    pub async fn execute(self, output: &OutputFormat, verbose: bool, transport_opts: &TransportOptions) -> anyhow::Result<ExitCode> {
        use crate::config::Config;
        use crate::identity::IdentityManager;
        use crate::output::OutputFormatter;
        use crate::pairing::{TransportClient, TransportPreference};
        use crate::pairings::PairingsStore;
        use crate::session::{SessionClient, SessionOptions};

        let formatter = OutputFormatter::new(*output, verbose);

        match self.action {
            SessionAction::Start { device, capabilities, transport } => {
                // Load config and identity
                let config = Config::load_default().unwrap_or_default();
                let identity = IdentityManager::init(&config.identity).await?;
                let identity = std::sync::Arc::new(identity);

                // Merge CLI transport options with config (CLI takes precedence)
                let resolved = transport_opts.merge_with_config(&config.transport);

                // Open pairings store
                let pairings_store = if let Some(path) = &config.pairings.db_path {
                    PairingsStore::open(path).ok()
                } else if let Some(path) = PairingsStore::default_path() {
                    PairingsStore::open(&path).ok()
                } else {
                    None
                };

                // Create transport client with resolved URLs
                let transport_client = TransportClient::with_urls(
                    resolved.rendezvous_urls.clone(),
                    resolved.relay_urls.clone(),
                    resolved.mesh_nodes.clone(),
                );

                // Create session client
                let mut client = SessionClient::with_config(
                    identity,
                    transport_client,
                    pairings_store,
                );

                // Set transport preference: command-specific > CLI global > config
                let transport_pref: TransportPreference = transport
                    .as_deref()
                    .map(|s| s.parse().unwrap_or_default())
                    .unwrap_or_else(|| resolved.preference.parse().unwrap_or_default());
                client.set_transport_preference(transport_pref);

                if verbose {
                    formatter.progress(&format!("Using transport: {:?}", transport_pref));
                    if !resolved.rendezvous_urls.is_empty() {
                        formatter.progress(&format!("Rendezvous URLs: {:?}", resolved.rendezvous_urls));
                    }
                }

                // Parse capabilities
                let caps: Vec<String> = capabilities
                    .map(|c| c.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default();

                let options = SessionOptions {
                    capabilities: caps,
                    transport_preference: transport_pref,
                    timeout: std::time::Duration::from_secs(resolved.timeout_seconds),
                };

                // Verify pairing and generate session request (Requirements: 3.1, 3.2, 3.3)
                formatter.progress(&format!("Verifying pairing with device {}...", device));

                match client.start_session(&device, options).await {
                    Ok(request) => {
                        formatter.success("Session request generated");
                        
                        // Display request info
                        let session_id = hex::encode(&request.session_id);
                        println!("Session ID: {}", session_id);
                        println!("Requested capabilities: 0x{:02x}", request.requested_capabilities);
                        
                        // Send the request via transport
                        formatter.progress("Sending session request...");
                        
                        let device_id_bytes = hex::decode(&device)
                            .map_err(|e| anyhow::anyhow!("Invalid device ID: {}", e))?;
                        
                        match client.send_session_request(&device_id_bytes, &request).await {
                            Ok(()) => {
                                formatter.success("Session request sent");
                                formatter.progress("Waiting for device response...");
                                
                                // Note: Response handling will be implemented in task 7.2
                                println!("\nSession request sent successfully.");
                                println!("Use 'zrc-controller session connect' with the response parameters to establish the connection.");
                                
                                Ok(ExitCode::Success)
                            }
                            Err(e) => {
                                formatter.error(&format!("Failed to send session request: {}", e));
                                Ok(ExitCode::ConnectionFailed)
                            }
                        }
                    }
                    Err(crate::session::SessionError::NotPaired(id)) => {
                        formatter.error(&format!("Device {} is not paired", id));
                        eprintln!("Use 'zrc-controller pair --device {}' to pair first.", id);
                        Ok(ExitCode::NotPaired)
                    }
                    Err(crate::session::SessionError::PermissionDenied(msg)) => {
                        formatter.error(&format!("Permission denied: {}", msg));
                        Ok(ExitCode::PermissionDenied)
                    }
                    Err(e) => {
                        formatter.error(&format!("Failed to start session: {}", e));
                        Ok(ExitCode::GeneralError)
                    }
                }
            }
            SessionAction::Connect { quic, cert, ticket, relay } => {
                // TODO: Implement in task 7.3
                formatter.progress(&format!("Connecting to {}...", quic));
                eprintln!("Session connect not yet implemented (task 7.3)");
                eprintln!("Parameters: quic={}, cert={}, ticket_len={}, relay={:?}", 
                    quic, cert, ticket.len(), relay);
                Ok(ExitCode::Success)
            }
            SessionAction::List => {
                // Load config and identity
                let config = Config::load_default().unwrap_or_default();
                let identity = IdentityManager::init(&config.identity).await?;
                let identity = std::sync::Arc::new(identity);

                let client = SessionClient::with_identity(identity);
                let sessions = client.list_sessions().await;

                if sessions.is_empty() {
                    println!("No active sessions.");
                } else {
                    println!("Active sessions:");
                    for session_id in sessions {
                        println!("  - {}", session_id);
                    }
                }
                Ok(ExitCode::Success)
            }
            SessionAction::End { session } => {
                // Load config and identity
                let config = Config::load_default().unwrap_or_default();
                let identity = IdentityManager::init(&config.identity).await?;
                let identity = std::sync::Arc::new(identity);

                let client = SessionClient::with_identity(identity);
                
                formatter.progress(&format!("Ending session {}...", session));
                match client.end_session(&session).await {
                    Ok(()) => {
                        formatter.success("Session ended");
                        Ok(ExitCode::Success)
                    }
                    Err(crate::session::SessionError::NotFound(id)) => {
                        formatter.error(&format!("Session {} not found", id));
                        Ok(ExitCode::GeneralError)
                    }
                    Err(e) => {
                        formatter.error(&format!("Failed to end session: {}", e));
                        Ok(ExitCode::GeneralError)
                    }
                }
            }
        }
    }
}

/// Session subcommands
#[derive(Subcommand, Debug)]
pub enum SessionAction {
    /// Start a new session
    Start {
        /// Device ID to connect to
        #[arg(long)]
        device: String,
        /// Requested capabilities (comma-separated)
        #[arg(long)]
        capabilities: Option<String>,
        /// Transport preference (auto, mesh, rendezvous, direct, relay)
        #[arg(long)]
        transport: Option<String>,
    },
    /// Connect to established session via QUIC
    Connect {
        /// QUIC endpoint (host:port)
        #[arg(long)]
        quic: String,
        /// Server certificate fingerprint
        #[arg(long)]
        cert: String,
        /// Session ticket (base64)
        #[arg(long)]
        ticket: String,
        /// Relay server URL (optional)
        #[arg(long)]
        relay: Option<String>,
    },
    /// List active sessions
    List,
    /// End a session
    End {
        /// Session ID to end
        #[arg(long)]
        session: String,
    },
}

/// Arguments for the input command
#[derive(Parser, Debug)]
pub struct InputArgs {
    #[command(subcommand)]
    pub action: InputAction,

    /// Session ID to send input to
    #[arg(long, global = true)]
    pub session: Option<String>,
}

impl InputArgs {
    pub async fn execute(self, output: &OutputFormat, verbose: bool) -> anyhow::Result<ExitCode> {
        use crate::input::{InputCommands, InputResult, KeyCode, MouseButton};
        use crate::output::OutputFormatter;

        let formatter = OutputFormatter::new(*output, verbose);

        // Create input commands handler with session if provided
        let cmds = if let Some(ref session_id) = self.session {
            InputCommands::with_session(session_id.clone())
        } else {
            InputCommands::new()
        };

        let result: Result<InputResult, crate::input::InputError> = match self.action {
            InputAction::Mouse { x, y, click } => {
                if let Some(button_str) = click {
                    // Parse button and send click
                    let button: MouseButton = button_str.parse().map_err(|e: String| {
                        crate::input::InputError::InvalidInput(e)
                    })?;
                    formatter.progress(&format!("Sending mouse click {:?} at ({}, {})", button, x, y));
                    cmds.mouse_click(self.session.as_deref(), x, y, button).await
                } else {
                    // Just move
                    formatter.progress(&format!("Sending mouse move to ({}, {})", x, y));
                    cmds.mouse_move(self.session.as_deref(), x, y).await
                }
            }
            InputAction::Key { code, down, up } => {
                let key_code = KeyCode::new(code);
                if down && up {
                    // Both down and up - send key press (down then up)
                    formatter.progress(&format!("Sending key press for code {}", code));
                    let down_result = cmds.key(self.session.as_deref(), key_code, true).await?;
                    let _up_result = cmds.key(self.session.as_deref(), key_code, false).await?;
                    Ok(down_result)
                } else if down {
                    formatter.progress(&format!("Sending key down for code {}", code));
                    cmds.key(self.session.as_deref(), key_code, true).await
                } else if up {
                    formatter.progress(&format!("Sending key up for code {}", code));
                    cmds.key(self.session.as_deref(), key_code, false).await
                } else {
                    // Default to key press (down + up)
                    formatter.progress(&format!("Sending key press for code {}", code));
                    let down_result = cmds.key(self.session.as_deref(), key_code, true).await?;
                    let _up_result = cmds.key(self.session.as_deref(), key_code, false).await?;
                    Ok(down_result)
                }
            }
            InputAction::Text { string } => {
                formatter.progress(&format!("Sending text input: {} chars", string.len()));
                cmds.text(self.session.as_deref(), &string).await
            }
            InputAction::Scroll { delta } => {
                formatter.progress(&format!("Sending scroll delta {}", delta));
                cmds.scroll(self.session.as_deref(), delta).await
            }
        };

        match result {
            Ok(input_result) => {
                formatter.success(&input_result.details);
                
                // Output result in requested format
                if *output == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&input_result)?);
                }
                
                Ok(ExitCode::Success)
            }
            Err(crate::input::InputError::NoSession) => {
                formatter.error("No active session. Use --session to specify a session ID.");
                Ok(ExitCode::InvalidInput)
            }
            Err(crate::input::InputError::InvalidInput(msg)) => {
                formatter.error(&format!("Invalid input: {}", msg));
                Ok(ExitCode::InvalidInput)
            }
            Err(crate::input::InputError::PermissionDenied(msg)) => {
                formatter.error(&format!("Permission denied: {}", msg));
                Ok(ExitCode::PermissionDenied)
            }
            Err(e) => {
                formatter.error(&format!("Input command failed: {}", e));
                Ok(ExitCode::GeneralError)
            }
        }
    }
}

/// Input subcommands
#[derive(Subcommand, Debug)]
pub enum InputAction {
    /// Send mouse input
    Mouse {
        /// X coordinate
        #[arg(long)]
        x: i32,
        /// Y coordinate
        #[arg(long)]
        y: i32,
        /// Mouse button to click
        #[arg(long)]
        click: Option<String>,
    },
    /// Send key input
    Key {
        /// Virtual key code
        #[arg(long)]
        code: u32,
        /// Key down event
        #[arg(long)]
        down: bool,
        /// Key up event
        #[arg(long)]
        up: bool,
    },
    /// Send text input
    Text {
        /// Text string to send
        #[arg(long)]
        string: String,
    },
    /// Send scroll input
    Scroll {
        /// Scroll delta
        #[arg(long)]
        delta: i32,
    },
}

/// Arguments for the pairings command
#[derive(Parser, Debug)]
pub struct PairingsArgs {
    #[command(subcommand)]
    pub action: PairingsAction,
}

impl PairingsArgs {
    pub async fn execute(self, output: &OutputFormat, verbose: bool) -> anyhow::Result<ExitCode> {
        use crate::config::Config;
        use crate::output::OutputFormatter;
        use crate::pairings::PairingsStore;
        use std::io::{self, Write};

        let formatter = OutputFormatter::new(*output, verbose);
        let config = Config::load_default().unwrap_or_default();

        // Open pairings store
        let db_path = config.pairings.db_path
            .or_else(PairingsStore::default_path)
            .ok_or_else(|| anyhow::anyhow!("Could not determine pairings database path"))?;

        let store = PairingsStore::open(&db_path)?;

        match self.action {
            PairingsAction::List => {
                // Requirements: 7.1
                formatter.progress("Loading pairings...");
                let pairings = store.list()?;
                println!("{}", formatter.format_pairings(&pairings));
                Ok(ExitCode::Success)
            }
            PairingsAction::Show { device_id } => {
                // Requirements: 7.3
                formatter.progress(&format!("Loading pairing for {}...", device_id));
                match store.get(&device_id)? {
                    Some(pairing) => {
                        println!("{}", formatter.format_pairing_detail(&pairing));
                        Ok(ExitCode::Success)
                    }
                    None => {
                        formatter.error(&format!("Pairing not found: {}", device_id));
                        Ok(ExitCode::NotPaired)
                    }
                }
            }
            PairingsAction::Revoke { device_id, force } => {
                // Requirements: 7.4, 7.7
                formatter.progress(&format!("Looking up pairing for {}...", device_id));
                
                // Check if pairing exists
                let pairing = match store.get(&device_id)? {
                    Some(p) => p,
                    None => {
                        formatter.error(&format!("Pairing not found: {}", device_id));
                        return Ok(ExitCode::NotPaired);
                    }
                };

                // Confirm unless --force is specified
                if !force {
                    let device_name = pairing.device_name.as_deref().unwrap_or(&device_id);
                    eprintln!("WARNING: This will revoke the pairing with device '{}'.", device_name);
                    eprintln!("You will need to re-pair to connect to this device again.");
                    eprint!("Are you sure you want to continue? [y/N] ");
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    
                    if !input.trim().eq_ignore_ascii_case("y") {
                        eprintln!("Aborted.");
                        return Ok(ExitCode::Success);
                    }
                }

                store.delete(&device_id)?;
                formatter.success(&format!("Pairing revoked for device: {}", device_id));
                Ok(ExitCode::Success)
            }
            PairingsAction::Export { output: output_path } => {
                // Requirements: 7.5
                formatter.progress(&format!("Exporting pairings to {}...", output_path.display()));
                store.export(&output_path)?;
                
                let count = store.list()?.len();
                formatter.success(&format!("Exported {} pairings to {}", count, output_path.display()));
                Ok(ExitCode::Success)
            }
            PairingsAction::Import { input: input_path } => {
                // Requirements: 7.6
                if !input_path.exists() {
                    formatter.error(&format!("File not found: {}", input_path.display()));
                    return Ok(ExitCode::InvalidInput);
                }

                formatter.progress(&format!("Importing pairings from {}...", input_path.display()));
                let count = store.import(&input_path)?;
                formatter.success(&format!("Imported {} pairings from {}", count, input_path.display()));
                Ok(ExitCode::Success)
            }
        }
    }
}

/// Pairings subcommands
#[derive(Subcommand, Debug)]
pub enum PairingsAction {
    /// List all pairings
    List,
    /// Show details for a specific pairing
    Show {
        /// Device ID
        device_id: String,
    },
    /// Revoke a pairing
    Revoke {
        /// Device ID
        device_id: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
    /// Export pairings to file
    Export {
        /// Output file path
        #[arg(long)]
        output: PathBuf,
    },
    /// Import pairings from file
    Import {
        /// Input file path
        #[arg(long)]
        input: PathBuf,
    },
}

/// Arguments for the identity command
#[derive(Parser, Debug)]
pub struct IdentityArgs {
    #[command(subcommand)]
    pub action: IdentityAction,

    /// Path to identity file (overrides config)
    #[arg(long, global = true)]
    pub identity_file: Option<PathBuf>,
}

impl IdentityArgs {
    pub async fn execute(self, output: &OutputFormat, verbose: bool) -> anyhow::Result<ExitCode> {
        use crate::config::Config;
        use crate::identity::IdentityManager;
        use crate::output::OutputFormatter;
        use std::io::{self, Write};

        let formatter = OutputFormatter::new(*output, verbose);

        // Build identity config, potentially with override
        let mut config = Config::load_default().unwrap_or_default();
        if let Some(path) = self.identity_file {
            config.identity.key_path = Some(path);
        }

        match self.action {
            IdentityAction::Show => {
                formatter.progress("Loading identity...");
                let identity = IdentityManager::init(&config.identity).await?;
                let info = identity.display_info();
                println!("{}", formatter.format_identity(&info));
                Ok(ExitCode::Success)
            }
            IdentityAction::Export { output_file } => {
                formatter.progress("Loading identity...");
                let identity = IdentityManager::init(&config.identity).await?;
                
                formatter.progress(&format!("Exporting to {}...", output_file.display()));
                identity.export_to_file(&output_file)?;
                
                formatter.success(&format!("Identity exported to {}", output_file.display()));
                Ok(ExitCode::Success)
            }
            IdentityAction::Rotate { force } => {
                // Warn user about consequences
                if !force {
                    eprintln!("WARNING: Rotating your identity will invalidate ALL existing pairings.");
                    eprintln!("You will need to re-pair with all devices.");
                    eprint!("Are you sure you want to continue? [y/N] ");
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    
                    if !input.trim().eq_ignore_ascii_case("y") {
                        eprintln!("Aborted.");
                        return Ok(ExitCode::Success);
                    }
                }

                formatter.progress("Loading identity...");
                let mut identity = IdentityManager::init(&config.identity).await?;
                
                let old_id = identity.operator_id().to_string();
                formatter.progress("Rotating identity...");
                identity.rotate().await?;
                
                let info = identity.display_info();
                formatter.success(&format!(
                    "Identity rotated. Old ID: {}, New ID: {}",
                    old_id,
                    info.operator_id
                ));
                
                println!("{}", formatter.format_identity(&info));
                Ok(ExitCode::Success)
            }
        }
    }
}

/// Identity subcommands
#[derive(Subcommand, Debug)]
pub enum IdentityAction {
    /// Show current identity
    Show,
    /// Export identity (public info only)
    Export {
        /// Output file path
        #[arg(long = "file", short = 'f')]
        output_file: PathBuf,
    },
    /// Rotate identity (warning: breaks existing pairings)
    Rotate {
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
}

/// Arguments for the frames command
#[derive(Parser, Debug)]
pub struct FramesArgs {
    #[command(subcommand)]
    pub action: FramesAction,

    /// Session ID to receive frames from
    #[arg(long, global = true)]
    pub session: Option<String>,
}

impl FramesArgs {
    pub async fn execute(self, output: &OutputFormat, verbose: bool) -> anyhow::Result<ExitCode> {
        use crate::frames::{FrameSaver, FrameStats, SaveFormat};
        use crate::output::OutputFormatter;

        let formatter = OutputFormatter::new(*output, verbose);

        match self.action {
            FramesAction::Save { output: output_path, format } => {
                // Parse save format
                let save_format: SaveFormat = format.parse().map_err(|e: String| {
                    anyhow::anyhow!("Invalid format: {}", e)
                })?;

                formatter.progress(&format!("Saving frames to {}...", output_path.display()));

                // Check if we have a session
                if self.session.is_none() {
                    formatter.error("No session specified. Use --session to specify a session ID.");
                    eprintln!("Note: Frame reception requires an active QUIC session.");
                    eprintln!("Use 'zrc-controller session connect' to establish a session first.");
                    return Ok(ExitCode::InvalidInput);
                }

                // Create frame saver
                let saver = FrameSaver::new(&output_path, save_format)?;

                formatter.success(&format!(
                    "Frame saver initialized. Output: {}, Format: {:?}",
                    output_path.display(),
                    save_format
                ));

                // Note: In a full implementation, we would:
                // 1. Get the frame receiver from the active session
                // 2. Loop receiving frames and saving them
                // 3. Handle Ctrl+C to stop gracefully
                
                eprintln!("Note: Frame saving requires an active QUIC session with frame stream.");
                eprintln!("This will be fully functional once session connect (task 7.3) is implemented.");

                let frames_saved = saver.finish()?;
                formatter.success(&format!("Saved {} frames", frames_saved));

                Ok(ExitCode::Success)
            }
            FramesAction::Stats => {
                formatter.progress("Fetching frame statistics...");

                // Check if we have a session
                if self.session.is_none() {
                    // Show empty/default stats
                    let stats = FrameStats::default();
                    println!("{}", format_frame_stats(&stats, *output));
                    
                    eprintln!("\nNote: No active session. Use --session to specify a session ID.");
                    return Ok(ExitCode::Success);
                }

                // Note: In a full implementation, we would get stats from the active session
                let stats = FrameStats::default();
                println!("{}", format_frame_stats(&stats, *output));

                Ok(ExitCode::Success)
            }
        }
    }
}

/// Format frame statistics for output
fn format_frame_stats(stats: &crate::frames::FrameStats, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => {
            serde_json::to_string_pretty(&FrameStatsJson::from(stats))
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
        }
        OutputFormat::Table => {
            use comfy_table::{presets::UTF8_FULL, Table};
            
            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(vec!["Metric", "Value"]);
            
            table.add_row(vec!["Frames Received", &stats.frames_received.to_string()]);
            table.add_row(vec!["Frames Dropped", &stats.frames_dropped.to_string()]);
            table.add_row(vec!["Bytes Received", &format_bytes(stats.bytes_received)]);
            table.add_row(vec!["Frame Rate", &format!("{:.1} fps", stats.frame_rate)]);
            table.add_row(vec!["Bandwidth", &format!("{}/s", format_bytes(stats.bandwidth as u64))]);
            
            if let Some((w, h)) = stats.resolution {
                table.add_row(vec!["Resolution", &format!("{}x{}", w, h)]);
            } else {
                table.add_row(vec!["Resolution", "N/A"]);
            }
            
            if let Some(fmt) = &stats.format {
                table.add_row(vec!["Format", &format!("{}", fmt)]);
            } else {
                table.add_row(vec!["Format", "N/A"]);
            }
            
            table.add_row(vec!["Avg Frame Size", &format_bytes(stats.avg_frame_size)]);
            table.add_row(vec!["Keyframes", &stats.keyframes.to_string()]);
            table.add_row(vec!["Partial Frames", &stats.partial_frames.to_string()]);
            
            table.to_string()
        }
        OutputFormat::Quiet => String::new(),
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// JSON output for frame stats
#[derive(serde::Serialize)]
struct FrameStatsJson {
    frames_received: u64,
    frames_dropped: u64,
    bytes_received: u64,
    frame_rate: f64,
    bandwidth: f64,
    resolution: Option<(u32, u32)>,
    format: Option<String>,
    avg_frame_size: u64,
    keyframes: u64,
    partial_frames: u64,
    timestamp: String,
}

impl From<&crate::frames::FrameStats> for FrameStatsJson {
    fn from(s: &crate::frames::FrameStats) -> Self {
        Self {
            frames_received: s.frames_received,
            frames_dropped: s.frames_dropped,
            bytes_received: s.bytes_received,
            frame_rate: s.frame_rate,
            bandwidth: s.bandwidth,
            resolution: s.resolution,
            format: s.format.map(|f| format!("{}", f)),
            avg_frame_size: s.avg_frame_size,
            keyframes: s.keyframes,
            partial_frames: s.partial_frames,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Frames subcommands
#[derive(Subcommand, Debug)]
pub enum FramesAction {
    /// Save frames to file
    Save {
        /// Output file path
        #[arg(long)]
        output: PathBuf,
        /// Output format (raw, png)
        #[arg(long, default_value = "raw")]
        format: String,
    },
    /// Display frame statistics
    Stats,
}

/// Arguments for the debug command
#[derive(Parser, Debug)]
pub struct DebugArgs {
    #[command(subcommand)]
    pub action: DebugAction,
}

impl DebugArgs {
    /// Execute debug commands
    /// Requirements: 12.1, 12.2, 12.3, 12.4, 12.6
    pub async fn execute(self, output: &OutputFormat, verbose: bool, _transport_opts: &TransportOptions) -> anyhow::Result<ExitCode> {
        use crate::debug::DebugTools;
        use crate::output::OutputFormatter;
        use std::time::Duration;

        let formatter = OutputFormatter::new(*output, verbose);
        let tools = DebugTools::with_verbose(verbose);

        match self.action {
            DebugAction::Envelope { decode } => {
                // Requirements: 12.1 - Decode and display envelope
                formatter.progress("Decoding envelope...");

                match tools.decode_envelope(&decode) {
                    Ok(info) => {
                        match output {
                            OutputFormat::Json => {
                                println!("{}", serde_json::to_string_pretty(&info)?);
                            }
                            OutputFormat::Table => {
                                use comfy_table::{presets::UTF8_FULL, Table};
                                
                                let mut table = Table::new();
                                table.load_preset(UTF8_FULL);
                                table.set_header(vec!["Field", "Value"]);
                                
                                table.add_row(vec!["Version", &info.version.to_string()]);
                                table.add_row(vec!["Message Type", &info.msg_type]);
                                table.add_row(vec!["Message Type Value", &info.msg_type_value.to_string()]);
                                table.add_row(vec!["Sender ID", &info.sender_id]);
                                table.add_row(vec!["Recipient ID", &info.recipient_id]);
                                table.add_row(vec!["Timestamp", &info.timestamp]);
                                table.add_row(vec!["Timestamp (Unix)", &info.timestamp_unix.to_string()]);
                                table.add_row(vec!["Nonce", &info.nonce]);
                                table.add_row(vec!["Sender KEX Pub", &info.sender_kex_pub]);
                                table.add_row(vec!["Payload Size", &format!("{} bytes", info.payload_size)]);
                                table.add_row(vec!["Signature Size", &format!("{} bytes", info.signature_size)]);
                                table.add_row(vec!["AAD Size", &format!("{} bytes", info.aad_size)]);
                                table.add_row(vec!["Raw Size", &format!("{} bytes", info.raw_size)]);
                                
                                if let Some(valid) = info.signature_valid {
                                    table.add_row(vec!["Signature Valid", if valid { "Yes" } else { "No" }]);
                                }
                                
                                println!("{}", table);
                            }
                            OutputFormat::Quiet => {}
                        }
                        formatter.success("Envelope decoded successfully");
                        Ok(ExitCode::Success)
                    }
                    Err(e) => {
                        formatter.error(&format!("Failed to decode envelope: {}", e));
                        Ok(ExitCode::InvalidInput)
                    }
                }
            }

            DebugAction::Transcript { compute } => {
                // Requirements: 12.2 - Compute transcript hash
                formatter.progress("Computing transcript hash...");

                match tools.compute_transcript_from_hex(&compute) {
                    Ok(hash) => {
                        let hash_hex = hex::encode(hash);
                        
                        match output {
                            OutputFormat::Json => {
                                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                                    "transcript_hash": hash_hex,
                                    "inputs": compute,
                                    "timestamp": chrono::Utc::now().to_rfc3339()
                                }))?);
                            }
                            OutputFormat::Table => {
                                println!("Transcript Hash: {}", hash_hex);
                            }
                            OutputFormat::Quiet => {
                                println!("{}", hash_hex);
                            }
                        }
                        formatter.success("Transcript computed successfully");
                        Ok(ExitCode::Success)
                    }
                    Err(e) => {
                        formatter.error(&format!("Failed to compute transcript: {}", e));
                        Ok(ExitCode::InvalidInput)
                    }
                }
            }

            DebugAction::Sas { compute } => {
                // Requirements: 12.3 - Compute SAS from transcript
                formatter.progress("Computing SAS code...");

                match tools.compute_sas_from_hex(&compute) {
                    Ok(sas) => {
                        match output {
                            OutputFormat::Json => {
                                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                                    "sas_code": sas,
                                    "transcript_hash": compute,
                                    "timestamp": chrono::Utc::now().to_rfc3339()
                                }))?);
                            }
                            OutputFormat::Table => {
                                println!("\n╔════════════════════════════════════════╗");
                                println!("║     SAS Verification Code              ║");
                                println!("║                                        ║");
                                println!("║           {:^6}                       ║", sas);
                                println!("║                                        ║");
                                println!("╚════════════════════════════════════════╝\n");
                            }
                            OutputFormat::Quiet => {
                                println!("{}", sas);
                            }
                        }
                        formatter.success("SAS computed successfully");
                        Ok(ExitCode::Success)
                    }
                    Err(e) => {
                        formatter.error(&format!("Failed to compute SAS: {}", e));
                        Ok(ExitCode::InvalidInput)
                    }
                }
            }

            DebugAction::Transport { test } => {
                // Requirements: 12.4 - Test transport connectivity
                formatter.progress(&format!("Testing transport: {}...", test));

                match tools.test_transport(&test).await {
                    Ok(result) => {
                        match output {
                            OutputFormat::Json => {
                                println!("{}", serde_json::to_string_pretty(&result)?);
                            }
                            OutputFormat::Table => {
                                use comfy_table::{presets::UTF8_FULL, Table};
                                
                                let mut table = Table::new();
                                table.load_preset(UTF8_FULL);
                                table.set_header(vec!["Property", "Value"]);
                                
                                table.add_row(vec!["URL", &result.url]);
                                table.add_row(vec!["Reachable", if result.reachable { "Yes" } else { "No" }]);
                                
                                if let Some(latency) = result.latency_ms {
                                    table.add_row(vec!["Latency", &format!("{} ms", latency)]);
                                }
                                
                                if let Some(version) = &result.protocol_version {
                                    table.add_row(vec!["Protocol", version]);
                                }
                                
                                if let Some(tls) = &result.tls_info {
                                    table.add_row(vec!["TLS", tls]);
                                }
                                
                                if let Some(error) = &result.error {
                                    table.add_row(vec!["Error", error]);
                                }
                                
                                println!("{}", table);
                            }
                            OutputFormat::Quiet => {
                                if result.reachable {
                                    println!("OK");
                                } else {
                                    println!("FAIL");
                                }
                            }
                        }
                        
                        if result.reachable {
                            formatter.success("Transport test passed");
                            Ok(ExitCode::Success)
                        } else {
                            formatter.error("Transport test failed");
                            Ok(ExitCode::ConnectionFailed)
                        }
                    }
                    Err(e) => {
                        formatter.error(&format!("Transport test error: {}", e));
                        Ok(ExitCode::ConnectionFailed)
                    }
                }
            }

            DebugAction::Capture { output: output_path, duration } => {
                // Requirements: 12.6 - Capture packets to file
                formatter.progress(&format!(
                    "Starting packet capture to {} for {} seconds...",
                    output_path.display(),
                    duration
                ));

                let capture_duration = Duration::from_secs(duration);
                
                match tools.capture_packets(&output_path, capture_duration).await {
                    Ok(stats) => {
                        match output {
                            OutputFormat::Json => {
                                println!("{}", serde_json::to_string_pretty(&stats)?);
                            }
                            OutputFormat::Table => {
                                use comfy_table::{presets::UTF8_FULL, Table};
                                
                                let mut table = Table::new();
                                table.load_preset(UTF8_FULL);
                                table.set_header(vec!["Metric", "Value"]);
                                
                                table.add_row(vec!["Output File", &stats.output_file]);
                                table.add_row(vec!["Duration", &format!("{} seconds", stats.duration_seconds)]);
                                table.add_row(vec!["Packets Captured", &stats.packets_captured.to_string()]);
                                table.add_row(vec!["Bytes Captured", &format!("{} bytes", stats.bytes_captured)]);
                                
                                println!("{}", table);
                            }
                            OutputFormat::Quiet => {}
                        }
                        
                        formatter.success(&format!(
                            "Capture complete: {} packets, {} bytes",
                            stats.packets_captured,
                            stats.bytes_captured
                        ));
                        Ok(ExitCode::Success)
                    }
                    Err(e) => {
                        formatter.error(&format!("Capture failed: {}", e));
                        Ok(ExitCode::GeneralError)
                    }
                }
            }
        }
    }
}

/// Debug subcommands
#[derive(Subcommand, Debug)]
pub enum DebugAction {
    /// Decode and display envelope
    Envelope {
        /// Base64-encoded envelope
        #[arg(long)]
        decode: String,
    },
    /// Compute transcript hash
    Transcript {
        /// Inputs for transcript computation (comma-separated hex)
        #[arg(long)]
        compute: String,
    },
    /// Compute SAS from transcript
    Sas {
        /// Transcript hash (hex)
        #[arg(long)]
        compute: String,
    },
    /// Test transport connectivity
    Transport {
        /// URL to test
        #[arg(long)]
        test: String,
    },
    /// Capture packets to file
    Capture {
        /// Output file path
        #[arg(long)]
        output: PathBuf,
        /// Capture duration in seconds
        #[arg(long, default_value = "10")]
        duration: u64,
    },
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TransportConfig;

    #[test]
    fn test_transport_options_default() {
        let opts = TransportOptions::default();
        
        assert!(opts.preference.is_none());
        assert!(opts.rendezvous_urls.is_empty());
        assert!(opts.relay_urls.is_empty());
        assert!(opts.mesh_nodes.is_empty());
    }

    #[test]
    fn test_transport_options_merge_uses_config_defaults() {
        // When CLI options are empty, config values should be used
        let opts = TransportOptions::default();
        let config = TransportConfig::default();
        
        let resolved = opts.merge_with_config(&config);
        
        assert_eq!(resolved.preference, "auto");
        assert!(!resolved.rendezvous_urls.is_empty());
        assert!(!resolved.relay_urls.is_empty());
    }

    #[test]
    fn test_transport_options_cli_overrides_config() {
        // Requirements: 10.5 - CLI flags override config values
        let opts = TransportOptions {
            preference: Some("mesh".to_string()),
            rendezvous_urls: vec!["https://custom-rendezvous.example.com".to_string()],
            relay_urls: vec!["https://custom-relay.example.com".to_string()],
            mesh_nodes: vec!["mesh.example.com:5000".to_string()],
        };
        
        let config = TransportConfig {
            default: "auto".to_string(),
            rendezvous_urls: vec!["https://default-rendezvous.example.com".to_string()],
            relay_urls: vec!["https://default-relay.example.com".to_string()],
            mesh_nodes: vec![],
            timeout_seconds: 30,
        };
        
        let resolved = opts.merge_with_config(&config);
        
        // CLI values should override config
        assert_eq!(resolved.preference, "mesh");
        assert_eq!(resolved.rendezvous_urls, vec!["https://custom-rendezvous.example.com"]);
        assert_eq!(resolved.relay_urls, vec!["https://custom-relay.example.com"]);
        assert_eq!(resolved.mesh_nodes, vec!["mesh.example.com:5000"]);
        // Timeout comes from config (no CLI override for timeout)
        assert_eq!(resolved.timeout_seconds, 30);
    }

    #[test]
    fn test_transport_options_partial_override() {
        // Only some CLI options specified, others use config defaults
        let opts = TransportOptions {
            preference: Some("rendezvous".to_string()),
            rendezvous_urls: vec![], // Empty - use config
            relay_urls: vec!["https://custom-relay.example.com".to_string()],
            mesh_nodes: vec![], // Empty - use config
        };
        
        let config = TransportConfig {
            default: "auto".to_string(),
            rendezvous_urls: vec!["https://config-rendezvous.example.com".to_string()],
            relay_urls: vec!["https://config-relay.example.com".to_string()],
            mesh_nodes: vec!["config-mesh.example.com:5000".to_string()],
            timeout_seconds: 60,
        };
        
        let resolved = opts.merge_with_config(&config);
        
        // CLI preference overrides
        assert_eq!(resolved.preference, "rendezvous");
        // Rendezvous uses config (CLI empty)
        assert_eq!(resolved.rendezvous_urls, vec!["https://config-rendezvous.example.com"]);
        // Relay uses CLI override
        assert_eq!(resolved.relay_urls, vec!["https://custom-relay.example.com"]);
        // Mesh uses config (CLI empty)
        assert_eq!(resolved.mesh_nodes, vec!["config-mesh.example.com:5000"]);
    }

    #[test]
    fn test_resolved_transport_fields() {
        let resolved = ResolvedTransport {
            preference: "direct".to_string(),
            rendezvous_urls: vec!["https://r1.example.com".to_string(), "https://r2.example.com".to_string()],
            relay_urls: vec!["https://relay.example.com".to_string()],
            mesh_nodes: vec!["mesh1.example.com:5000".to_string()],
            timeout_seconds: 45,
        };
        
        assert_eq!(resolved.preference, "direct");
        assert_eq!(resolved.rendezvous_urls.len(), 2);
        assert_eq!(resolved.relay_urls.len(), 1);
        assert_eq!(resolved.mesh_nodes.len(), 1);
        assert_eq!(resolved.timeout_seconds, 45);
    }

    #[test]
    fn test_cli_parse_transport_flags() {
        use clap::Parser;
        
        // Test parsing with transport flags
        let args = vec![
            "zrc-controller",
            "--transport", "mesh",
            "--rendezvous-url", "https://r1.example.com",
            "--rendezvous-url", "https://r2.example.com",
            "--relay-url", "https://relay.example.com",
            "--mesh-node", "mesh.example.com:5000",
            "identity", "show",
        ];
        
        let cli = Cli::try_parse_from(args).unwrap();
        
        assert_eq!(cli.transport, Some("mesh".to_string()));
        assert_eq!(cli.rendezvous_urls.len(), 2);
        assert_eq!(cli.relay_urls.len(), 1);
        assert_eq!(cli.mesh_nodes.len(), 1);
    }

    #[test]
    fn test_cli_parse_without_transport_flags() {
        use clap::Parser;
        
        // Test parsing without transport flags (should use defaults)
        let args = vec![
            "zrc-controller",
            "identity", "show",
        ];
        
        let cli = Cli::try_parse_from(args).unwrap();
        
        assert!(cli.transport.is_none());
        assert!(cli.rendezvous_urls.is_empty());
        assert!(cli.relay_urls.is_empty());
        assert!(cli.mesh_nodes.is_empty());
    }
}
