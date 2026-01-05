# zrc-rendezvous Deployment Guide

## Static Binary Build

Build a static binary for deployment:

```bash
# Linux (musl for static linking)
cargo build --release --target x86_64-unknown-linux-musl

# Or use the default target
cargo build --release
```

The binary will be at: `target/release/zrc-rendezvous`

## Configuration

1. Copy `examples/config.toml` to your deployment location
2. Customize the configuration as needed
3. Set `ZRC_CONFIG_PATH` environment variable or use command-line args

## systemd Service

1. Copy `zrc-rendezvous.service` to `/etc/systemd/system/`
2. Create user and directory:
   ```bash
   sudo useradd -r -s /bin/false zrc-rendezvous
   sudo mkdir -p /opt/zrc-rendezvous
   sudo cp target/release/zrc-rendezvous /opt/zrc-rendezvous/
   sudo chown -R zrc-rendezvous:zrc-rendezvous /opt/zrc-rendezvous
   ```
3. Enable and start:
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable zrc-rendezvous
   sudo systemctl start zrc-rendezvous
   ```

## Environment Variables

- `ZRC_BIND_ADDR` - Server bind address (default: 0.0.0.0:8080)
- `ZRC_CONFIG_PATH` - Path to TOML config file
- `ZRC_MAX_MESSAGE_SIZE` - Max message size in bytes
- `ZRC_MAX_QUEUE_LENGTH` - Max messages per mailbox
- `ZRC_MESSAGE_TTL_SECS` - Message TTL in seconds
- `ZRC_AUTH_MODE` - Authentication mode (disabled/server_wide/per_mailbox)
- `ZRC_SERVER_TOKENS` - Comma-separated list of server tokens
- `RUST_LOG` - Logging level (default: info)

## TLS/HTTPS

For production deployments, use a reverse proxy (nginx, Caddy, Traefik) for TLS termination.
The server supports TLS configuration but full integration is pending.

## Monitoring

- Health check: `GET /health`
- Metrics: `GET /metrics` (Prometheus format)
