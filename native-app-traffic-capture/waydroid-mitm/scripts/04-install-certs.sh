#!/bin/bash
set -euo pipefail

# Waydroid MITM - Certificate Installation Script
# Installs mitmproxy CA certificate into Waydroid system trust store

echo "================================================"
echo "Waydroid MITM - Certificate Installation"
echo "================================================"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "❌ Please run as root (sudo)"
    exit 1
fi

# Check if mitmproxy cert exists
MITM_CERT="/root/.mitmproxy/mitmproxy-ca-cert.pem"
if [ ! -f "$MITM_CERT" ]; then
    echo "❌ mitmproxy certificate not found at $MITM_CERT"
    echo "   Run 03-setup-mitm.sh first."
    exit 1
fi

echo "📜 Found mitmproxy CA certificate: $MITM_CERT"

# Check if Waydroid is running
if ! waydroid status 2>&1 | grep -q "Session.*RUNNING"; then
    echo "⚠️  Waydroid session not running, starting..."
    waydroid session start &
    sleep 15
fi

# Check ADB connection
echo "🔧 Checking ADB connection..."
adb connect localhost:5555 2>/dev/null || true
sleep 2

if ! adb devices | grep -q "5555"; then
    echo "❌ ADB not connected to Waydroid"
    echo "   Try: adb connect localhost:5555"
    exit 1
fi

echo "✅ ADB connected"

# Calculate certificate hash (Android uses subject_hash_old format)
echo "🔧 Calculating certificate hash..."
CERT_HASH=$(openssl x509 -inform PEM -subject_hash_old -in "$MITM_CERT" | head -1)
echo "   Certificate hash: $CERT_HASH"

# Convert to the format Android expects
CERT_DEST="/system/etc/security/cacerts/${CERT_HASH}.0"
echo "   Target location: $CERT_DEST"

# For Waydroid, we can use the overlay system
OVERLAY_DIR="/var/lib/waydroid/overlay"
OVERLAY_CACERTS="$OVERLAY_DIR/system/etc/security/cacerts"

echo "🔧 Creating overlay directory structure..."
mkdir -p "$OVERLAY_CACERTS"

# Copy certificate to overlay
echo "📋 Installing certificate to system trust store..."
cp "$MITM_CERT" "$OVERLAY_CACERTS/${CERT_HASH}.0"
chmod 644 "$OVERLAY_CACERTS/${CERT_HASH}.0"

echo "✅ Certificate installed to overlay: $OVERLAY_CACERTS/${CERT_HASH}.0"

# Restart Waydroid to apply changes
echo "🔄 Restarting Waydroid to apply changes..."
waydroid session stop
sleep 3
waydroid session start &
sleep 15

# Reconnect ADB
echo "🔧 Reconnecting ADB..."
adb connect localhost:5555
sleep 3

# Verify installation using ADB
echo "🔍 Verifying certificate installation..."
if adb shell "ls /system/etc/security/cacerts/${CERT_HASH}.0" 2>/dev/null; then
    echo "✅ Certificate visible in Android system"
else
    echo "⚠️  Certificate not visible via ADB (may be normal with overlay)"
fi

# Alternative: try to push via ADB if overlay doesn't work
echo "🔧 Attempting ADB-based installation as backup..."
echo "   (This requires Waydroid root access)"

# Check if we can remount system as RW
if adb shell "su -c 'mount -o rw,remount /system'" 2>/dev/null; then
    echo "   System remounted as read-write"

    # Push certificate
    TMP_CERT="/sdcard/mitmproxy-cert.pem"
    adb push "$MITM_CERT" "$TMP_CERT"

    # Install to system
    adb shell "su -c 'cp $TMP_CERT /system/etc/security/cacerts/${CERT_HASH}.0'"
    adb shell "su -c 'chmod 644 /system/etc/security/cacerts/${CERT_HASH}.0'"
    adb shell "su -c 'chown root:root /system/etc/security/cacerts/${CERT_HASH}.0'"

    # Clean up
    adb shell "rm $TMP_CERT"

    echo "✅ Certificate installed via ADB"

    # Verify
    if adb shell "ls -l /system/etc/security/cacerts/${CERT_HASH}.0" 2>/dev/null; then
        echo "✅ Certificate verified in /system/etc/security/cacerts/"
    fi
else
    echo "⚠️  Could not remount system (this is OK if overlay method worked)"
fi

echo "🔧 Listing system certificates..."
CERT_COUNT=$(adb shell "ls /system/etc/security/cacerts/ | wc -l" 2>/dev/null || echo "0")
echo "   Total system certificates: $CERT_COUNT"

# Check if our cert is there
if adb shell "ls /system/etc/security/cacerts/${CERT_HASH}.0" 2>/dev/null; then
    echo "✅ mitmproxy certificate is in system trust store"
    adb shell "ls -l /system/etc/security/cacerts/${CERT_HASH}.0"
else
    echo "⚠️  Certificate installation may have failed"
    echo "   Manual steps:"
    echo "     1. adb shell"
    echo "     2. su"
    echo "     3. mount -o rw,remount /system"
    echo "     4. cp /sdcard/cert.pem /system/etc/security/cacerts/${CERT_HASH}.0"
    echo "     5. chmod 644 /system/etc/security/cacerts/${CERT_HASH}.0"
fi

echo "✅ Certificate installation complete!"
echo ""
echo "📝 Next steps:"
echo "  1. Run: ./test-traffic.sh (test with curl/browser)"
echo "  2. Run: ./05-install-frida.sh (optional, for stubborn apps)"
echo "  3. Install apps and test: ./06-test-youtube.sh"
echo ""
echo "📋 Certificate info:"
echo "  - Hash: $CERT_HASH"
echo "  - Overlay location: $OVERLAY_CACERTS/${CERT_HASH}.0"
echo "  - System location: /system/etc/security/cacerts/${CERT_HASH}.0"
