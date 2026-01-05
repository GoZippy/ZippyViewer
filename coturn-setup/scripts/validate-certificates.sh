#!/bin/bash
# Certificate Validation Script for coturn
# Validates that TLS certificates exist and are valid

set -e

CERT_DIR="./certs"
CERT_FILE="$CERT_DIR/cert.pem"
KEY_FILE="$CERT_DIR/key.pem"

echo "Validating TLS certificates..."

# Check if certs directory exists
if [ ! -d "$CERT_DIR" ]; then
    echo "ERROR: Certificate directory not found: $CERT_DIR"
    exit 1
fi

# Check if certificate file exists
if [ ! -f "$CERT_FILE" ]; then
    echo "ERROR: Certificate file not found: $CERT_FILE"
    echo "Run setup-selfsigned.sh or setup-letsencrypt.sh to generate certificates"
    exit 1
fi

# Check if private key file exists
if [ ! -f "$KEY_FILE" ]; then
    echo "ERROR: Private key file not found: $KEY_FILE"
    echo "Run setup-selfsigned.sh or setup-letsencrypt.sh to generate certificates"
    exit 1
fi

# Validate certificate
echo "Checking certificate validity..."
if openssl x509 -in "$CERT_FILE" -noout -text > /dev/null 2>&1; then
    echo "✓ Certificate file is valid"
else
    echo "ERROR: Certificate file is invalid or corrupted"
    exit 1
fi

# Validate private key
echo "Checking private key validity..."
if openssl rsa -in "$KEY_FILE" -check -noout > /dev/null 2>&1; then
    echo "✓ Private key file is valid"
else
    echo "ERROR: Private key file is invalid or corrupted"
    exit 1
fi

# Check if certificate and key match
echo "Verifying certificate and key match..."
CERT_MODULUS=$(openssl x509 -noout -modulus -in "$CERT_FILE" | openssl md5)
KEY_MODULUS=$(openssl rsa -noout -modulus -in "$KEY_FILE" | openssl md5)

if [ "$CERT_MODULUS" = "$KEY_MODULUS" ]; then
    echo "✓ Certificate and private key match"
else
    echo "ERROR: Certificate and private key do not match"
    exit 1
fi

# Check certificate expiration
echo "Checking certificate expiration..."
EXPIRY_DATE=$(openssl x509 -in "$CERT_FILE" -noout -enddate | cut -d= -f2)
EXPIRY_EPOCH=$(date -d "$EXPIRY_DATE" +%s 2>/dev/null || date -j -f "%b %d %H:%M:%S %Y %Z" "$EXPIRY_DATE" +%s 2>/dev/null)
CURRENT_EPOCH=$(date +%s)
DAYS_UNTIL_EXPIRY=$(( ($EXPIRY_EPOCH - $CURRENT_EPOCH) / 86400 ))

if [ $DAYS_UNTIL_EXPIRY -lt 0 ]; then
    echo "ERROR: Certificate has expired"
    exit 1
elif [ $DAYS_UNTIL_EXPIRY -lt 30 ]; then
    echo "WARNING: Certificate expires in $DAYS_UNTIL_EXPIRY days"
    echo "Consider renewing the certificate"
else
    echo "✓ Certificate is valid for $DAYS_UNTIL_EXPIRY more days"
fi

# Check file permissions
echo "Checking file permissions..."
if [ "$(stat -c %a "$CERT_FILE" 2>/dev/null || stat -f %A "$CERT_FILE" 2>/dev/null)" != "644" ]; then
    echo "WARNING: Certificate file permissions should be 644"
fi

if [ "$(stat -c %a "$KEY_FILE" 2>/dev/null || stat -f %A "$KEY_FILE" 2>/dev/null)" != "600" ]; then
    echo "WARNING: Private key file permissions should be 600"
    echo "Fixing permissions..."
    chmod 600 "$KEY_FILE"
fi

echo ""
echo "✓ All certificate validations passed!"
echo "Certificate: $CERT_FILE"
echo "Private Key: $KEY_FILE"
