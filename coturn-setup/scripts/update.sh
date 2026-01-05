#!/bin/bash
# coturn Update Script
# Updates configuration and restarts the service

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_DIR"

echo "Updating coturn configuration..."

# Check if .env file exists
if [ ! -f ".env" ]; then
    echo "Error: .env file not found"
    echo "Run setup.sh first"
    exit 1
fi

# Source .env file
set -a
source .env
set +a

# Update turnserver.conf
if [ -f "turnserver.conf.template" ]; then
    echo "Updating turnserver.conf..."
    envsubst < turnserver.conf.template > turnserver.conf
    echo "✓ Updated turnserver.conf"
fi

# Pull latest image (optional)
read -p "Pull latest coturn image? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    docker-compose pull
fi

# Restart the service
echo "Restarting coturn service..."
docker-compose down
docker-compose up -d

# Wait for service to start
sleep 5

# Check status
if docker ps | grep -q zrc-coturn; then
    echo "✓ coturn updated and running"
    docker-compose ps
else
    echo "ERROR: coturn failed to start after update"
    docker-compose logs coturn
    exit 1
fi

echo ""
echo "Update complete!"
