#!/bin/bash
set -euo pipefail

# Waydroid MITM - VM Setup Script
# This script sets up an Ubuntu 22.04 VM with all required dependencies for Waydroid

echo "================================================"
echo "Waydroid MITM - VM Setup"
echo "================================================"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "❌ Please run as root (sudo)"
    exit 1
fi

echo "📦 Updating system packages..."
apt-get update
apt-get upgrade -y

echo "📦 Installing base dependencies..."
apt-get install -y \
    curl \
    wget \
    git \
    python3 \
    python3-pip \
    build-essential \
    linux-headers-$(uname -r) \
    dkms \
    ca-certificates \
    gnupg \
    lsb-release

echo "🔧 Checking kernel module support..."
# Check for binder support
if ! modprobe binder_linux 2>/dev/null; then
    echo "⚠️  binder_linux module not found, installing via DKMS..."

    # Install binder and ashmem via anbox-modules-dkms
    apt-get install -y linux-headers-generic

    # Clone and build binder modules
    cd /tmp
    if [ ! -d "anbox-modules" ]; then
        git clone https://github.com/anbox/anbox-modules.git
    fi
    cd anbox-modules

    # Install via DKMS
    cp -r binder /usr/src/anbox-binder-1
    cp -r ashmem /usr/src/anbox-ashmem-1

    # Create dkms.conf for binder
    cat > /usr/src/anbox-binder-1/dkms.conf << 'EOF'
PACKAGE_NAME="anbox-binder"
PACKAGE_VERSION="1"
MAKE[0]="make KVERSION=$kernelver"
CLEAN="make clean"
BUILT_MODULE_NAME[0]="binder_linux"
DEST_MODULE_LOCATION[0]="/kernel/drivers/staging"
AUTOINSTALL="yes"
EOF

    # Create dkms.conf for ashmem
    cat > /usr/src/anbox-ashmem-1/dkms.conf << 'EOF'
PACKAGE_NAME="anbox-ashmem"
PACKAGE_VERSION="1"
MAKE[0]="make KVERSION=$kernelver"
CLEAN="make clean"
BUILT_MODULE_NAME[0]="ashmem_linux"
DEST_MODULE_LOCATION[0]="/kernel/drivers/staging"
AUTOINSTALL="yes"
EOF

    # Build and install
    dkms add -m anbox-binder -v 1
    dkms build -m anbox-binder -v 1
    dkms install -m anbox-binder -v 1

    dkms add -m anbox-ashmem -v 1
    dkms build -m anbox-ashmem -v 1
    dkms install -m anbox-ashmem -v 1

    # Load modules
    modprobe binder_linux
    modprobe ashmem_linux

    # Make persistent
    echo "binder_linux" >> /etc/modules
    echo "ashmem_linux" >> /etc/modules

    echo "✅ Kernel modules installed"
else
    echo "✅ binder_linux module already available"
    modprobe ashmem_linux 2>/dev/null || echo "⚠️  ashmem not loaded (may not be needed)"
fi

echo "🖥️  Installing Wayland compositor (Weston)..."
apt-get install -y \
    weston \
    xwayland \
    wayland-protocols \
    libwayland-dev

echo "🔧 Installing Waydroid dependencies..."
apt-get install -y \
    lxc \
    python3 \
    python3-gi \
    python3-dbus \
    python3-gbulb \
    gir1.2-glib-2.0

echo "📥 Installing Waydroid..."
# Add Waydroid repository
if [ ! -f /etc/apt/sources.list.d/waydroid.list ]; then
    curl https://repo.waydro.id | sh
fi

apt-get update
apt-get install -y waydroid

echo "📥 Installing mitmproxy..."
# Install via pip to get latest version
pip3 install mitmproxy

echo "📥 Installing ADB..."
apt-get install -y android-tools-adb android-tools-fastboot

echo "📥 Installing VNC server for headless mode..."
apt-get install -y \
    tigervnc-standalone-server \
    tigervnc-common \
    x11vnc \
    xvfb

echo "🔧 Configuring IP forwarding..."
# Enable IP forwarding (required for iptables DNAT)
echo "net.ipv4.ip_forward=1" >> /etc/sysctl.conf
sysctl -p

echo "🔧 Creating mitmproxy certificate directory..."
mkdir -p /root/.mitmproxy
chmod 700 /root/.mitmproxy

echo "✅ VM setup complete!"
echo ""
echo "Next steps:"
echo "  1. Run: ./02-init-waydroid.sh"
echo "  2. Run: ./03-setup-mitm.sh"
echo "  3. Run: ./04-install-certs.sh"
echo ""
echo "📝 System info:"
echo "  - Kernel: $(uname -r)"
echo "  - Waydroid: $(waydroid --version 2>/dev/null || echo 'installed')"
echo "  - mitmproxy: $(mitmproxy --version | head -1)"
echo "  - ADB: $(adb version | head -1)"
