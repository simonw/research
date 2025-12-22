#!/bin/bash
# Install Brightdata CA certificate to Chrome's NSS database
# Required for HTTPS proxy to work without SSL errors

CERT_PATH="/usr/local/share/ca-certificates/brightdata.crt"
NSS_DIR="/config/.pki/nssdb"

if [ ! -f "$CERT_PATH" ]; then
    echo "Brightdata CA certificate not found at $CERT_PATH, skipping"
    exit 0
fi

mkdir -p "$NSS_DIR"

if [ ! -f "$NSS_DIR/cert9.db" ]; then
    certutil -d sql:"$NSS_DIR" -N --empty-password
fi

certutil -d sql:"$NSS_DIR" -A -t "C,," -n "Brightdata CA" -i "$CERT_PATH" 2>/dev/null || true

chown -R abc:abc /config/.pki

echo "Brightdata CA certificate installed to Chrome NSS database"
