# Design Document: coturn-setup

## Overview

coturn-setup provides deployment configuration and documentation for coturn (CoTURN), a TURN/STUN server used as a fallback relay for WebRTC connections in the ZRC system. This setup enables NAT traversal when direct peer-to-peer connections fail.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      coturn Deployment                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Docker Container                         │   │
│  │  ┌──────────────────────────────────────────────────────┐  │   │
│  │  │              coturn Server                            │  │   │
│  │  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │  │   │
│  │  │  │  STUN        │  │  TURN/UDP    │  │  TURN/TLS    │ │  │   │
│  │  │  │  Port 3478   │  │  Port 3478   │  │  Port 5349   │ │  │   │
│  │  │  └──────────────┘  └──────────────┘  └──────────────┘ │  │   │
│  │  └──────────────────────────────────────────────────────┘  │  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│                              ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                  Configuration Files                         │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │   │
│  │  │turnserver.conf│  │docker-compose│  │  TLS Certs   │    │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘    │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Components

### Docker Compose Configuration

```yaml
version: '3.8'

services:
  coturn:
    image: coturn/coturn:latest
    container_name: zrc-coturn
    restart: unless-stopped
    network_mode: host  # Required for TURN to work correctly
    volumes:
      - ./turnserver.conf:/etc/turnserver.conf:ro
      - ./certs:/etc/coturn/certs:ro
      - ./logs:/var/log/coturn
    environment:
      - TURN_USERNAME=${TURN_USERNAME}
      - TURN_PASSWORD=${TURN_PASSWORD}
      - EXTERNAL_IP=${EXTERNAL_IP}
    command: ["-c", "/etc/turnserver.conf"]
    healthcheck:
      test: ["CMD", "turnutils_stunclient", "127.0.0.1"]
      interval: 30s
      timeout: 10s
      retries: 3
```

### Configuration Template (turnserver.conf)

```conf
# Listening addresses
listening-ip=0.0.0.0
listening-port=3478
tls-listening-port=5349

# External IP (required for NAT)
external-ip=${EXTERNAL_IP}

# Realm
realm=zrc.local

# Authentication
static-auth-secret=${TURN_SECRET}
use-auth-secret

# TLS Configuration
cert=/etc/coturn/certs/cert.pem
pkey=/etc/coturn/certs/key.pem

# Relay IP range
relay-ip=0.0.0.0

# Logging
log-file=/var/log/coturn/turn.log
verbose
no-stdout-log

# Security
no-cli
no-loopback-peers
no-multicast-peers

# Resource limits
max-bps=10000000  # 10 Mbps per user
max-allocate-lifetime=28800  # 8 hours
max-allocate-timeout=30  # 30 seconds idle

# User quota
user-quota=12  # 12 allocations per user
total-quota=1000  # 1000 total allocations

# Performance
min-port=49152
max-port=65535
```

### Environment Variables

```bash
# External IP address (required for NAT)
EXTERNAL_IP=203.0.113.1

# Authentication
TURN_USERNAME=zrc
TURN_PASSWORD=secure-password-here
TURN_SECRET=shared-secret-for-static-auth

# Network
LISTENING_IP=0.0.0.0
LISTENING_PORT=3478
TLS_LISTENING_PORT=5349

# Realm
REALM=zrc.local

# Resource limits
MAX_BPS=10000000
MAX_ALLOCATIONS=1000
```

### TLS Certificate Setup

```bash
# Option 1: Let's Encrypt (recommended for production)
certbot certonly --standalone -d turn.example.com
cp /etc/letsencrypt/live/turn.example.com/fullchain.pem certs/cert.pem
cp /etc/letsencrypt/live/turn.example.com/privkey.pem certs/key.pem

# Option 2: Self-signed (for testing)
openssl req -x509 -newkey rsa:2048 -keyout certs/key.pem -out certs/cert.pem -days 365 -nodes
```

## Deployment Scenarios

### Scenario 1: Single Server Deployment

```
Internet
    │
    ▼
┌─────────────┐
│   Firewall  │ (Ports: 3478/UDP, 3478/TCP, 5349/TLS)
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   coturn    │
│  Container  │
└─────────────┘
```

### Scenario 2: Load Balanced Deployment

```
Internet
    │
    ▼
┌─────────────┐
│ Load Balancer│
└──────┬──────┘
       │
   ┌───┴───┐
   │       │
   ▼       ▼
┌─────┐ ┌─────┐
│coturn│ │coturn│
│  1   │ │  2   │
└─────┘ └─────┘
```

## Integration with ZRC

### Client Configuration

ZRC clients need to be configured with:
- TURN server URL: `turn:turn.example.com:3478`
- TLS TURN URL: `turns:turn.example.com:5349`
- Username: Generated from static-auth-secret
- Password: Generated from static-auth-secret
- Realm: `zrc.local`

### Credential Generation

For static-auth-secret authentication, credentials are generated using:
```
username = timestamp + ":" + username
password = HMAC-SHA1(secret, username)
```

## Security Considerations

1. **TLS Required**: Use TLS (TURNS) for production deployments
2. **Secret Management**: Store TURN_SECRET securely (secrets manager, not in git)
3. **Network Isolation**: Run coturn in isolated network when possible
4. **Rate Limiting**: Configure bandwidth and allocation limits
5. **Access Control**: Use IP allowlisting for known clients
6. **Logging**: Monitor for abuse patterns

## Monitoring

### Health Checks

```bash
# STUN test
turnutils_stunclient turn.example.com

# TURN allocation test
turnutils_peer -L turn.example.com -U username -P password
```

### Log Monitoring

Monitor `/var/log/coturn/turn.log` for:
- Authentication failures
- Allocation events
- Bandwidth usage
- Error messages

## Performance Tuning

1. **Port Range**: Configure min-port/max-port for relay allocations
2. **Bandwidth**: Adjust max-bps based on server capacity
3. **Concurrent Allocations**: Set total-quota based on server resources
4. **Network**: Use network_mode: host for best performance
5. **CPU**: Allocate sufficient CPU for encryption/decryption

## Troubleshooting

### Common Issues

1. **Allocation Fails**: Check external-ip configuration
2. **TLS Errors**: Verify certificate paths and permissions
3. **Authentication Fails**: Verify static-auth-secret matches
4. **High Latency**: Check network path and bandwidth limits
5. **Port Conflicts**: Ensure ports 3478 and 5349 are available

### Diagnostic Commands

```bash
# Check if coturn is listening
netstat -tuln | grep -E '3478|5349'

# Test STUN
turnutils_stunclient localhost

# View logs
tail -f /var/log/coturn/turn.log

# Check container status
docker ps | grep coturn
docker logs zrc-coturn
```
