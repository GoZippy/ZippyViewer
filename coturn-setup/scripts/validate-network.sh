#!/bin/bash
# Network Validation Script for coturn
# Validates network connectivity and port availability

set -e

EXTERNAL_IP="${1:-}"
LISTENING_PORT="${2:-3478}"
TLS_PORT="${3:-5349}"

echo "Validating network configuration for coturn..."

# Check if ports are available
check_port() {
    local port=$1
    local protocol=$2
    
    if command -v netstat &> /dev/null; then
        if netstat -tuln | grep -q ":$port "; then
            echo "WARNING: Port $port ($protocol) is already in use"
            return 1
        fi
    elif command -v ss &> /dev/null; then
        if ss -tuln | grep -q ":$port "; then
            echo "WARNING: Port $port ($protocol) is already in use"
            return 1
        fi
    else
        echo "WARNING: Cannot check port availability (netstat/ss not found)"
    fi
    
    echo "✓ Port $port ($protocol) appears to be available"
    return 0
}

# Check required ports
echo "Checking port availability..."
check_port "$LISTENING_PORT" "UDP/TCP" || true
check_port "$TLS_PORT" "TLS" || true

# Validate external IP
if [ -n "$EXTERNAL_IP" ]; then
    echo "Checking external IP: $EXTERNAL_IP"
    
    # Basic IP format validation
    if [[ $EXTERNAL_IP =~ ^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]]; then
        echo "✓ External IP format is valid"
    else
        echo "WARNING: External IP format may be invalid: $EXTERNAL_IP"
    fi
    
    # Try to ping (may fail if ICMP is blocked, which is OK)
    if ping -c 1 -W 2 "$EXTERNAL_IP" > /dev/null 2>&1; then
        echo "✓ External IP is reachable"
    else
        echo "INFO: Cannot ping external IP (this is normal if ICMP is blocked)"
    fi
else
    echo "WARNING: EXTERNAL_IP is not set"
    echo "Set EXTERNAL_IP in .env file for NAT scenarios"
fi

# Check if running in Docker
if [ -f /.dockerenv ] || [ -n "$DOCKER_CONTAINER" ]; then
    echo "INFO: Running inside Docker container"
    echo "Note: Using network_mode: host is required for TURN to work correctly"
fi

# Check firewall (if iptables is available)
if command -v iptables &> /dev/null && [ "$EUID" -eq 0 ]; then
    echo "Checking firewall rules..."
    if iptables -L -n | grep -q "$LISTENING_PORT"; then
        echo "INFO: Firewall rules found for port $LISTENING_PORT"
    else
        echo "INFO: No specific firewall rules found (may be using default policy)"
    fi
fi

echo ""
echo "Network validation complete!"
echo ""
echo "Required ports for firewall configuration:"
echo "  - UDP $LISTENING_PORT (STUN/TURN)"
echo "  - TCP $LISTENING_PORT (TURN)"
echo "  - TCP $TLS_PORT (TURNS/TLS)"
