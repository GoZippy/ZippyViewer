# Self-Hosting Guide

Run your own ZRC infrastructure for complete control over your remote access platform.

## Components to Host

| Component | Required | Purpose |
|-----------|----------|---------|
| Rendezvous Server | Yes | Session signaling |
| Relay Server | Recommended | NAT traversal fallback |
| Directory Node | Optional | Device discovery |
| TURN Server | Optional | Additional NAT traversal |

## Quick Setup with Docker

### Rendezvous Server

```bash
docker run -d \
  --name zrc-rendezvous \
  -p 8443:8443 \
  -v /data/zrc:/data \
  ghcr.io/gozippy/zrc-rendezvous:latest
```

### Relay Server

```bash
docker run -d \
  --name zrc-relay \
  -p 4433:4433/udp \
  -v /etc/zrc/certs:/certs \
  ghcr.io/gozippy/zrc-relay:latest
```

## Manual Installation

### 1. Build from Source

```bash
cd zippy-remote
cargo build --release --bin zrc-rendezvous --bin zrc-relay
```

### 2. Create Config Files

See [Configuration](configuration.md) for detailed options.

### 3. Set Up TLS Certificates

```bash
# Using Let's Encrypt
certbot certonly --standalone -d relay.example.com

# Or self-signed for testing
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
```

### 4. Create Systemd Services

```ini
# /etc/systemd/system/zrc-relay.service
[Unit]
Description=ZRC Relay Server
After=network.target

[Service]
Type=simple
User=zrc
ExecStart=/usr/local/bin/zrc-relay --config /etc/zrc/relay.toml
Restart=always

[Install]
WantedBy=multi-user.target
```

## TURN/STUN with Coturn

For additional NAT traversal support, see the [coturn-setup](../coturn-setup/README.md) directory.

## Firewall Rules

```bash
# Rendezvous (HTTPS)
ufw allow 8443/tcp

# Relay (QUIC)
ufw allow 4433/udp

# TURN (if using Coturn)
ufw allow 3478/tcp
ufw allow 3478/udp
ufw allow 5349/tcp
```

## High Availability

For production deployments:

1. Run multiple rendezvous instances behind a load balancer
2. Use shared database (PostgreSQL) for state
3. Deploy relay servers geographically distributed
4. Monitor with Prometheus metrics endpoints

## Client Configuration

Point clients to your self-hosted infrastructure:

```toml
[network]
rendezvous_url = "https://rendezvous.yourcompany.com"
relay_urls = ["relay.yourcompany.com:4433"]
```
