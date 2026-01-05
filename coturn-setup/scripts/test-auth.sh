#!/bin/bash
# Authentication Test Script
# Tests TURN authentication with static-auth-secret

set -e

HOST="${1:-127.0.0.1}"
PORT="${2:-3478}"
SECRET="${3:-}"

if [ -z "$SECRET" ]; then
    echo "Usage: $0 <host> <port> <static-auth-secret>"
    echo ""
    echo "Example: $0 turn.example.com 3478 my-secret-key"
    exit 1
fi

echo "Testing TURN authentication..."
echo "Host: $HOST"
echo "Port: $PORT"
echo ""

# Generate credentials
echo "Generating test credentials..."
CREDS=$(bash scripts/generate-credentials.sh "$SECRET" "test" 3600)
USERNAME=$(echo "$CREDS" | grep "Username:" | cut -d' ' -f2)
PASSWORD=$(echo "$CREDS" | grep "Password:" | cut -d' ' -f2)

echo "Generated credentials:"
echo "  Username: $USERNAME"
echo "  Password: $PASSWORD"
echo ""

# Test allocation with generated credentials
echo "Testing TURN allocation with generated credentials..."
bash scripts/test-turn.sh "$HOST" "$PORT" "$USERNAME" "$PASSWORD" && {
    echo ""
    echo "✓ Authentication test successful"
} || {
    echo ""
    echo "ERROR: Authentication test failed"
    echo "Check:"
    echo "  1. TURN_SECRET in .env matches the secret used"
    echo "  2. coturn is running and configured correctly"
    echo "  3. Network connectivity to TURN server"
    exit 1
}
