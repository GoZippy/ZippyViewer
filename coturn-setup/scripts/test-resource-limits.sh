#!/bin/bash
# Resource Limits Test Script
# Tests bandwidth and quota limits

set -e

HOST="${1:-127.0.0.1}"
PORT="${2:-3478}"
SECRET="${3:-}"

if [ -z "$SECRET" ]; then
    echo "Usage: $0 <host> <port> <static-auth-secret>"
    exit 1
fi

echo "Testing resource limits..."
echo "Host: $HOST"
echo "Port: $PORT"
echo ""

# Read limits from .env or use defaults
source .env 2>/dev/null || true
MAX_BPS="${MAX_BPS:-10000000}"
MAX_ALLOCATIONS="${MAX_ALLOCATIONS:-1000}"
USER_QUOTA="${USER_QUOTA:-12}"

echo "Configured limits:"
echo "  Max bandwidth per user: $MAX_BPS bps ($((MAX_BPS / 1000000)) Mbps)"
echo "  Max total allocations: $MAX_ALLOCATIONS"
echo "  User quota: $USER_QUOTA allocations"
echo ""

# Generate credentials
CREDS=$(bash scripts/generate-credentials.sh "$SECRET" "test" 3600)
USERNAME=$(echo "$CREDS" | grep "Username:" | cut -d' ' -f2)
PASSWORD=$(echo "$CREDS" | grep "Password:" | cut -d' ' -f2)

echo "Testing allocation creation..."
# Create multiple allocations to test quota
for i in $(seq 1 $((USER_QUOTA + 1))); do
    echo "Creating allocation $i/$USER_QUOTA..."
    if bash scripts/test-turn.sh "$HOST" "$PORT" "$USERNAME" "$PASSWORD" > /dev/null 2>&1; then
        echo "  ✓ Allocation $i created"
    else
        if [ $i -le $USER_QUOTA ]; then
            echo "  ✗ Allocation $i failed (unexpected)"
        else
            echo "  ✓ Quota limit reached at allocation $i (expected)"
        fi
        break
    fi
    sleep 1
done

echo ""
echo "Resource limits test complete"
echo ""
echo "Note: Bandwidth limits are harder to test automatically."
echo "Monitor logs and actual usage to verify bandwidth limits."
