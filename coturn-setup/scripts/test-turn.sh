#!/bin/bash
# TURN Allocation Test Script
# Tests TURN allocation functionality

set -e

HOST="${1:-127.0.0.1}"
PORT="${2:-3478}"
USERNAME="${3:-}"
PASSWORD="${4:-}"

if [ -z "$USERNAME" ] || [ -z "$PASSWORD" ]; then
    echo "Usage: $0 <host> <port> <username> <password>"
    echo ""
    echo "Example: $0 turn.example.com 3478 '1234567890:zrc' 'generated-password'"
    echo ""
    echo "Generate credentials with: bash scripts/generate-credentials.sh <secret>"
    exit 1
fi

echo "Testing TURN allocation..."
echo "Host: $HOST"
echo "Port: $PORT"
echo "Username: $USERNAME"
echo ""

# Check if turnutils_peer is available
if command -v turnutils_peer &> /dev/null; then
    echo "Running TURN allocation test..."
    turnutils_peer -L "$HOST" -p "$PORT" -U "$USERNAME" -P "$PASSWORD" || {
        echo "ERROR: TURN allocation test failed"
        exit 1
    }
elif docker ps | grep -q zrc-coturn; then
    echo "Running TURN allocation test from coturn container..."
    docker exec zrc-coturn turnutils_peer -L "$HOST" -p "$PORT" -U "$USERNAME" -P "$PASSWORD" || {
        echo "ERROR: TURN allocation test failed"
        exit 1
    }
else
    echo "Error: turnutils_peer not found and coturn container not running"
    exit 1
fi

echo ""
echo "✓ TURN allocation test complete"
