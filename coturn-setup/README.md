# coturn Setup for ZRC

Deployment and configuration for coturn (CoTURN), a TURN/STUN server used as a fallback relay for WebRTC connections in the ZRC system. This setup enables NAT traversal when direct peer-to-peer connections fail.

## Quick Start

1. **Clone and navigate to the coturn-setup directory:**
   ```bash
   cd coturn-setup
   ```

2. **Run the setup script:**
   ```bash
   bash scripts/setup.sh
   ```
   This will:
   - Create `.env` file from `env.example`
   - Validate configuration
   - Check certificates
   - Validate network settings

3. **Edit `.env` file:**
   ```bash
   # Set your public IP address
   EXTERNAL_IP=your.public.ip.address
   
   # Generate a strong secret
   TURN_SECRET=$(openssl rand -hex 32)
   ```

4. **Set up TLS certificates:**
   
   For development/testing (self-signed):
   ```bash
   bash scripts/setup-selfsigned.sh turn.local
   ```
   
   For production (Let's Encrypt):
   ```bash
   bash scripts/setup-letsencrypt.sh turn.example.com admin@example.com
   ```

5. **Start coturn:**
   ```bash
   bash scripts/start.sh
   ```

6. **Verify it's working:**
   ```bash
   bash scripts/test-stun.sh
   ```

## Directory Structure

```
coturn-setup/
├── docker-compose.yml          # Docker Compose configuration
├── turnserver.conf             # coturn configuration file
├── turnserver.conf.template    # Configuration template
├── env.example                 # Environment variables example
├── ip-access-control.conf.example  # IP access control example
├── README.md                   # This file
├── CONFIGURATION.md            # Configuration options documentation
├── TROUBLESHOOTING.md          # Troubleshooting guide
├── SECURITY.md                 # Security best practices
├── HIGH_AVAILABILITY.md        # High availability setup
├── certs/                      # TLS certificates directory
├── logs/                       # Log files directory
├── scripts/                    # Utility scripts
│   ├── setup.sh               # Initial setup
│   ├── start.sh               # Start service
│   ├── stop.sh                # Stop service
│   ├── update.sh              # Update configuration
│   ├── setup-letsencrypt.sh   # Let's Encrypt setup
│   ├── setup-selfsigned.sh    # Self-signed cert setup
│   ├── validate-certificates.sh
│   ├── validate-network.sh
│   ├── generate-credentials.sh
│   ├── monitor-logs.sh
│   ├── monitor-limits.sh
│   └── test-*.sh              # Test scripts
└── logrotate/                 # Log rotation configuration
    └── coturn
```

## Configuration

### Environment Variables

Copy `env.example` to `.env` and configure:

- **EXTERNAL_IP** (required): Your public IP address for NAT scenarios
- **TURN_SECRET** (required): Static authentication secret (generate with `openssl rand -hex 32`)
- **REALM**: Authentication realm (default: `zrc.local`)
- **LISTENING_PORT**: STUN/TURN port (default: `3478`)
- **TLS_LISTENING_PORT**: TLS TURN port (default: `5349`)
- **MAX_BPS**: Maximum bandwidth per user (default: `10000000` = 10 Mbps)
- **MAX_ALLOCATIONS**: Maximum total allocations (default: `1000`)
- **USER_QUOTA**: Allocations per user (default: `12`)

See `CONFIGURATION.md` for detailed configuration options.

## TLS Certificates

### Self-Signed (Development/Testing)

```bash
bash scripts/setup-selfsigned.sh turn.local
```

### Let's Encrypt (Production)

```bash
bash scripts/setup-letsencrypt.sh turn.example.com admin@example.com
```

Certificates will be placed in `certs/` directory:
- `certs/cert.pem` - Certificate
- `certs/key.pem` - Private key

Validate certificates:
```bash
bash scripts/validate-certificates.sh
```

## Authentication

coturn uses static-auth-secret authentication. Credentials are generated using:

```
username = timestamp:username
password = HMAC-SHA1(secret, username)
```

Generate credentials for ZRC clients:
```bash
bash scripts/generate-credentials.sh <TURN_SECRET> [username] [ttl-seconds]
```

## Network Configuration

### Firewall Requirements

Open the following ports:
- **UDP 3478**: STUN/TURN
- **TCP 3478**: TURN
- **TCP 5349**: TURNS (TLS)

### External IP

The `EXTERNAL_IP` must be set to your public IP address. This is critical for NAT traversal.

Validate network configuration:
```bash
bash scripts/validate-network.sh <EXTERNAL_IP>
```

## Monitoring

### View Logs

```bash
# Docker logs
docker-compose logs -f coturn

# Log file
tail -f logs/turn.log

# Monitor with script
bash scripts/monitor-logs.sh
```

### Monitor Resource Limits

```bash
bash scripts/monitor-limits.sh
```

### Log Rotation

Install logrotate configuration:
```bash
sudo cp logrotate/coturn /etc/logrotate.d/
```

## Testing

### STUN Test
```bash
bash scripts/test-stun.sh [host] [port]
```

### TURN Allocation Test
```bash
bash scripts/test-turn.sh <host> <port> <username> <password>
```

### TLS/TURNS Test
```bash
bash scripts/test-tls.sh [host] [port]
```

### Authentication Test
```bash
bash scripts/test-auth.sh <host> <port> <secret>
```

### Resource Limits Test
```bash
bash scripts/test-resource-limits.sh <host> <port> <secret>
```

### ZRC Integration Test
```bash
bash scripts/test-zrc-integration.sh <host> <stun-port> <turns-port> <secret>
```

## ZRC Client Integration

Configure ZRC clients with:

```toml
[turn]
turn_uri = "turn:turn.example.com:3478"
turns_uri = "turns:turn.example.com:5349"
username = "<generated-username>"
password = "<generated-password>"
realm = "zrc.local"
```

Generate credentials:
```bash
bash scripts/generate-credentials.sh <TURN_SECRET> zrc 3600
```

## Deployment Scripts

- **setup.sh**: Initial deployment and configuration validation
- **start.sh**: Start coturn service
- **stop.sh**: Stop coturn service
- **update.sh**: Update configuration and restart

## Security

See `SECURITY.md` for security best practices, including:
- TLS configuration
- Secret management
- Access control
- Rate limiting
- DDoS mitigation

## High Availability

See `HIGH_AVAILABILITY.md` for:
- Load balancer configuration
- Health checks
- Failover procedures
- Geographic distribution

## Troubleshooting

See `TROUBLESHOOTING.md` for common issues and solutions.

Common issues:
- **Allocation fails**: Check `EXTERNAL_IP` configuration
- **TLS errors**: Verify certificate paths and permissions
- **Authentication fails**: Verify `TURN_SECRET` matches
- **Port conflicts**: Ensure ports 3478 and 5349 are available

## Performance Tuning

- **Bandwidth limits**: Adjust `MAX_BPS` based on server capacity
- **Concurrent allocations**: Set `MAX_ALLOCATIONS` based on resources
- **Port range**: Configure `min-port`/`max-port` in `turnserver.conf`
- **Network mode**: Using `network_mode: host` for best performance

## Documentation

- [Configuration Guide](CONFIGURATION.md) - All configuration options
- [Troubleshooting Guide](TROUBLESHOOTING.md) - Common issues and solutions
- [Security Guide](SECURITY.md) - Security best practices
- [High Availability Guide](HIGH_AVAILABILITY.md) - HA setup and configuration

## Support

For issues related to:
- **coturn itself**: See [coturn documentation](https://github.com/coturn/coturn)
- **ZRC integration**: See ZRC project documentation
- **This setup**: Check troubleshooting guide or open an issue

## License

This setup configuration is part of the ZRC project.
