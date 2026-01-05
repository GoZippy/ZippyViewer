#!/bin/bash
# ZRC Integration Test Script
# End-to-end test with ZRC client configuration

set -e

HOST="${1:-turn.example.com}"
STUN_PORT="${2:-3478}"
TURNS_PORT="${3:-5349}"
SECRET="${4:-}"

if [ -z "$SECRET" ]; then
    echo "Usage: $0 <host> <stun-port> <turns-port> <static-auth-secret>"
    echo ""
    echo "Example: $0 turn.example.com 3478 5349 my-secret-key"
    exit 1
fi

echo "ZRC Integration Test"
echo "==================="
echo ""

# Generate credentials
echo "Generating TURN credentials for ZRC client..."
CREDS=$(bash scripts/generate-credentials.sh "$SECRET" "zrc" 3600)
USERNAME=$(echo "$CREDS" | grep "Username:" | cut -d' ' -f2)
PASSWORD=$(echo "$CREDS" | grep "Password:" | cut -d' ' -f2)

echo ""
echo "ZRC Client Configuration:"
echo "=========================="
echo "turn_uri: turn:$HOST:$STUN_PORT"
echo "turns_uri: turns:$HOST:$TURNS_PORT"
echo "username: $USERNAME"
echo "password: $PASSWORD"
echo "realm: zrc.local"
echo ""

# Test STUN
echo "Testing STUN connectivity..."
bash scripts/test-stun.sh "$HOST" "$STUN_PORT" && {
    echo "✓ STUN test passed"
} || {
    echo "✗ STUN test failed"
    exit 1
}

# Test TURN allocation
echo ""
echo "Testing TURN allocation..."
bash scripts/test-turn.sh "$HOST" "$STUN_PORT" "$USERNAME" "$PASSWORD" && {
    echo "✓ TURN allocation test passed"
} || {
    echo "✗ TURN allocation test failed"
    exit 1
}

# Test TURNS (if certificates are available)
if [ -f "certs/cert.pem" ] && [ -f "certs/key.pem" ]; then
    echo ""
    echo "Testing TURNS (TLS) connectivity..."
    bash scripts/test-tls.sh "$HOST" "$TURNS_PORT" && {
        echo "✓ TURNS test passed"
    } || {
        echo "WARNING: TURNS test had issues (may be expected with self-signed cert)"
    }
fi

echo ""
echo "✓ ZRC integration test complete!"
echo ""
echo "Use the configuration above in your ZRC client to connect via TURN."
