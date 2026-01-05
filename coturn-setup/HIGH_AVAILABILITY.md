# coturn High Availability Guide

Setup and configuration for high availability coturn deployment.

## Table of Contents

- [Overview](#overview)
- [Load Balancer Configuration](#load-balancer-configuration)
- [Health Checks](#health-checks)
- [Failover Procedures](#failover-procedures)
- [Geographic Distribution](#geographic-distribution)
- [Session Affinity](#session-affinity)

## Overview

High availability for coturn involves:

1. **Multiple coturn instances** behind a load balancer
2. **Health checks** to detect failures
3. **Automatic failover** when instances fail
4. **Geographic distribution** for low latency
5. **Session affinity** for TURN allocations

## Load Balancer Configuration

### Nginx Configuration

Example nginx configuration for load balancing:

```nginx
stream {
    upstream turn_udp {
        least_conn;
        server turn1.example.com:3478;
        server turn2.example.com:3478;
        server turn3.example.com:3478;
    }
    
    upstream turn_tcp {
        least_conn;
        server turn1.example.com:3478;
        server turn2.example.com:3478;
        server turn3.example.com:3478;
    }
    
    upstream turns_tcp {
        least_conn;
        server turn1.example.com:5349;
        server turn2.example.com:5349;
        server turn3.example.com:5349;
    }
    
    server {
        listen 3478 udp;
        proxy_pass turn_udp;
        proxy_timeout 1s;
        proxy_responses 1;
        error_log /var/log/nginx/turn_udp.log;
    }
    
    server {
        listen 3478;
        proxy_pass turn_tcp;
        proxy_timeout 30s;
        error_log /var/log/nginx/turn_tcp.log;
    }
    
    server {
        listen 5349;
        proxy_pass turns_tcp;
        proxy_timeout 30s;
        error_log /var/log/nginx/turns_tcp.log;
    }
}
```

### HAProxy Configuration

Example HAProxy configuration:

```haproxy
global
    log /dev/log local0
    maxconn 4096
    daemon

defaults
    log global
    mode tcp
    option tcplog
    timeout connect 5s
    timeout client 30s
    timeout server 30s

frontend turn_udp
    bind *:3478 udp
    default_backend turn_servers_udp

frontend turn_tcp
    bind *:3478
    default_backend turn_servers_tcp

frontend turns_tcp
    bind *:5349
    default_backend turn_servers_tls

backend turn_servers_udp
    balance roundrobin
    option httpchk GET /health
    server turn1 turn1.example.com:3478 check
    server turn2 turn2.example.com:3478 check
    server turn3 turn3.example.com:3478 check

backend turn_servers_tcp
    balance roundrobin
    option httpchk GET /health
    server turn1 turn1.example.com:3478 check
    server turn2 turn2.example.com:3478 check
    server turn3 turn3.example.com:3478 check

backend turn_servers_tls
    balance roundrobin
    option httpchk GET /health
    server turn1 turn1.example.com:5349 check
    server turn2 turn2.example.com:5349 check
    server turn3 turn3.example.com:5349 check
```

### Cloud Load Balancers

#### AWS ELB/ALB

- Use Network Load Balancer (NLB) for UDP/TCP
- Configure health checks on port 3478
- Enable cross-zone load balancing
- Use multiple availability zones

#### Google Cloud Load Balancer

- Use Network Load Balancer
- Configure health checks
- Use multiple regions for geographic distribution

#### Azure Load Balancer

- Use Standard Load Balancer
- Configure health probes
- Use multiple availability zones

## Health Checks

### STUN Health Check

Simple STUN health check script:

```bash
#!/bin/bash
# health-check.sh
HOST="${1:-127.0.0.1}"
PORT="${2:-3478}"

if turnutils_stunclient "$HOST" -p "$PORT" > /dev/null 2>&1; then
    exit 0
else
    exit 1
fi
```

### TURN Allocation Health Check

More comprehensive health check:

```bash
#!/bin/bash
# health-check-turn.sh
HOST="${1:-127.0.0.1}"
PORT="${2:-3478}"
USERNAME="${3:-}"
PASSWORD="${4:-}"

if [ -z "$USERNAME" ] || [ -z "$PASSWORD" ]; then
    echo "Usage: $0 <host> <port> <username> <password>"
    exit 1
fi

if turnutils_peer -L "$HOST" -p "$PORT" -U "$USERNAME" -P "$PASSWORD" > /dev/null 2>&1; then
    exit 0
else
    exit 1
fi
```

### Docker Health Check

In `docker-compose.yml`:

```yaml
healthcheck:
  test: ["CMD", "turnutils_stunclient", "127.0.0.1"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 10s
```

### Load Balancer Health Checks

Configure health checks in your load balancer:

- **Protocol**: UDP/TCP
- **Port**: 3478
- **Interval**: 30 seconds
- **Timeout**: 10 seconds
- **Unhealthy threshold**: 3 failures
- **Healthy threshold**: 2 successes

## Failover Procedures

### Automatic Failover

Load balancers automatically fail over when health checks fail:

1. Health check fails
2. Instance marked unhealthy
3. Traffic routed to healthy instances
4. Instance removed from pool

### Manual Failover

For planned maintenance:

1. **Drain connections:**
   ```bash
   # Mark instance as maintenance mode in load balancer
   ```

2. **Wait for connections to drain:**
   ```bash
   # Monitor active connections
   docker-compose logs coturn | grep -i "session"
   ```

3. **Stop instance:**
   ```bash
   bash scripts/stop.sh
   ```

4. **Perform maintenance:**
   ```bash
   # Update, restart, etc.
   ```

5. **Restart instance:**
   ```bash
   bash scripts/start.sh
   ```

6. **Verify health:**
   ```bash
   bash scripts/test-stun.sh
   ```

7. **Return to pool:**
   ```bash
   # Re-enable in load balancer
   ```

### Failover Testing

Test failover procedures:

```bash
# Stop one instance
docker-compose -f docker-compose.turn1.yml down

# Verify traffic routes to other instances
bash scripts/test-stun.sh <load-balancer-ip>

# Restart instance
docker-compose -f docker-compose.turn1.yml up -d

# Verify it rejoins pool
```

## Geographic Distribution

### Multi-Region Setup

Deploy coturn instances in multiple regions:

```
Region 1 (US East):
  - turn-us-east-1.example.com
  - turn-us-east-2.example.com

Region 2 (EU West):
  - turn-eu-west-1.example.com
  - turn-eu-west-2.example.com

Region 3 (Asia Pacific):
  - turn-ap-southeast-1.example.com
  - turn-ap-southeast-2.example.com
```

### DNS-Based Routing

Use DNS to route clients to nearest region:

```dns
turn.example.com    A    203.0.113.1  # US East
turn.example.com    A    203.0.113.2  # EU West
turn.example.com    A    203.0.113.3  # Asia Pacific
```

### Client-Side Selection

Clients can select nearest TURN server:

```toml
[turn]
# Primary (nearest)
turn_uri = "turn:turn-us-east.example.com:3478"

# Fallback (other regions)
turn_fallback = [
    "turn:turn-eu-west.example.com:3478",
    "turn:turn-ap-southeast.example.com:3478"
]
```

## Session Affinity

### Why Session Affinity?

TURN allocations are stateful. Clients should use the same TURN server for the duration of a session.

### Implementation

1. **Source IP affinity:**
   - Route based on client source IP
   - Use consistent hashing

2. **Cookie-based (for TCP):**
   - Not applicable for UDP TURN

3. **Client-side:**
   - Clients cache TURN server selection
   - Reuse same server for session

### Load Balancer Configuration

Configure session affinity in load balancer:

```nginx
# Nginx - IP hash
upstream turn_udp {
    ip_hash;
    server turn1.example.com:3478;
    server turn2.example.com:3478;
    server turn3.example.com:3478;
}
```

## Monitoring

### Health Check Monitoring

Monitor health check status:

```bash
# Check all instances
for instance in turn1 turn2 turn3; do
    echo "Checking $instance..."
    bash scripts/test-stun.sh $instance.example.com
done
```

### Load Balancer Monitoring

Monitor load balancer metrics:
- Active connections
- Health check failures
- Response times
- Throughput

### Alerting

Set up alerts for:
- Health check failures
- High error rates
- Resource exhaustion
- Geographic latency

## Deployment Example

### Multi-Instance docker-compose.yml

```yaml
version: '3.8'

services:
  coturn1:
    image: coturn/coturn:latest
    container_name: zrc-coturn-1
    network_mode: host
    volumes:
      - ./turnserver.conf:/etc/turnserver.conf:ro
      - ./certs:/etc/coturn/certs:ro
      - ./logs/turn1:/var/log/coturn
    environment:
      - EXTERNAL_IP=${EXTERNAL_IP_1}
    command: ["-c", "/etc/turnserver.conf"]

  coturn2:
    image: coturn/coturn:latest
    container_name: zrc-coturn-2
    network_mode: host
    volumes:
      - ./turnserver.conf:/etc/turnserver.conf:ro
      - ./certs:/etc/coturn/certs:ro
      - ./logs/turn2:/var/log/coturn
    environment:
      - EXTERNAL_IP=${EXTERNAL_IP_2}
    command: ["-c", "/etc/turnserver.conf"]

  coturn3:
    image: coturn/coturn:latest
    container_name: zrc-coturn-3
    network_mode: host
    volumes:
      - ./turnserver.conf:/etc/turnserver.conf:ro
      - ./certs:/etc/coturn/certs:ro
      - ./logs/turn3:/var/log/coturn
    environment:
      - EXTERNAL_IP=${EXTERNAL_IP_3}
    command: ["-c", "/etc/turnserver.conf"]
```

## Best Practices

1. **Deploy at least 3 instances** for redundancy
2. **Use health checks** to detect failures
3. **Configure automatic failover** in load balancer
4. **Distribute geographically** for low latency
5. **Monitor all instances** continuously
6. **Test failover procedures** regularly
7. **Use session affinity** for TURN allocations
8. **Balance load** across instances
9. **Plan for capacity** (scale horizontally)
10. **Document procedures** for operations team

## Additional Resources

- [coturn High Availability](https://github.com/coturn/coturn/wiki/High-Availability)
- [Load Balancer Best Practices](https://www.nginx.com/blog/nginx-high-availability-load-balancing/)
- [TURN Protocol RFC](https://tools.ietf.org/html/rfc5766)
