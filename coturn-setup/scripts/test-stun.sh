#!/bin/bash
# STUN Functionality Test Script
# Tests STUN server functionality

set -e

HOST="${1:-127.0.0.1}"
PORT="${2:-3478}"

echo "Testing STUN functionality..."
echo "Host: $HOST"
echo "Port: $PORT"
echo ""

# Check if turnutils_stunclient is available
if command -v turnutils_stunclient &> /dev/null; then
    echo "Running STUN test with turnutils_stunclient..."
    turnutils_stunclient "$HOST" -p "$PORT"
elif docker ps | grep -q zrc-coturn; then
    echo "Running STUN test from coturn container..."
    docker exec zrc-coturn turnutils_stunclient "$HOST" -p "$PORT"
else
    echo "Error: turnutils_stunclient not found and coturn container not running"
    echo "Install coturn tools or start the container first"
    exit 1
fi

echo ""
echo "✓ STUN test complete"
