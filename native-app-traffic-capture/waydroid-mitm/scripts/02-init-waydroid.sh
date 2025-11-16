#!/bin/bash
set -euo pipefail

# Waydroid MITM - Waydroid Initialization Script
# Initializes Waydroid with LineageOS and configures basic settings

echo "================================================"
echo "Waydroid MITM - Waydroid Initialization"
echo "================================================"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "❌ Please run as root (sudo)"
    exit 1
fi

# Check if Waydroid is installed
if ! command -v waydroid &> /dev/null; then
    echo "❌ Waydroid is not installed. Run 01-setup-vm.sh first."
    exit 1
fi

echo "🔧 Checking if Waydroid is already initialized..."
if [ -d "/var/lib/waydroid" ] && [ -f "/var/lib/waydroid/waydroid.cfg" ]; then
    echo "⚠️  Waydroid already initialized"
    read -p "Do you want to reinitialize (will delete all data)? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "🗑️  Stopping Waydroid..."
        waydroid session stop 2>/dev/null || true
        echo "🗑️  Removing existing Waydroid data..."
        rm -rf /var/lib/waydroid
        rm -rf ~/.local/share/waydroid
    else
        echo "✅ Skipping initialization"
        exit 0
    fi
fi

echo "📥 Initializing Waydroid with LineageOS..."
# Initialize with GAPPS (Google Apps) variant
# Use -s for system only (smaller, faster), or -g for GAPPS
waydroid init -s -f

echo "⏳ Waiting for initialization to complete..."
sleep 5

echo "🔧 Configuring Waydroid properties..."
# Set useful properties for MITM
cat > /var/lib/waydroid/waydroid_base.prop << 'EOF'
# Waydroid MITM Configuration
persist.waydroid.multi_windows=true
persist.waydroid.invert_colors=false
ro.debuggable=1
ro.secure=0
ro.adb.secure=0
persist.sys.usb.config=adb
EOF

echo "🔧 Enabling ADB over network..."
# Configure Waydroid for network ADB access
if [ -f /var/lib/waydroid/waydroid.cfg ]; then
    # Enable ADB
    if ! grep -q "adb.enabled" /var/lib/waydroid/waydroid.cfg; then
        echo "[properties]" >> /var/lib/waydroid/waydroid.cfg
        echo "adb.enabled=true" >> /var/lib/waydroid/waydroid.cfg
    fi
fi

echo "🚀 Starting Waydroid container..."
systemctl start waydroid-container

echo "⏳ Waiting for container to start (10 seconds)..."
sleep 10

echo "🚀 Starting Waydroid session..."
# Start in background
nohup waydroid session start > /var/log/waydroid-session.log 2>&1 &
WAYDROID_PID=$!

echo "⏳ Waiting for Android to boot (30 seconds)..."
sleep 30

echo "🔧 Checking Waydroid status..."
waydroid status || true

echo "🔧 Checking for ADB connection..."
# Waydroid ADB is available at localhost:5555
adb connect localhost:5555 || echo "⚠️  ADB connection failed, will retry..."
sleep 5
adb connect localhost:5555 || echo "⚠️  ADB still not available"

# Check if connected
if adb devices | grep -q "5555"; then
    echo "✅ ADB connected to Waydroid"
    echo ""
    echo "📱 Android device info:"
    adb shell getprop ro.build.version.release
    adb shell getprop ro.product.model
    echo ""
else
    echo "⚠️  Warning: ADB not connected yet. You may need to:"
    echo "     - Wait a bit longer for Android to fully boot"
    echo "     - Run: adb connect localhost:5555"
fi

echo "✅ Waydroid initialization complete!"
echo ""
echo "Next steps:"
echo "  1. Run: ./03-setup-mitm.sh"
echo "  2. Run: ./04-install-certs.sh"
echo ""
echo "📝 Useful commands:"
echo "  - Check status: waydroid status"
echo "  - Show UI: waydroid show-full-ui"
echo "  - ADB shell: adb -s localhost:5555 shell"
echo "  - Stop session: waydroid session stop"
echo "  - View logs: tail -f /var/log/waydroid-session.log"
