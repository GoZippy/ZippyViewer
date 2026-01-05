#!/bin/bash
# TURN Credential Generation Helper
# Generates TURN credentials using static-auth-secret for ZRC client integration

set -e

SECRET="${1:-}"
USERNAME="${2:-zrc}"
TTL="${3:-3600}"  # Default 1 hour

if [ -z "$SECRET" ]; then
    echo "Usage: $0 <static-auth-secret> [username] [ttl-seconds]"
    echo ""
    echo "Example: $0 my-secret-key zrc 3600"
    echo ""
    echo "This script generates TURN credentials using the static-auth-secret method."
    echo "The credentials are time-limited and can be used by ZRC clients."
    exit 1
fi

# Check if openssl is available
if ! command -v openssl &> /dev/null; then
    echo "Error: openssl is not installed"
    exit 1
fi

# Generate timestamp (Unix epoch)
TIMESTAMP=$(date +%s)
EXPIRY=$((TIMESTAMP + TTL))

# Create username: timestamp:username
TURN_USERNAME="${TIMESTAMP}:${USERNAME}"

# Generate password: HMAC-SHA1(secret, username)
TURN_PASSWORD=$(echo -n "$TURN_USERNAME" | openssl dgst -sha1 -hmac "$SECRET" -binary | base64)

echo "TURN Credentials Generated:"
echo "=========================="
echo "Username: $TURN_USERNAME"
echo "Password: $TURN_PASSWORD"
echo "Realm: zrc.local"
echo "Expires: $(date -d "@$EXPIRY" 2>/dev/null || date -r "$EXPIRY" 2>/dev/null)"
echo ""
echo "For ZRC client configuration:"
echo "  turn_uri: turn:turn.example.com:3478"
echo "  turns_uri: turns:turn.example.com:5349"
echo "  username: $TURN_USERNAME"
echo "  password: $TURN_PASSWORD"
echo "  realm: zrc.local"
