# Configuration

This guide covers configuration options for ZRC components.

## Agent Configuration

The agent can be configured via command line, environment variables, or config file.

### Config File Location

- **Windows**: `%APPDATA%\ZippyRemote\agent.toml`
- **macOS**: `~/Library/Application Support/ZippyRemote/agent.toml`
- **Linux**: `~/.config/zippyremote/agent.toml`

### Example Agent Config

```toml
[general]
device_name = "My Workstation"
log_level = "info"

[network]
rendezvous_url = "https://rendezvous.example.com"
stun_servers = ["stun:stun.l.google.com:19302"]

[permissions]
allow_unattended = false
require_consent = true
allowed_capabilities = ["view", "input", "clipboard"]

[security]
auto_lock_on_disconnect = true
consent_timeout_seconds = 30
```

## Relay Server Configuration

```toml
[server]
listen_addr = "0.0.0.0"
listen_port = 4433
max_allocations = 10000

[tls]
cert_path = "/etc/zrc/cert.pem"
key_path = "/etc/zrc/key.pem"

[limits]
default_bandwidth_bytes = 10485760  # 10 MB/s
default_quota_bytes = 1073741824    # 1 GB per session
allocation_ttl_seconds = 3600

[security]
rate_limit_per_ip = 10
require_valid_token = true
```

## Rendezvous Server Configuration

```toml
[server]
listen_addr = "0.0.0.0"
listen_port = 8443
mailbox_ttl_seconds = 300

[database]
url = "sqlite:///var/lib/zrc/rendezvous.db"

[limits]
max_mailboxes_per_ip = 100
max_message_size_bytes = 65536
```

## Environment Variables

All config options can be overridden via environment variables:

```bash
ZRC_LOG_LEVEL=debug
ZRC_RENDEZVOUS_URL=https://my-server.example.com
ZRC_ALLOW_UNATTENDED=false
```

## Command Line Options

```bash
# Agent
zrc-agent --foreground --log-level debug --config /path/to/agent.toml

# Relay
zrc-relay --config /etc/zrc/relay.toml --port 4433

# Rendezvous
zrc-rendezvous --config /etc/zrc/rendezvous.toml
```
