#!/bin/bash
set -euo pipefail

# Waydroid MITM - MITM Proxy Setup Script
# Configures mitmproxy and iptables for transparent traffic interception

echo "================================================"
echo "Waydroid MITM - MITM Proxy Setup"
echo "================================================"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "❌ Please run as root (sudo)"
    exit 1
fi

# Check if mitmproxy is installed
if ! command -v mitmproxy &> /dev/null; then
    echo "❌ mitmproxy is not installed. Run 01-setup-vm.sh first."
    exit 1
fi

echo "🔧 Generating mitmproxy certificates..."
# Run mitmproxy once to generate certificates
timeout 5 mitmproxy --set confdir=/root/.mitmproxy 2>/dev/null || true

# Verify cert was generated
if [ ! -f /root/.mitmproxy/mitmproxy-ca-cert.pem ]; then
    echo "❌ Failed to generate mitmproxy certificate"
    exit 1
fi

echo "✅ mitmproxy CA certificate generated: /root/.mitmproxy/mitmproxy-ca-cert.pem"

# Get the Waydroid network interface
echo "🔧 Detecting Waydroid network interface..."
WAYDROID_IFACE=$(ip addr show | grep waydroid | head -1 | awk '{print $2}' | tr -d ':')

if [ -z "$WAYDROID_IFACE" ]; then
    echo "⚠️  Warning: Waydroid interface not found (waydroid0 expected)"
    echo "   Assuming waydroid0..."
    WAYDROID_IFACE="waydroid0"
fi

echo "   Using interface: $WAYDROID_IFACE"

# Get the bridge IP (typically 192.168.250.1)
BRIDGE_IP=$(ip addr show $WAYDROID_IFACE 2>/dev/null | grep 'inet ' | awk '{print $2}' | cut -d/ -f1 || echo "192.168.250.1")
echo "   Bridge IP: $BRIDGE_IP"

echo "🔧 Configuring iptables for transparent proxy..."

# Clear any existing rules for our chain
iptables -t nat -D PREROUTING -i $WAYDROID_IFACE -j WAYDROID_MITM 2>/dev/null || true
iptables -t nat -F WAYDROID_MITM 2>/dev/null || true
iptables -t nat -X WAYDROID_MITM 2>/dev/null || true

# Create custom chain
iptables -t nat -N WAYDROID_MITM

# Redirect HTTP traffic (port 80) to mitmproxy (port 8080)
iptables -t nat -A WAYDROID_MITM -p tcp --dport 80 -j REDIRECT --to-port 8080

# Redirect HTTPS traffic (port 443) to mitmproxy (port 8080)
iptables -t nat -A WAYDROID_MITM -p tcp --dport 443 -j REDIRECT --to-port 8080

# Apply the chain to Waydroid traffic
iptables -t nat -A PREROUTING -i $WAYDROID_IFACE -j WAYDROID_MITM

echo "✅ iptables rules configured:"
echo "   HTTP  (80)  -> mitmproxy (8080)"
echo "   HTTPS (443) -> mitmproxy (8080)"

echo "🔧 Verifying IP forwarding..."
if [ "$(cat /proc/sys/net/ipv4/ip_forward)" != "1" ]; then
    echo "   Enabling IP forwarding..."
    echo 1 > /proc/sys/net/ipv4/ip_forward
fi
echo "✅ IP forwarding enabled"

echo "🔧 Creating systemd service for mitmproxy..."
cat > /etc/systemd/system/mitmproxy-waydroid.service << EOF
[Unit]
Description=mitmproxy for Waydroid traffic interception
After=network.target waydroid-container.service

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/mitmweb \\
    --mode transparent \\
    --showhost \\
    --set confdir=/root/.mitmproxy \\
    --web-host 0.0.0.0 \\
    --web-port 8081 \\
    --listen-host 0.0.0.0 \\
    --listen-port 8080
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

echo "🚀 Starting mitmproxy service..."
systemctl daemon-reload
systemctl enable mitmproxy-waydroid
systemctl restart mitmproxy-waydroid

echo "⏳ Waiting for mitmproxy to start (5 seconds)..."
sleep 5

if systemctl is-active --quiet mitmproxy-waydroid; then
    echo "✅ mitmproxy service is running"
else
    echo "❌ mitmproxy service failed to start"
    echo "   Check logs: journalctl -u mitmproxy-waydroid -n 50"
    exit 1
fi

echo "🔧 Testing mitmproxy web interface..."
if curl -s http://localhost:8081/ > /dev/null 2>&1; then
    echo "✅ mitmproxy web UI is accessible"
else
    echo "⚠️  mitmproxy web UI not responding yet (may need more time)"
fi

echo "✅ MITM proxy setup complete!"
echo ""
echo "📊 Service status:"
systemctl status mitmproxy-waydroid --no-pager | head -10
echo ""
echo "🌐 Access points:"
echo "  - mitmproxy web UI: http://localhost:8081"
echo "  - Proxy endpoint: $BRIDGE_IP:8080"
echo ""
echo "📝 Useful commands:"
echo "  - View flows: Open http://localhost:8081 in browser"
echo "  - Service logs: journalctl -u mitmproxy-waydroid -f"
echo "  - Restart proxy: systemctl restart mitmproxy-waydroid"
echo "  - Check iptables: iptables -t nat -L WAYDROID_MITM -n -v"
echo ""
echo "Next steps:"
echo "  1. Run: ./04-install-certs.sh (to install CA cert in Waydroid)"
echo "  2. Run: ./test-traffic.sh (to verify traffic capture)"
