# coturn Troubleshooting Guide

Common issues and solutions for coturn deployment.

## Table of Contents

- [Allocation Fails](#allocation-fails)
- [TLS Errors](#tls-errors)
- [Authentication Fails](#authentication-fails)
- [Port Conflicts](#port-conflicts)
- [High Latency](#high-latency)
- [Container Won't Start](#container-wont-start)
- [Logs Show Errors](#logs-show-errors)
- [Network Connectivity Issues](#network-connectivity-issues)

## Allocation Fails

### Symptoms
- TURN allocation requests fail
- Clients cannot establish relay connections
- Logs show allocation errors

### Solutions

1. **Check EXTERNAL_IP configuration:**
   ```bash
   # Verify EXTERNAL_IP is set correctly in .env
   grep EXTERNAL_IP .env
   
   # Should be your public IP address
   # Test: curl ifconfig.me
   ```

2. **Verify external IP is reachable:**
   ```bash
   bash scripts/validate-network.sh <EXTERNAL_IP>
   ```

3. **Check firewall rules:**
   ```bash
   # Ensure ports are open
   # UDP 3478, TCP 3478, TCP 5349
   sudo ufw status
   # or
   sudo iptables -L -n
   ```

4. **Verify network mode:**
   ```yaml
   # docker-compose.yml should have:
   network_mode: host
   ```

5. **Check logs:**
   ```bash
   docker-compose logs coturn | grep -i allocation
   ```

## TLS Errors

### Symptoms
- TLS/TURNS connections fail
- Certificate errors in logs
- Clients cannot connect via TURNS

### Solutions

1. **Validate certificates:**
   ```bash
   bash scripts/validate-certificates.sh
   ```

2. **Check certificate paths:**
   ```bash
   # Verify files exist
   ls -la certs/cert.pem certs/key.pem
   
   # Check permissions
   # cert.pem should be 644
   # key.pem should be 600
   ```

3. **Regenerate certificates:**
   ```bash
   # Self-signed (testing)
   bash scripts/setup-selfsigned.sh turn.local
   
   # Let's Encrypt (production)
   bash scripts/setup-letsencrypt.sh turn.example.com admin@example.com
   ```

4. **Check certificate expiration:**
   ```bash
   openssl x509 -in certs/cert.pem -noout -dates
   ```

5. **Verify certificate and key match:**
   ```bash
   openssl x509 -noout -modulus -in certs/cert.pem | openssl md5
   openssl rsa -noout -modulus -in certs/key.pem | openssl md5
   # Should match
   ```

## Authentication Fails

### Symptoms
- Authentication errors in logs
- Clients cannot authenticate
- "401 Unauthorized" responses

### Solutions

1. **Verify TURN_SECRET matches:**
   ```bash
   # Check .env file
   grep TURN_SECRET .env
   
   # Must match the secret used to generate credentials
   ```

2. **Regenerate credentials:**
   ```bash
   # Use the same secret as in .env
   bash scripts/generate-credentials.sh <TURN_SECRET>
   ```

3. **Check credential format:**
   ```bash
   # Username should be: timestamp:username
   # Password should be: HMAC-SHA1(secret, username)
   ```

4. **Test authentication:**
   ```bash
   bash scripts/test-auth.sh <host> <port> <secret>
   ```

5. **Check realm:**
   ```bash
   # Verify realm matches in .env and client config
   grep REALM .env
   ```

## Port Conflicts

### Symptoms
- Container fails to start
- "Address already in use" errors
- Port binding failures

### Solutions

1. **Check if ports are in use:**
   ```bash
   # Linux
   sudo netstat -tuln | grep -E '3478|5349'
   # or
   sudo ss -tuln | grep -E '3478|5349'
   
   # Find process using port
   sudo lsof -i :3478
   ```

2. **Stop conflicting services:**
   ```bash
   # Find and stop the process
   sudo kill <PID>
   ```

3. **Change ports in configuration:**
   ```bash
   # Edit .env
   LISTENING_PORT=3479
   TLS_LISTENING_PORT=5350
   
   # Update configuration
   bash scripts/update.sh
   ```

4. **Check Docker port conflicts:**
   ```bash
   docker ps | grep -E '3478|5349'
   ```

## High Latency

### Symptoms
- Slow connection establishment
- High latency in media relay
- Poor performance

### Solutions

1. **Check bandwidth limits:**
   ```bash
   # Verify MAX_BPS is appropriate
   grep MAX_BPS .env
   
   # May need to increase for high-traffic scenarios
   ```

2. **Monitor resource usage:**
   ```bash
   bash scripts/monitor-limits.sh
   docker stats zrc-coturn
   ```

3. **Check network path:**
   ```bash
   # Test connectivity
   ping <EXTERNAL_IP>
   traceroute <EXTERNAL_IP>
   ```

4. **Optimize port range:**
   ```conf
   # In turnserver.conf
   min-port=49152
   max-port=65535
   ```

5. **Check server resources:**
   ```bash
   # CPU, memory, network
   htop
   iftop
   ```

## Container Won't Start

### Symptoms
- Docker container exits immediately
- Container status shows "Exited"
- Startup errors

### Solutions

1. **Check logs:**
   ```bash
   docker-compose logs coturn
   ```

2. **Validate configuration:**
   ```bash
   # Check .env file
   cat .env
   
   # Validate certificates
   bash scripts/validate-certificates.sh
   ```

3. **Check Docker:**
   ```bash
   docker info
   docker-compose config
   ```

4. **Verify file permissions:**
   ```bash
   # Configuration should be readable
   ls -la turnserver.conf
   
   # Certificates should have correct permissions
   ls -la certs/
   ```

5. **Test configuration:**
   ```bash
   # Run setup again
   bash scripts/setup.sh
   ```

## Logs Show Errors

### Symptoms
- Error messages in logs
- Warnings about configuration
- Failed operations

### Solutions

1. **View recent logs:**
   ```bash
   docker-compose logs --tail=100 coturn
   tail -100 logs/turn.log
   ```

2. **Monitor logs in real-time:**
   ```bash
   bash scripts/monitor-logs.sh
   ```

3. **Search for specific errors:**
   ```bash
   docker-compose logs coturn | grep -i error
   docker-compose logs coturn | grep -i fail
   ```

4. **Check log rotation:**
   ```bash
   # Ensure logs directory is writable
   ls -ld logs/
   ```

5. **Increase verbosity:**
   ```conf
   # In turnserver.conf
   verbose
   ```

## Network Connectivity Issues

### Symptoms
- Cannot connect to TURN server
- Timeouts
- Connection refused errors

### Solutions

1. **Validate network configuration:**
   ```bash
   bash scripts/validate-network.sh <EXTERNAL_IP>
   ```

2. **Test STUN:**
   ```bash
   bash scripts/test-stun.sh <host>
   ```

3. **Check firewall:**
   ```bash
   # Ensure ports are open
   # UDP 3478, TCP 3478, TCP 5349
   sudo ufw allow 3478/udp
   sudo ufw allow 3478/tcp
   sudo ufw allow 5349/tcp
   ```

4. **Verify external IP:**
   ```bash
   # Get your public IP
   curl ifconfig.me
   
   # Compare with EXTERNAL_IP in .env
   grep EXTERNAL_IP .env
   ```

5. **Test from client:**
   ```bash
   # Use test scripts
   bash scripts/test-zrc-integration.sh <host> <stun-port> <turns-port> <secret>
   ```

## Diagnostic Commands

### Check Container Status
```bash
docker-compose ps
docker ps | grep coturn
```

### View Logs
```bash
docker-compose logs -f coturn
tail -f logs/turn.log
```

### Test Connectivity
```bash
# STUN
bash scripts/test-stun.sh

# TURN
bash scripts/test-turn.sh <host> <port> <username> <password>

# TLS
bash scripts/test-tls.sh <host> <port>
```

### Check Ports
```bash
netstat -tuln | grep -E '3478|5349'
ss -tuln | grep -E '3478|5349'
```

### Validate Configuration
```bash
bash scripts/validate-certificates.sh
bash scripts/validate-network.sh <EXTERNAL_IP>
```

### Monitor Resources
```bash
docker stats zrc-coturn
bash scripts/monitor-limits.sh
```

## Getting Help

If issues persist:

1. **Collect diagnostic information:**
   ```bash
   # Container logs
   docker-compose logs coturn > coturn-logs.txt
   
   # Configuration
   cat .env > config.txt
   cat turnserver.conf > turnserver-conf.txt
   
   # System info
   docker info > docker-info.txt
   ```

2. **Check coturn documentation:**
   - [coturn GitHub](https://github.com/coturn/coturn)
   - [coturn Wiki](https://github.com/coturn/coturn/wiki)

3. **Review this troubleshooting guide** for similar issues

4. **Check ZRC project documentation** for integration-specific issues
