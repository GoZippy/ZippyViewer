#!/bin/bash
# coturn Start Script
# Starts the coturn service and verifies health

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_DIR"

echo "Starting coturn service..."

# Check if Docker is running
if ! docker info &> /dev/null; then
    echo "Error: Docker is not running"
    exit 1
fi

# Start the service
docker-compose up -d

# Wait for service to start
echo "Waiting for coturn to start..."
sleep 5

# Check if container is running
if docker ps | grep -q zrc-coturn; then
    echo "✓ coturn container is running"
else
    echo "ERROR: coturn container failed to start"
    docker-compose logs coturn
    exit 1
fi

# Wait a bit more for health check
sleep 5

# Check health
echo "Checking health..."
if docker ps | grep zrc-coturn | grep -q "healthy\|Up"; then
    echo "✓ coturn is healthy"
else
    echo "WARNING: coturn may not be fully healthy yet"
    echo "Check logs with: docker-compose logs -f coturn"
fi

# Show status
echo ""
echo "coturn Status:"
docker-compose ps

echo ""
echo "View logs: docker-compose logs -f coturn"
echo "Stop service: docker-compose down"
