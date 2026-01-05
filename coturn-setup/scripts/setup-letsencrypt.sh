#!/bin/bash
# Let's Encrypt Certificate Setup Script for coturn
# This script sets up Let's Encrypt certificates for TURN/TLS

set -e

DOMAIN="${1:-}"
EMAIL="${2:-}"
CERT_DIR="./certs"

if [ -z "$DOMAIN" ]; then
    echo "Usage: $0 <domain> [email]"
    echo "Example: $0 turn.example.com admin@example.com"
    exit 1
fi

echo "Setting up Let's Encrypt certificate for domain: $DOMAIN"

# Check if certbot is installed
if ! command -v certbot &> /dev/null; then
    echo "Error: certbot is not installed"
    echo "Install it with: sudo apt-get install certbot (Ubuntu/Debian)"
    echo "                  sudo yum install certbot (RHEL/CentOS)"
    exit 1
fi

# Create certs directory if it doesn't exist
mkdir -p "$CERT_DIR"

# Stop coturn if running (certbot needs port 80/443)
echo "Note: Make sure ports 80 and 443 are available for certbot validation"
echo "You may need to stop coturn temporarily: docker-compose down"

# Obtain certificate
if [ -n "$EMAIL" ]; then
    certbot certonly --standalone \
        --preferred-challenges http \
        -d "$DOMAIN" \
        --email "$EMAIL" \
        --agree-tos \
        --non-interactive
else
    certbot certonly --standalone \
        --preferred-challenges http \
        -d "$DOMAIN" \
        --register-unsafely-without-email \
        --agree-tos \
        --non-interactive
fi

# Copy certificates to certs directory
CERT_PATH="/etc/letsencrypt/live/$DOMAIN"
if [ ! -d "$CERT_PATH" ]; then
    echo "Error: Certificate path not found: $CERT_PATH"
    exit 1
fi

echo "Copying certificates to $CERT_DIR..."
cp "$CERT_PATH/fullchain.pem" "$CERT_DIR/cert.pem"
cp "$CERT_PATH/privkey.pem" "$CERT_DIR/key.pem"

# Set appropriate permissions
chmod 644 "$CERT_DIR/cert.pem"
chmod 600 "$CERT_DIR/key.pem"

echo "Certificates installed successfully!"
echo ""
echo "Certificate: $CERT_DIR/cert.pem"
echo "Private Key: $CERT_DIR/key.pem"
echo ""
echo "To set up automatic renewal, add to crontab:"
echo "0 0 * * * certbot renew --quiet --deploy-hook 'cd $(pwd) && docker-compose restart coturn'"
