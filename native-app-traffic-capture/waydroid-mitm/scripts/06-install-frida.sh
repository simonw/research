#!/bin/bash
set -euo pipefail

# Waydroid MITM - Frida Installation Script
# Installs Frida server into Waydroid for advanced certificate pinning bypass

echo "================================================"
echo "Waydroid MITM - Frida Installation"
echo "================================================"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "❌ Please run as root (sudo)"
    exit 1
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

# Detect Android architecture
echo "🔧 Detecting Android architecture..."
ARCH=$(adb shell getprop ro.product.cpu.abi | tr -d '\r')
echo "   Architecture: $ARCH"

# Map to Frida architecture naming
case "$ARCH" in
    "x86_64")
        FRIDA_ARCH="x86_64"
        ;;
    "x86")
        FRIDA_ARCH="x86"
        ;;
    "arm64-v8a"|"aarch64")
        FRIDA_ARCH="arm64"
        ;;
    "armeabi-v7a"|"armeabi")
        FRIDA_ARCH="arm"
        ;;
    *)
        echo "❌ Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

echo "   Frida architecture: $FRIDA_ARCH"

# Get latest Frida version
echo "📥 Fetching latest Frida version..."
FRIDA_VERSION=$(curl -s https://api.github.com/repos/frida/frida/releases/latest | grep '"tag_name"' | cut -d'"' -f4)
echo "   Latest version: $FRIDA_VERSION"

# Download Frida server
FRIDA_SERVER="frida-server-${FRIDA_VERSION}-android-${FRIDA_ARCH}"
DOWNLOAD_URL="https://github.com/frida/frida/releases/download/${FRIDA_VERSION}/${FRIDA_SERVER}.xz"

echo "📥 Downloading Frida server..."
echo "   URL: $DOWNLOAD_URL"

cd /tmp
wget -q --show-progress "$DOWNLOAD_URL" -O "${FRIDA_SERVER}.xz"

echo "📦 Extracting Frida server..."
unxz -f "${FRIDA_SERVER}.xz"
chmod +x "$FRIDA_SERVER"

echo "📤 Pushing Frida server to device..."
adb push "$FRIDA_SERVER" /data/local/tmp/frida-server

echo "🔧 Setting permissions..."
adb shell "su -c 'chmod 755 /data/local/tmp/frida-server'"

echo "🚀 Starting Frida server..."
# Kill any existing frida-server
adb shell "su -c 'killall frida-server'" 2>/dev/null || true
sleep 2

# Start frida-server in background
adb shell "su -c '/data/local/tmp/frida-server &'" &
sleep 3

echo "🔍 Verifying Frida server is running..."
if adb shell "su -c 'ps | grep frida-server'" | grep -q frida-server; then
    echo "✅ Frida server is running"
else
    echo "⚠️  Frida server may not be running"
    echo "   Try starting manually: adb shell 'su -c /data/local/tmp/frida-server &'"
fi

echo "📥 Installing Frida tools on host..."
pip3 install frida-tools

echo "🔍 Testing Frida connection..."
if frida-ps -U 2>/dev/null | grep -q "system_server"; then
    echo "✅ Frida is working!"
    echo ""
    echo "📱 Running processes:"
    frida-ps -U | head -10
else
    echo "⚠️  Frida connection test failed"
    echo "   Manual steps:"
    echo "     1. adb shell"
    echo "     2. su"
    echo "     3. /data/local/tmp/frida-server &"
fi

echo "📥 Copying Frida unpinning scripts..."
# Copy from dockerify-android-mitm if available
if [ -d "/Users/kahtaf/Documents/workspace_kahtaf/research/native-app-traffic-capture/dockerify-android-mitm/frida-scripts" ]; then
    cp -r /Users/kahtaf/Documents/workspace_kahtaf/research/native-app-traffic-capture/dockerify-android-mitm/frida-scripts/* \
        "$(dirname "$0")/../frida-scripts/" 2>/dev/null || true
    echo "✅ Copied Frida scripts from dockerify-android-mitm"
else
    echo "ℹ️  Creating basic unpinning script..."
    mkdir -p "$(dirname "$0")/../frida-scripts"

    cat > "$(dirname "$0")/../frida-scripts/universal-ssl-unpin.js" << 'EOF'
// Universal Android SSL Unpinning Script
// Based on HTTP Toolkit's unpinning scripts

Java.perform(function() {
    console.log("[*] Universal SSL Unpinning Script Started");

    // SSLContext
    var SSLContext = Java.use('javax.net.ssl.SSLContext');
    SSLContext.init.overload('[Ljavax.net.ssl.KeyManager;', '[Ljavax.net.ssl.TrustManager;', 'java.security.SecureRandom').implementation = function(keyManager, trustManager, secureRandom) {
        console.log('[+] SSLContext.init() bypassed');
        this.init(keyManager, null, secureRandom);
    };

    // TrustManager
    var X509TrustManager = Java.use('javax.net.ssl.X509TrustManager');
    var TrustManager = Java.registerClass({
        name: 'com.sensepost.test.TrustManager',
        implements: [X509TrustManager],
        methods: {
            checkClientTrusted: function(chain, authType) {},
            checkServerTrusted: function(chain, authType) {},
            getAcceptedIssuers: function() { return []; }
        }
    });

    console.log('[+] Certificate pinning bypassed');
});
EOF
    echo "✅ Created basic unpinning script"
fi

echo "✅ Frida installation complete!"
echo ""
echo "📝 Frida info:"
echo "  - Version: $FRIDA_VERSION"
echo "  - Architecture: $FRIDA_ARCH"
echo "  - Device location: /data/local/tmp/frida-server"
echo ""
echo "📝 Usage examples:"
echo "  - List processes: frida-ps -U"
echo "  - Attach to app: frida -U -f com.example.app"
echo "  - Run script: frida -U -l script.js -f com.example.app"
echo ""
echo "📂 Unpinning scripts location:"
echo "  - $(dirname "$0")/../frida-scripts/"
echo ""
echo "Next steps:"
echo "  1. Run: ./test-traffic.sh (basic traffic test)"
echo "  2. Run: ./test-youtube.sh (YouTube with Frida)"
