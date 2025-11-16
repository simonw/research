# Waydroid MITM - Quick Start Guide

## TL;DR

**Don't use this. Use `../dockerify-android-mitm/` instead.**

Waydroid CAN intercept certificate-pinned app traffic, but it's more complex to set up than docker-android without offering meaningful advantages.

## If You Still Want Waydroid...

### 5-Minute Summary

```bash
# Ubuntu 22.04, as root:
cd native-app-traffic-capture/waydroid-mitm

# Run scripts in order
sudo bash scripts/01-setup-vm.sh          # 15-30 min
sudo bash scripts/02-init-waydroid.sh     # 10-15 min
sudo bash scripts/03-setup-mitm.sh        # 5 min
sudo bash scripts/04-install-certs.sh     # 5 min
sudo bash scripts/test-traffic.sh         # Test it works

# Optional
sudo bash scripts/05-setup-headless.sh    # VNC access
sudo bash scripts/06-install-frida.sh     # For stubborn apps
sudo bash scripts/test-youtube.sh         # E2E test
```

## Why NOT Waydroid?

| Feature | docker-android | Waydroid |
|---------|----------------|----------|
| Works for MITM? | ✅ YES | ✅ YES |
| Easy deployment? | ✅ YES | ❌ NO |
| Cloud-friendly? | ✅ YES | ❌ NO |
| Team-friendly? | ✅ YES | ❌ NO |
| CI/CD friendly? | ✅ YES | ❌ NO |

## What You Get with Waydroid

✅ **Better performance** (no virtualization)
✅ **Lower resource usage** (containers vs VMs)
✅ **Native root access** (no rooting needed)
✅ **Clean Linux integration**

❌ **Complex setup** (kernel modules + Wayland)
❌ **Linux-only** (no macOS/Windows Docker)
❌ **Headless complexity** (nested Wayland + VNC)
❌ **Less documented** (for MITM use case)

## Prerequisites

- Ubuntu 22.04 (bare metal or VM)
- Linux kernel 5.15+ with binder support
- 8GB RAM minimum
- Root access
- 2-3 hours for full setup

## Quick Commands

### Start Waydroid
```bash
sudo waydroid session start
```

### Stop Waydroid
```bash
sudo waydroid session stop
```

### Check Status
```bash
waydroid status
```

### Connect ADB
```bash
adb connect localhost:5555
adb devices
```

### View mitmproxy Flows
Open in browser: http://localhost:8081

### Check Traffic Capture
```bash
# View iptables rules
sudo iptables -t nat -L WAYDROID_MITM -n -v

# Check mitmproxy service
sudo systemctl status mitmproxy-waydroid

# View proxy logs
sudo journalctl -u mitmproxy-waydroid -f
```

### VNC Access (if configured)
```bash
# macOS
open vnc://localhost:5900

# Linux
vncviewer localhost:5900
```

### Frida Commands (if installed)
```bash
# List processes
frida-ps -U

# Attach to YouTube with unpinning
frida -U -f com.google.android.youtube -l frida-scripts/universal-ssl-unpin.js
```

## Troubleshooting

### Kernel modules not loading
```bash
sudo modprobe binder_linux
sudo modprobe ashmem_linux
lsmod | grep binder
```

### Waydroid not starting
```bash
journalctl -u waydroid-container -f
waydroid log
```

### No traffic captured
```bash
# Check iptables
sudo iptables -t nat -L WAYDROID_MITM -n -v

# Check cert installation
adb shell "ls /system/etc/security/cacerts/" | grep $(openssl x509 -inform PEM -subject_hash_old -in /root/.mitmproxy/mitmproxy-ca-cert.pem | head -1)
```

### ADB not connecting
```bash
adb kill-server
adb start-server
adb connect localhost:5555
```

## File Locations

- **Waydroid data**: `/var/lib/waydroid/`
- **mitmproxy cert**: `/root/.mitmproxy/mitmproxy-ca-cert.pem`
- **Certificate overlay**: `/var/lib/waydroid/overlay/system/etc/security/cacerts/`
- **Frida server**: `/data/local/tmp/frida-server` (in Android)
- **Config**: `/var/lib/waydroid/waydroid.cfg`

## Service Management

```bash
# mitmproxy
sudo systemctl start mitmproxy-waydroid
sudo systemctl stop mitmproxy-waydroid
sudo systemctl status mitmproxy-waydroid

# Waydroid container
sudo systemctl start waydroid-container
sudo systemctl stop waydroid-container

# Headless mode (if configured)
sudo systemctl start weston-headless
sudo systemctl start wayvnc
```

## Key Differences from docker-android

| Aspect | docker-android | Waydroid |
|--------|----------------|----------|
| Platform | Docker container | LXC container |
| Android Base | Emulator | LineageOS |
| Root Access | Via Magisk | Built-in |
| VNC | Built-in noVNC | Manual setup |
| Setup | `docker-compose up` | Multi-step scripts |
| Deployment | Any Docker host | Linux only |

## When to Actually Use Waydroid

1. **Local Linux workstation** for security research
2. **Need max performance** (no emulation overhead)
3. **Long-term research station** (one-time setup)
4. **Already familiar with Waydroid**

## Recommended Alternative

**Use `../dockerify-android-mitm/` instead**

```bash
cd ../dockerify-android-mitm
bash scripts/deploy-to-gcp.sh  # Done!
```

Much simpler, same results, better documented.

## Full Documentation

See [README.md](README.md) for:
- Detailed architecture
- Complete setup instructions
- Comprehensive troubleshooting
- Performance comparisons
- Security considerations
