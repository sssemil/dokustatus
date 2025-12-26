#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}")"
CERT_DIR="$SCRIPT_DIR/certs"

# Check if mkcert is installed
if ! command -v mkcert &> /dev/null; then
    echo "Error: mkcert is not installed."
    echo "Install it with: brew install mkcert (macOS) or see https://github.com/FiloSottile/mkcert"
    exit 1
fi

# Install mkcert CA if not already (needs sudo for system trust store)
if ! mkcert -check 2>/dev/null; then
    echo "Installing mkcert CA (requires sudo)..."
    sudo mkcert -install
fi

# Create certs directory (after sudo, to ensure user owns it)
mkdir -p "$CERT_DIR"

echo "Generating certificates..."

# Generate localhost cert
mkcert -cert-file "$CERT_DIR/localhost.pem" \
       -key-file "$CERT_DIR/localhost-key.pem" \
       localhost 127.0.0.1 ::1

# Generate wildcard cert for .test TLD
mkcert -cert-file "$CERT_DIR/wildcard.test.pem" \
       -key-file "$CERT_DIR/wildcard.test-key.pem" \
       "*.test" "*.reauth.test" "*.local.test" "*.example.test" localhost.test

echo ""
echo "Certificates generated in $CERT_DIR:"
ls -la "$CERT_DIR"
echo ""
echo "Done! You can now run ./run infra to start the local development stack."
