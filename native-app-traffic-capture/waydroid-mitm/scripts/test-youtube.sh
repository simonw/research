#!/bin/bash
set -euo pipefail

# Waydroid MITM - YouTube E2E Test Script
# Tests traffic interception from YouTube app (certificate pinned)

echo "================================================"
echo "Waydroid MITM - YouTube E2E Test"
echo "================================================"

# Check ADB connection
echo "🔧 Checking ADB connection..."
adb connect localhost:5555 2>/dev/null || true
sleep 2

if ! adb devices | grep -q "5555"; then
    echo "❌ ADB not connected to Waydroid"
    exit 1
fi
echo "✅ ADB connected"

# Check if YouTube is installed
echo "🔧 Checking for YouTube app..."
if adb shell "pm list packages | grep -q com.google.android.youtube"; then
    echo "✅ YouTube is installed"
else
    echo "⚠️  YouTube not found, installing..."

    # Check if APK exists locally
    YOUTUBE_APK="../apks/youtube.apk"
    if [ ! -f "$YOUTUBE_APK" ]; then
        echo "📥 Downloading YouTube APK..."
        echo "   Please download YouTube APK manually from APKMirror:"
        echo "   https://www.apkmirror.com/apk/google-inc/youtube/"
        echo "   Save to: $YOUTUBE_APK"
        exit 1
    fi

    echo "📤 Installing YouTube..."
    adb install -r "$YOUTUBE_APK"
    sleep 3
fi

# Get YouTube package name
PKG="com.google.android.youtube"

echo "🔧 Checking if Frida is running..."
if frida-ps -U 2>/dev/null | grep -q "frida-server"; then
    FRIDA_AVAILABLE=true
    echo "✅ Frida is available"
else
    FRIDA_AVAILABLE=false
    echo "⚠️  Frida not available (basic MITM only)"
fi

echo ""
echo "================================================"
echo "Test Scenario: YouTube Traffic Capture"
echo "================================================"
echo ""
echo "📝 Test steps:"
echo "  1. Clearing mitmproxy flows"
echo "  2. Starting YouTube app"
if [ "$FRIDA_AVAILABLE" = true ]; then
    echo "  3. Attaching Frida for SSL unpinning"
else
    echo "  3. (Skipping Frida - using system cert only)"
fi
echo "  4. Waiting for app to load"
echo "  5. Checking captured traffic"
echo ""

# Clear mitmproxy flows
echo "🗑️  Clearing mitmproxy flows..."
curl -X DELETE http://localhost:8081/flows 2>/dev/null || true

# Stop YouTube if running
echo "🛑 Stopping YouTube if running..."
adb shell "am force-stop $PKG" 2>/dev/null || true
sleep 2

if [ "$FRIDA_AVAILABLE" = true ]; then
    echo "🚀 Starting YouTube with Frida SSL unpinning..."

    # Check for unpinning script
    UNPIN_SCRIPT="../frida-scripts/universal-ssl-unpin.js"
    if [ ! -f "$UNPIN_SCRIPT" ]; then
        echo "⚠️  Unpinning script not found: $UNPIN_SCRIPT"
        echo "   Falling back to basic mode"
        FRIDA_AVAILABLE=false
    else
        # Start YouTube with Frida
        echo "📱 Launching YouTube with Frida..."
        frida -U -f $PKG -l "$UNPIN_SCRIPT" --no-pause &
        FRIDA_PID=$!
        sleep 5
    fi
fi

if [ "$FRIDA_AVAILABLE" = false ]; then
    echo "📱 Starting YouTube (basic mode)..."
    adb shell "monkey -p $PKG -c android.intent.category.LAUNCHER 1" 2>/dev/null || \
    adb shell "am start -n $PKG/com.google.android.apps.youtube.app.WatchWhileActivity"
    sleep 5
fi

echo "⏳ Waiting for YouTube to make network requests (15 seconds)..."
sleep 15

echo ""
echo "================================================"
echo "Test Results"
echo "================================================"

# Check mitmproxy flows
FLOW_COUNT=$(curl -s http://localhost:8081/flows | jq '. | length' 2>/dev/null || echo "0")

if [ "$FLOW_COUNT" = "0" ]; then
    echo "❌ No traffic captured!"
    echo ""
    echo "📝 Troubleshooting:"
    echo "  - Check if YouTube is actually running: adb shell 'ps | grep youtube'"
    echo "  - Check iptables rules: iptables -t nat -L WAYDROID_MITM -n -v"
    echo "  - Check mitmproxy logs: journalctl -u mitmproxy-waydroid -f"
    if [ "$FRIDA_AVAILABLE" = true ]; then
        echo "  - Check Frida logs above for errors"
    else
        echo "  - Consider installing Frida: ./06-install-frida.sh"
    fi
    exit 1
fi

echo "✅ Captured $FLOW_COUNT flow(s)!"
echo ""
echo "📊 Flow summary:"
curl -s http://localhost:8081/flows | jq '.[] | {
    method: .request.method,
    host: .request.host,
    path: .request.path,
    status: .response.status_code
}' | head -40

# Check for YouTube domains
YOUTUBE_FLOWS=$(curl -s http://localhost:8081/flows | jq '[.[] | select(.request.host | contains("youtube") or contains("googlevideo"))] | length')

if [ "$YOUTUBE_FLOWS" != "0" ]; then
    echo ""
    echo "🎉 SUCCESS! Captured $YOUTUBE_FLOWS YouTube-related requests!"
    echo ""
    echo "📺 YouTube domains found:"
    curl -s http://localhost:8081/flows | jq -r '.[] | select(.request.host | contains("youtube") or contains("googlevideo")) | .request.host' | sort -u
    echo ""
    echo "✅ Certificate pinning bypass is working!"
else
    echo ""
    echo "⚠️  No YouTube-specific traffic detected yet"
    echo "   Traffic was captured, but may not be from YouTube"
    echo "   Try interacting with the app more"
fi

echo ""
echo "🌐 View full details in mitmproxy web UI:"
echo "   http://localhost:8081"
echo ""
echo "📝 To see request/response bodies:"
echo "   1. Open http://localhost:8081"
echo "   2. Click on any YouTube request"
echo "   3. View Headers, Request, Response tabs"
echo "   4. All content should be decrypted!"

# Clean up Frida if we started it
if [ "$FRIDA_AVAILABLE" = true ] && [ ! -z "${FRIDA_PID:-}" ]; then
    kill $FRIDA_PID 2>/dev/null || true
fi
