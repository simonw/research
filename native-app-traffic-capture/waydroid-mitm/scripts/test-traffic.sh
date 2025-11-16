#!/bin/bash
set -euo pipefail

# Waydroid MITM - Basic Traffic Test Script
# Tests that HTTPS traffic is being intercepted and decrypted

echo "================================================"
echo "Waydroid MITM - Traffic Interception Test"
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

# Check if mitmproxy is running
echo "🔧 Checking mitmproxy status..."
if ! systemctl is-active --quiet mitmproxy-waydroid; then
    echo "❌ mitmproxy service is not running"
    echo "   Start it with: systemctl start mitmproxy-waydroid"
    exit 1
fi
echo "✅ mitmproxy is running"

echo ""
echo "================================================"
echo "Test 1: Basic HTTP Request"
echo "================================================"
echo "📡 Testing HTTP request from Android..."
adb shell "curl -v http://example.com/" 2>&1 | head -20

echo ""
echo "================================================"
echo "Test 2: HTTPS Request (Should be intercepted)"
echo "================================================"
echo "📡 Testing HTTPS request from Android..."
adb shell "curl -v https://example.com/" 2>&1 | head -20

echo ""
echo "================================================"
echo "Test 3: Check mitmproxy Flows"
echo "================================================"
echo "🔍 Checking if mitmproxy captured any flows..."

# Give mitmproxy a moment to process
sleep 2

# Try to access mitmproxy web API
FLOW_COUNT=$(curl -s http://localhost:8081/flows | jq '. | length' 2>/dev/null || echo "0")

if [ "$FLOW_COUNT" != "0" ]; then
    echo "✅ mitmproxy has captured $FLOW_COUNT flow(s)"
    echo ""
    echo "Recent flows:"
    curl -s http://localhost:8081/flows | jq '.[] | {method: .request.method, url: .request.url, status: .response.status_code}' | head -20
else
    echo "⚠️  No flows captured yet"
    echo "   This could mean:"
    echo "     - iptables rules not working"
    echo "     - Certificate not trusted"
    echo "     - Apps not making requests yet"
fi

echo ""
echo "================================================"
echo "Test 4: Manual App Test"
echo "================================================"
echo "📱 Please manually test an app in Waydroid:"
echo "   1. Open a browser in Waydroid (via VNC)"
echo "   2. Navigate to https://example.com"
echo "   3. Check mitmproxy web UI at http://localhost:8081"
echo "   4. You should see the request with decrypted content"
echo ""
echo "🌐 mitmproxy Web UI: http://localhost:8081"
echo ""
echo "📝 Troubleshooting:"
echo "  - Check iptables: iptables -t nat -L WAYDROID_MITM -n -v"
echo "  - Check proxy logs: journalctl -u mitmproxy-waydroid -f"
echo "  - Check certificate: adb shell 'ls -l /system/etc/security/cacerts/' | grep mitmproxy"
echo "  - Test directly: curl -x http://192.168.250.1:8080 https://example.com"
