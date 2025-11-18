#!/bin/bash
set -e

VM_NAME="waydroid-mitm"
ZONE="us-central1-a"
PROJECT="corsali-development"

echo "=========================================="
echo "Setting up Waydroid GUI Browser Access"
echo "=========================================="

# Install required packages
echo "[1/5] Installing packages..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} --command="
sudo apt update && \
sudo apt install -y weston wayvnc websockify novnc python3-websockify tigervnc-standalone-server xvfb x11vnc 2>&1 | tail -10
"

# Configure Weston with Xwayland
echo "[2/5] Configuring Weston..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} --command="
sudo bash -c 'cat > /etc/xdg/weston/weston.ini << EOF
[core]
backend=headless-backend.so
xwayland=true

[shell]
background-color=0x002040
EOF'
"

# Start Weston
echo "[3/5] Starting Weston..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} --command="
sudo systemctl restart weston-headless && \
sleep 5 && \
sudo systemctl status weston-headless --no-pager | head -10
"

# Try multiple VNC approaches
echo "[4/5] Setting up VNC access..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} --command="
# Create virtual X display
sudo bash -c 'cat > /etc/systemd/system/xvfb-waydroid.service << EOF
[Unit]
Description=Virtual X Server for Waydroid
After=weston-headless.service

[Service]
Type=simple
User=kahtaf
Environment=DISPLAY=:99
ExecStart=/usr/bin/Xvfb :99 -screen 0 1920x1080x24
Restart=always

[Install]
WantedBy=multi-user.target
EOF'

# X11VNC on virtual display
sudo bash -c 'cat > /etc/systemd/system/x11vnc-wayland.service << EOF
[Unit]
Description=X11VNC Server
After=xvfb-waydroid.service
Requires=xvfb-waydroid.service

[Service]
Type=simple
User=kahtaf
Environment=DISPLAY=:99
ExecStart=/usr/bin/x11vnc -display :99 -forever -shared -rfbport 5900 -passwd password -noxdamage
Restart=always

[Install]
WantedBy=multi-user.target
EOF'

sudo systemctl daemon-reload
sudo systemctl enable xvfb-waydroid x11vnc-wayland
sudo systemctl start xvfb-waydroid
sleep 3
sudo systemctl start x11vnc-wayland
sleep 3
"

# Start noVNC
echo "[5/5] Starting noVNC web interface..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} --command="
if sudo netstat -tlnp | grep -q ':5900.*LISTEN'; then
    sudo systemctl start novnc
    sleep 2
    echo '✅ VNC and noVNC started'
else
    echo '⚠️  VNC not listening, may need manual configuration'
fi
"

VM_IP=$(gcloud compute instances describe ${VM_NAME} --zone=${ZONE} --project=${PROJECT} --format="get(networkInterfaces[0].accessConfigs[0].natIP)")

echo ""
echo "=========================================="
echo "Setup Complete!"
echo "=========================================="
echo ""
echo "Access Waydroid GUI:"
echo "  🌐 noVNC: http://${VM_IP}:6080/vnc.html?autoconnect=true&resize=scale"
echo "  📄 Web: http://${VM_IP}:8080"
echo ""
echo "VNC Password: password"
echo ""

