#!/bin/bash
# Self-Signed Certificate Setup Script for coturn
# This script generates self-signed certificates for testing/development

set -e

DOMAIN="${1:-turn.local}"
CERT_DIR="./certs"
DAYS="${2:-365}"

echo "Generating self-signed certificate for domain: $DOMAIN"
echo "Valid for $DAYS days"

# Create certs directory if it doesn't exist
mkdir -p "$CERT_DIR"

# Generate self-signed certificate
openssl req -x509 \
    -newkey rsa:2048 \
    -keyout "$CERT_DIR/key.pem" \
    -out "$CERT_DIR/cert.pem" \
    -days "$DAYS" \
    -nodes \
    -subj "/C=US/ST=State/L=City/O=Organization/CN=$DOMAIN"

# Set appropriate permissions
chmod 644 "$CERT_DIR/cert.pem"
chmod 600 "$CERT_DIR/key.pem"

echo "Self-signed certificate generated successfully!"
echo ""
echo "Certificate: $CERT_DIR/cert.pem"
echo "Private Key: $CERT_DIR/key.pem"
echo ""
echo "WARNING: This is a self-signed certificate for testing only."
echo "For production, use Let's Encrypt certificates (see setup-letsencrypt.sh)"
