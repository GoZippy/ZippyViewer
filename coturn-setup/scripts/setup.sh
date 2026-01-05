#!/bin/bash
# coturn Setup Script
# Initial deployment and configuration validation

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "coturn Setup Script"
echo "==================="
echo ""

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "Error: Docker is not installed"
    echo "Install Docker from: https://docs.docker.com/get-docker/"
    exit 1
fi

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo "Error: Docker Compose is not installed"
    echo "Install Docker Compose from: https://docs.docker.com/compose/install/"
    exit 1
fi

cd "$PROJECT_DIR"

# Check if .env file exists
if [ ! -f ".env" ]; then
    echo "Creating .env file from env.example..."
    if [ -f "env.example" ]; then
        cp env.example .env
        echo "✓ Created .env file"
        echo ""
        echo "IMPORTANT: Edit .env file and set:"
        echo "  - EXTERNAL_IP (your public IP address)"
        echo "  - TURN_SECRET (generate with: openssl rand -hex 32)"
        echo ""
        read -p "Press Enter to continue after editing .env file..."
    else
        echo "Error: env.example not found"
        exit 1
    fi
fi

# Source .env file
set -a
source .env
set +a

# Validate required variables
if [ -z "$EXTERNAL_IP" ] || [ "$EXTERNAL_IP" = "203.0.113.1" ]; then
    echo "ERROR: EXTERNAL_IP must be set to your public IP address in .env"
    exit 1
fi

if [ -z "$TURN_SECRET" ] || [ "$TURN_SECRET" = "your-secret-key-here-change-this" ]; then
    echo "ERROR: TURN_SECRET must be set in .env"
    echo "Generate a secret with: openssl rand -hex 32"
    exit 1
fi

# Update turnserver.conf with environment variables
echo "Updating turnserver.conf with environment variables..."
if [ -f "turnserver.conf.template" ]; then
    envsubst < turnserver.conf.template > turnserver.conf
    echo "✓ Updated turnserver.conf"
else
    echo "WARNING: turnserver.conf.template not found, using existing turnserver.conf"
fi

# Validate certificates
echo "Validating TLS certificates..."
if [ -f "scripts/validate-certificates.sh" ]; then
    bash scripts/validate-certificates.sh || {
        echo "WARNING: Certificate validation failed"
        echo "Generate certificates with:"
        echo "  - Self-signed: bash scripts/setup-selfsigned.sh"
        echo "  - Let's Encrypt: bash scripts/setup-letsencrypt.sh <domain>"
        read -p "Continue without valid certificates? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    }
else
    echo "WARNING: Certificate validation script not found"
fi

# Validate network
echo "Validating network configuration..."
if [ -f "scripts/validate-network.sh" ]; then
    bash scripts/validate-network.sh "$EXTERNAL_IP" "${LISTENING_PORT:-3478}" "${TLS_LISTENING_PORT:-5349}"
fi

# Create necessary directories
echo "Creating necessary directories..."
mkdir -p certs logs
chmod 755 certs logs
chmod 600 certs/*.pem 2>/dev/null || true

echo ""
echo "✓ Setup complete!"
echo ""
echo "Next steps:"
echo "  1. Review turnserver.conf configuration"
echo "  2. Ensure certificates are in place (certs/cert.pem, certs/key.pem)"
echo "  3. Start coturn: docker-compose up -d"
echo "  4. Check logs: docker-compose logs -f coturn"
echo "  5. Test connectivity: bash scripts/test-stun.sh"
