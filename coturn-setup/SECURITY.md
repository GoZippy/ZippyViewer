# coturn Security Guide

Security best practices and hardening for coturn deployment.

## Table of Contents

- [TLS Configuration](#tls-configuration)
- [Secret Management](#secret-management)
- [Access Control](#access-control)
- [Rate Limiting](#rate-limiting)
- [Network Security](#network-security)
- [DDoS Mitigation](#ddos-mitigation)
- [Running as Non-Root](#running-as-non-root)
- [Security Checklist](#security-checklist)

## TLS Configuration

### Use TLS in Production

Always use TLS (TURNS) for production deployments:

```conf
# In turnserver.conf
tls-listening-port=5349
cert=/etc/coturn/certs/cert.pem
pkey=/etc/coturn/certs/key.pem
```

### Certificate Best Practices

1. **Use Let's Encrypt for production:**
   ```bash
   bash scripts/setup-letsencrypt.sh turn.example.com admin@example.com
   ```

2. **Set up automatic renewal:**
   ```bash
   # Add to crontab
   0 0 * * * certbot renew --quiet --deploy-hook 'cd /path/to/coturn-setup && docker-compose restart coturn'
   ```

3. **Validate certificates regularly:**
   ```bash
   bash scripts/validate-certificates.sh
   ```

4. **Set proper permissions:**
   ```bash
   chmod 644 certs/cert.pem
   chmod 600 certs/key.pem
   ```

## Secret Management

### Generate Strong Secrets

```bash
# Generate a strong secret
openssl rand -hex 32

# Store in .env (never commit to git)
TURN_SECRET=<generated-secret>
```

### Secret Storage

1. **Never commit secrets to git:**
   ```bash
   # .env should be in .gitignore
   echo ".env" >> .gitignore
   ```

2. **Use secrets manager in production:**
   - AWS Secrets Manager
   - HashiCorp Vault
   - Kubernetes Secrets
   - Docker Secrets

3. **Rotate secrets regularly:**
   ```bash
   # Update TURN_SECRET in .env
   # Regenerate turnserver.conf
   bash scripts/update.sh
   ```

### Credential Generation

Credentials are time-limited and generated server-side:

```bash
# Generate credentials for clients
bash scripts/generate-credentials.sh <TURN_SECRET> <username> <ttl>
```

## Access Control

### IP Allowlisting

Restrict access to known IP addresses:

```conf
# In turnserver.conf
allowed-peer-ip=192.168.1.0/24
allowed-peer-ip=10.0.0.0/8
```

See `ip-access-control.conf.example` for examples.

### IP Blocklisting

Block specific IP addresses:

```conf
# In turnserver.conf
denied-peer-ip=192.168.1.100
denied-peer-ip=10.0.0.0/24
```

### Connection Limits

Limit connections per IP:

```conf
# In turnserver.conf
max-connections-per-ip=10
```

## Rate Limiting

### Bandwidth Limits

Limit bandwidth per user:

```conf
# In turnserver.conf
max-bps=10000000  # 10 Mbps per user
```

### Allocation Limits

Limit allocations:

```conf
# In turnserver.conf
user-quota=12          # Per user
total-quota=1000       # Total
```

### Monitor Limits

```bash
bash scripts/monitor-limits.sh
```

## Network Security

### Firewall Configuration

Only open required ports:

```bash
# UDP 3478: STUN/TURN
# TCP 3478: TURN
# TCP 5349: TURNS (TLS)
sudo ufw allow 3478/udp
sudo ufw allow 3478/tcp
sudo ufw allow 5349/tcp
```

### Network Isolation

1. **Use isolated Docker network (if not using host mode):**
   ```yaml
   # docker-compose.yml
   networks:
     coturn-net:
       driver: bridge
   ```

2. **Restrict container access:**
   ```yaml
   # docker-compose.yml
   cap_drop:
     - ALL
   cap_add:
     - NET_BIND_SERVICE
   ```

### Disable Unnecessary Features

```conf
# In turnserver.conf
no-cli              # Disable CLI
no-loopback-peers   # Disable loopback
no-multicast-peers  # Disable multicast
```

## DDoS Mitigation

### Resource Limits

Set appropriate limits to prevent abuse:

```conf
# In turnserver.conf
max-bps=10000000
user-quota=12
total-quota=1000
max-connections-per-ip=10
```

### Rate Limiting

Use external rate limiting:
- nginx rate limiting
- Cloudflare
- AWS WAF
- iptables rate limiting

### Monitoring

Monitor for abuse patterns:

```bash
# Monitor logs
bash scripts/monitor-logs.sh

# Check for unusual activity
docker-compose logs coturn | grep -i "error\|fail\|denied"
```

### IP Reputation

Block known malicious IPs:

```conf
# In turnserver.conf
denied-peer-ip=<malicious-ip>
```

## Running as Non-Root

### Docker Configuration

coturn runs as non-root user in Docker:

```yaml
# docker-compose.yml
user: "65534:65534"  # nobody user
cap_drop:
  - ALL
cap_add:
  - NET_BIND_SERVICE
```

### File Permissions

Ensure proper file permissions:

```bash
# Logs directory
chown -R 65534:65534 logs/
chmod 755 logs/

# Certificates
chmod 644 certs/cert.pem
chmod 600 certs/key.pem
```

## Security Checklist

### Pre-Deployment

- [ ] Strong `TURN_SECRET` generated and stored securely
- [ ] TLS certificates configured (Let's Encrypt for production)
- [ ] `EXTERNAL_IP` set correctly
- [ ] Firewall rules configured
- [ ] IP access control configured (if needed)
- [ ] Resource limits set appropriately
- [ ] Security settings enabled (`no-cli`, `no-loopback-peers`)
- [ ] Running as non-root user
- [ ] Logging configured
- [ ] `.env` file not committed to git

### Ongoing Maintenance

- [ ] Monitor logs for errors and abuse
- [ ] Validate certificates regularly
- [ ] Rotate secrets periodically
- [ ] Update coturn image regularly
- [ ] Review resource usage
- [ ] Check for security advisories
- [ ] Test failover procedures (if HA)
- [ ] Backup configuration

### Monitoring

- [ ] Set up log monitoring
- [ ] Configure alerts for errors
- [ ] Monitor resource limits
- [ ] Track authentication failures
- [ ] Monitor bandwidth usage

## Security Best Practices Summary

1. **Always use TLS in production**
2. **Generate and store secrets securely**
3. **Set appropriate resource limits**
4. **Enable security settings** (`no-cli`, `no-loopback-peers`)
5. **Configure IP access control** when possible
6. **Monitor logs** for abuse
7. **Run as non-root** user
8. **Keep coturn updated**
9. **Use firewall rules** to restrict access
10. **Regular security audits**

## Additional Resources

- [coturn Security](https://github.com/coturn/coturn/wiki/Security)
- [OWASP WebRTC Security](https://owasp.org/www-community/vulnerabilities/WebRTC_security)
- [TURN Security Considerations](https://tools.ietf.org/html/rfc5766#section-17)

## Incident Response

If a security incident occurs:

1. **Immediately rotate secrets:**
   ```bash
   # Generate new secret
   openssl rand -hex 32
   # Update .env and restart
   bash scripts/update.sh
   ```

2. **Review logs:**
   ```bash
   docker-compose logs coturn | grep -i "error\|fail\|unauthorized"
   ```

3. **Block malicious IPs:**
   ```conf
   # Add to turnserver.conf
   denied-peer-ip=<malicious-ip>
   ```

4. **Notify affected users** if credentials are compromised

5. **Document the incident** and update security procedures
