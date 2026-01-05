#!/bin/bash
# coturn Stop Script
# Gracefully stops the coturn service

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_DIR"

echo "Stopping coturn service..."

# Stop the service
docker-compose down

# Wait for graceful shutdown
sleep 2

# Verify container is stopped
if docker ps -a | grep -q zrc-coturn; then
    if docker ps | grep -q zrc-coturn; then
        echo "WARNING: Container is still running"
        echo "Force stopping..."
        docker-compose kill
        docker-compose down
    else
        echo "✓ coturn container stopped"
    fi
else
    echo "✓ coturn container not found (already stopped)"
fi

echo ""
echo "coturn has been stopped"
