#!/bin/bash
set -euo pipefail

# Waydroid MITM - Headless VNC Setup Script
# Configures Waydroid for headless operation with VNC access

echo "================================================"
echo "Waydroid MITM - Headless VNC Setup"
echo "================================================"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "❌ Please run as root (sudo)"
    exit 1
fi

echo "📦 Installing VNC and Wayland dependencies..."
apt-get update
apt-get install -y \
    weston \
    wayvnc \
    tigervnc-standalone-server \
    xvfb \
    x11vnc

echo "🔧 Creating Weston configuration for headless mode..."
mkdir -p /root/.config

cat > /root/.config/weston.ini << 'EOF'
[core]
backend=headless-backend.so
use-pixman=true

[shell]
client=waydroid
panel-position=none

[output]
name=HEADLESS-1
mode=1920x1080

[screen-share]
command=/usr/bin/wayvnc 0.0.0.0 5900
EOF

echo "🔧 Creating systemd service for headless Weston..."
cat > /etc/systemd/system/weston-headless.service << 'EOF'
[Unit]
Description=Weston Wayland Compositor (Headless)
After=network.target

[Service]
Type=simple
User=root
Environment="XDG_RUNTIME_DIR=/run/user/0"
Environment="WAYLAND_DISPLAY=wayland-0"
ExecStartPre=/bin/mkdir -p /run/user/0
ExecStartPre=/bin/chmod 700 /run/user/0
ExecStart=/usr/bin/weston --backend=headless-backend.so --width=1920 --height=1080
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

echo "🔧 Creating systemd service for wayvnc..."
cat > /etc/systemd/system/wayvnc.service << 'EOF'
[Unit]
Description=WayVNC Server for Waydroid
After=weston-headless.service
Requires=weston-headless.service

[Service]
Type=simple
User=root
Environment="XDG_RUNTIME_DIR=/run/user/0"
Environment="WAYLAND_DISPLAY=wayland-0"
ExecStartPre=/bin/sleep 5
ExecStart=/usr/bin/wayvnc 0.0.0.0 5900
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

echo "🚀 Starting Weston headless compositor..."
systemctl daemon-reload
systemctl enable weston-headless
systemctl restart weston-headless

echo "⏳ Waiting for Weston to start (5 seconds)..."
sleep 5

if systemctl is-active --quiet weston-headless; then
    echo "✅ Weston is running"
else
    echo "❌ Weston failed to start"
    journalctl -u weston-headless -n 20
    exit 1
fi

echo "🚀 Starting wayvnc service..."
systemctl enable wayvnc
systemctl restart wayvnc

echo "⏳ Waiting for wayvnc to start (3 seconds)..."
sleep 3

if systemctl is-active --quiet wayvnc; then
    echo "✅ wayvnc is running"
else
    echo "⚠️  wayvnc may not be running (check logs if VNC doesn't work)"
fi

echo "🔧 Configuring Waydroid to use the Wayland display..."
export XDG_RUNTIME_DIR=/run/user/0
export WAYLAND_DISPLAY=wayland-0

# If Waydroid is running, restart it to use new display
if waydroid status 2>&1 | grep -q "Session.*RUNNING"; then
    echo "🔄 Restarting Waydroid session with Wayland display..."
    waydroid session stop
    sleep 3
fi

echo "🚀 Starting Waydroid with Wayland..."
XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-0 waydroid session start &
sleep 10

echo "✅ Headless VNC setup complete!"
echo ""
echo "🌐 VNC Access:"
echo "  - Host: localhost:5900"
echo "  - Password: (none - configure if needed)"
echo ""
echo "📝 Connect with VNC client:"
echo "  - macOS: open vnc://localhost:5900"
echo "  - Linux: vncviewer localhost:5900"
echo "  - Windows: Use TigerVNC/RealVNC to connect to localhost:5900"
echo ""
echo "📝 Service status:"
echo "  - Weston: systemctl status weston-headless"
echo "  - wayvnc: systemctl status wayvnc"
echo "  - Waydroid: waydroid status"
echo ""
echo "📝 Troubleshooting:"
echo "  - Check Weston logs: journalctl -u weston-headless -f"
echo "  - Check wayvnc logs: journalctl -u wayvnc -f"
echo "  - Check Waydroid logs: journalctl -u waydroid-container -f"
echo ""
echo "Next steps:"
echo "  1. Connect via VNC to see Waydroid UI"
echo "  2. Run: ./06-install-frida.sh (optional)"
echo "  3. Run: ./test-traffic.sh"
