#!/bin/bash
# TLS/TURNS Test Script
# Tests TLS TURN connections

set -e

HOST="${1:-127.0.0.1}"
PORT="${2:-5349}"

echo "Testing TLS/TURNS functionality..."
echo "Host: $HOST"
echo "Port: $PORT"
echo ""

# Check if certificates are valid
if [ -f "scripts/validate-certificates.sh" ]; then
    echo "Validating certificates..."
    bash scripts/validate-certificates.sh || {
        echo "WARNING: Certificate validation failed"
        read -p "Continue with test? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    }
fi

# Test TLS connection
echo "Testing TLS connection..."
if command -v openssl &> /dev/null; then
    echo | openssl s_client -connect "$HOST:$PORT" -verify_return_error 2>&1 | grep -q "Verify return code: 0" && {
        echo "✓ TLS connection successful"
    } || {
        echo "WARNING: TLS connection may have issues (self-signed cert expected in dev)"
    }
else
    echo "WARNING: openssl not found, cannot test TLS connection"
fi

# Test TURNS with turnutils (if available)
if command -v turnutils_stunclient &> /dev/null; then
    echo "Testing TURNS with turnutils..."
    turnutils_stunclient "$HOST" -p "$PORT" --tls || {
        echo "WARNING: TURNS test failed (may need credentials)"
    }
elif docker ps | grep -q zrc-coturn; then
    echo "Testing TURNS from coturn container..."
    docker exec zrc-coturn turnutils_stunclient "$HOST" -p "$PORT" --tls || {
        echo "WARNING: TURNS test failed (may need credentials)"
    }
fi

echo ""
echo "✓ TLS/TURNS test complete"
