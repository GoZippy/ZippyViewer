# coturn Configuration Guide

Complete reference for all coturn configuration options.

## Configuration Files

### turnserver.conf

Main configuration file for coturn. Located at `./turnserver.conf`.

### Environment Variables (.env)

Environment variables are used to configure coturn via `turnserver.conf.template`.

## Listening Configuration

### listening-ip

IP address to listen on.

- **Default**: `0.0.0.0` (all interfaces)
- **Example**: `listening-ip=0.0.0.0`
- **Environment**: `LISTENING_IP`

### listening-port

Port for STUN/TURN (UDP and TCP).

- **Default**: `3478`
- **Example**: `listening-port=3478`
- **Environment**: `LISTENING_PORT`

### tls-listening-port

Port for TURNS (TLS).

- **Default**: `5349`
- **Example**: `tls-listening-port=5349`
- **Environment**: `TLS_LISTENING_PORT`

### listening-ipv6

IPv6 listening address (optional).

- **Example**: `listening-ipv6=::`

## External IP Configuration

### external-ip

Public IP address for NAT scenarios. **REQUIRED** for TURN to work behind NAT.

- **Required**: Yes (for NAT scenarios)
- **Example**: `external-ip=203.0.113.1`
- **Environment**: `EXTERNAL_IP`

## Realm and Authentication

### realm

Authentication realm (domain).

- **Default**: `zrc.local`
- **Example**: `realm=zrc.local`
- **Environment**: `REALM`

### static-auth-secret

Shared secret for static authentication.

- **Required**: Yes
- **Generate**: `openssl rand -hex 32`
- **Example**: `static-auth-secret=your-secret-key`
- **Environment**: `TURN_SECRET`

### use-auth-secret

Enable static-auth-secret authentication.

- **Default**: Enabled
- **Value**: `use-auth-secret`

## TLS Configuration

### cert

Path to TLS certificate file.

- **Default**: `/etc/coturn/certs/cert.pem`
- **Example**: `cert=/etc/coturn/certs/cert.pem`

### pkey

Path to TLS private key file.

- **Default**: `/etc/coturn/certs/key.pem`
- **Example**: `pkey=/etc/coturn/certs/key.pem`

## Relay Configuration

### relay-ip

IP address range for relay allocations.

- **Default**: `0.0.0.0` (all interfaces)
- **Example**: `relay-ip=0.0.0.0`

### min-port

Minimum port for relay allocations.

- **Default**: `49152`
- **Example**: `min-port=49152`

### max-port

Maximum port for relay allocations.

- **Default**: `65535`
- **Example**: `max-port=65535`

## Logging Configuration

### log-file

Path to log file.

- **Default**: `/var/log/coturn/turn.log`
- **Example**: `log-file=/var/log/coturn/turn.log`

### verbose

Enable verbose logging.

- **Default**: Enabled
- **Value**: `verbose`

### no-stdout-log

Disable logging to stdout.

- **Default**: Enabled
- **Value**: `no-stdout-log`

## Security Settings

### no-cli

Disable command-line interface.

- **Default**: Enabled
- **Value**: `no-cli`

### no-loopback-peers

Disable loopback peers (security best practice).

- **Default**: Enabled
- **Value**: `no-loopback-peers`

### no-multicast-peers

Disable multicast peers.

- **Default**: Enabled
- **Value**: `no-multicast-peers`

## Resource Limits

### max-bps

Maximum bandwidth per user (bits per second).

- **Default**: `10000000` (10 Mbps)
- **Example**: `max-bps=10000000`
- **Environment**: `MAX_BPS`

### max-allocate-lifetime

Maximum allocation lifetime (seconds).

- **Default**: `28800` (8 hours)
- **Example**: `max-allocate-lifetime=28800`

### max-allocate-timeout

Maximum allocation timeout (seconds). Allocation removed if idle.

- **Default**: `30`
- **Example**: `max-allocate-timeout=30`

### user-quota

Maximum allocations per user.

- **Default**: `12`
- **Example**: `user-quota=12`
- **Environment**: `USER_QUOTA`

### total-quota

Maximum total allocations.

- **Default**: `1000`
- **Example**: `total-quota=1000`
- **Environment**: `MAX_ALLOCATIONS`

## Performance Tuning

### fingerprint

Enable STUN/TURN fingerprint (protocol compatibility).

- **Default**: Enabled
- **Value**: `fingerprint`

### lt-cred-mech

Use long-term credentials mechanism.

- **Default**: Enabled
- **Value**: `lt-cred-mech`

## IP Access Control

### allowed-peer-ip

Allow connections from specific IP addresses/ranges.

- **Example**: `allowed-peer-ip=192.168.1.0/24`
- **Example**: `allowed-peer-ip=10.0.0.1`

### denied-peer-ip

Deny connections from specific IP addresses/ranges.

- **Example**: `denied-peer-ip=192.168.1.100`
- **Example**: `denied-peer-ip=10.0.0.0/24`

### max-connections-per-ip

Limit connections per IP address.

- **Example**: `max-connections-per-ip=10`

## Rate Limiting

Rate limiting can be configured via:
- `max-connections-per-ip`: Limit connections per IP
- `max-bps`: Limit bandwidth per user
- `user-quota`: Limit allocations per user
- `total-quota`: Limit total allocations

## IPv6 Support

To enable IPv6:

```conf
listening-ipv6=::
relay-ipv6=::
```

## Configuration Examples

### Minimal Configuration

```conf
listening-ip=0.0.0.0
listening-port=3478
external-ip=203.0.113.1
realm=zrc.local
static-auth-secret=your-secret
use-auth-secret
```

### Production Configuration

See `turnserver.conf` for a complete production-ready configuration.

### High Traffic Configuration

```conf
max-bps=50000000  # 50 Mbps per user
total-quota=5000  # 5000 total allocations
user-quota=50     # 50 allocations per user
```

## Updating Configuration

1. Edit `.env` file with new values
2. Run `bash scripts/update.sh` to regenerate `turnserver.conf`
3. Or manually edit `turnserver.conf` and restart: `docker-compose restart coturn`

## Validation

Validate configuration:

```bash
# Validate certificates
bash scripts/validate-certificates.sh

# Validate network
bash scripts/validate-network.sh <EXTERNAL_IP>

# Test connectivity
bash scripts/test-stun.sh
```

## Environment Variable Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `EXTERNAL_IP` | (required) | Public IP address |
| `TURN_SECRET` | (required) | Static auth secret |
| `REALM` | `zrc.local` | Authentication realm |
| `LISTENING_IP` | `0.0.0.0` | Listening IP |
| `LISTENING_PORT` | `3478` | STUN/TURN port |
| `TLS_LISTENING_PORT` | `5349` | TLS TURN port |
| `MAX_BPS` | `10000000` | Max bandwidth per user |
| `MAX_ALLOCATIONS` | `1000` | Max total allocations |
| `USER_QUOTA` | `12` | Allocations per user |

## Additional Resources

- [coturn Documentation](https://github.com/coturn/coturn/wiki)
- [TURN Protocol RFC](https://tools.ietf.org/html/rfc5766)
- [STUN Protocol RFC](https://tools.ietf.org/html/rfc5389)
