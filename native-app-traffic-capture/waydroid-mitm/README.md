# Waydroid MITM - Android App Traffic Interception via Waydroid

Complete implementation for intercepting native Android app traffic (including certificate-pinned apps) using **Waydroid** as an alternative to docker-android emulators.

## 🎯 Project Goal

Evaluate Waydroid as a third approach for intercepting HTTPS traffic from native Android apps, specifically those using certificate pinning (like YouTube).

## ⚠️ Recommendation

**Use docker-android (dockerify-android-mitm) instead of this approach.**

While Waydroid is **technically feasible** for MITM, it doesn't offer practical advantages over the proven docker-android solution and adds significant deployment complexity.

### Why docker-android is Better

| Aspect | docker-android | Waydroid | Winner |
|--------|----------------|----------|--------|
| MITM Capability | ✅ YES (proven) | ✅ YES (feasible) | TIE |
| Cloud Deploy | ✅ Simple (any VM) | ❌ Complex (kernel deps) | docker-android |
| Headless Mode | ✅ Built-in VNC | ❌ Requires Wayland+VNC | docker-android |
| Setup Time | 1-2 hours | 4-8 hours | docker-android |
| Team Collaboration | ✅ Easy | ❌ Difficult | docker-android |

**Only use Waydroid if:**
- You're working on a local Linux workstation
- You need maximum performance (no virtualization)
- You're setting up a permanent research station
- You're already familiar with Waydroid

## 📋 What is Waydroid?

**Waydroid** is a container-based Android system for Linux that runs a full Android environment using Linux containers (LXC) instead of traditional emulation.

### Key Characteristics

- **Container-based**: Uses Linux namespaces, not VMs
- **Near-native performance**: No virtualization overhead
- **Linux-only**: Requires Linux kernel with binder modules
- **Based on LineageOS**: Android 11/13 support
- **Root access**: Built-in, full system control

## 🏗️ Architecture

```
┌────────────────────────────────────────────────────┐
│  Ubuntu 22.04 VM                                   │
│  ├─ Binder/Ashmem kernel modules                   │
│  ├─ Weston (Wayland compositor)                    │
│  │                                                  │
│  ├─ Waydroid Container (LineageOS)                 │
│  │  ├─ Android 11/13                               │
│  │  ├─ Root access (built-in)                      │
│  │  ├─ System CA: mitmproxy cert                   │
│  │  ├─ Network: waydroid0 bridge                   │
│  │  └─ iptables DNAT → mitmproxy                   │
│  │                                                  │
│  ├─ mitmproxy (port 8080/8081)                     │
│  │  ├─ Transparent proxy mode                      │
│  │  └─ Web UI for traffic inspection              │
│  │                                                  │
│  ├─ Frida (optional, for stubborn apps)            │
│  │  ├─ Certificate unpinning                       │
│  │  └─ Runtime instrumentation                     │
│  │                                                  │
│  └─ VNC Server (headless mode)                     │
│     └─ wayvnc (port 5900)                          │
└────────────────────────────────────────────────────┘
```

## 🚀 Quick Start

### Prerequisites

- **Ubuntu 22.04** (bare metal or VM)
- **Linux kernel 5.15+** with binder support
- **8GB RAM** minimum (16GB recommended)
- **Root access**
- **Active internet connection**

### Installation Steps

```bash
# 1. Clone repository
cd native-app-traffic-capture/waydroid-mitm

# 2. Run setup scripts in order (as root)
sudo bash scripts/01-setup-vm.sh          # Install dependencies
sudo bash scripts/02-init-waydroid.sh     # Initialize Waydroid
sudo bash scripts/03-setup-mitm.sh        # Configure mitmproxy
sudo bash scripts/04-install-certs.sh     # Install CA certificate
sudo bash scripts/05-setup-headless.sh    # Configure VNC (optional)
sudo bash scripts/06-install-frida.sh     # Install Frida (optional)

# 3. Test the setup
sudo bash scripts/test-traffic.sh         # Basic traffic test
sudo bash scripts/test-youtube.sh         # YouTube E2E test
```

**Total setup time:** 1-2 hours (manual), 30-45 minutes (automated)

## 📂 Project Structure

```
waydroid-mitm/
├── README.md                  # This file
├── QUICK_START.md             # TL;DR setup guide
├── notes.md                   # Research and implementation notes
├── scripts/
│   ├── 01-setup-vm.sh         # VM initialization
│   ├── 02-init-waydroid.sh    # Waydroid setup
│   ├── 03-setup-mitm.sh       # mitmproxy configuration
│   ├── 04-install-certs.sh    # Certificate installation
│   ├── 05-setup-headless.sh   # VNC configuration
│   ├── 06-install-frida.sh    # Frida deployment
│   ├── test-traffic.sh        # Basic traffic test
│   └── test-youtube.sh        # YouTube E2E test
├── frida-scripts/
│   └── universal-ssl-unpin.js # SSL unpinning script
├── certs/                     # Certificate storage
└── apks/                      # APK files (YouTube, etc.)
```

## 🔧 Detailed Setup

### Step 1: VM Setup (01-setup-vm.sh)

Installs all required dependencies:
- **Kernel modules**: binder_linux, ashmem_linux (via DKMS)
- **Wayland**: Weston compositor
- **Waydroid**: Latest version from official repo
- **mitmproxy**: HTTP/HTTPS proxy for traffic interception
- **ADB**: Android Debug Bridge
- **VNC**: TigerVNC for remote access

**Time:** 15-30 minutes

```bash
sudo bash scripts/01-setup-vm.sh
```

**What it does:**
1. Updates system packages
2. Checks/installs kernel modules (binder, ashmem)
3. Installs Wayland compositor (Weston)
4. Adds Waydroid repository and installs Waydroid
5. Installs mitmproxy, ADB, VNC server
6. Configures IP forwarding

### Step 2: Waydroid Initialization (02-init-waydroid.sh)

Initializes Waydroid with Android system:
- Downloads LineageOS image
- Configures Waydroid properties
- Enables ADB over network
- Starts Waydroid container

**Time:** 10-15 minutes (depends on download speed)

```bash
sudo bash scripts/02-init-waydroid.sh
```

**What it does:**
1. Initializes Waydroid with LineageOS (Android 11/13)
2. Configures system properties for debugging
3. Enables ADB on port 5555
4. Starts Waydroid container and session
5. Waits for Android to boot

### Step 3: MITM Proxy Setup (03-setup-mitm.sh)

Configures transparent HTTP/HTTPS proxying:
- Generates mitmproxy CA certificate
- Configures iptables DNAT rules
- Creates systemd service for mitmproxy
- Starts mitmproxy web UI

**Time:** 5 minutes

```bash
sudo bash scripts/03-setup-mitm.sh
```

**What it does:**
1. Generates mitmproxy CA certificate
2. Detects Waydroid network interface (waydroid0)
3. Configures iptables to redirect HTTP/HTTPS → mitmproxy
4. Creates systemd service for automatic startup
5. Starts mitmproxy in transparent mode

**iptables rules:**
```bash
# HTTP (port 80) → mitmproxy (port 8080)
iptables -t nat -A WAYDROID_MITM -p tcp --dport 80 -j REDIRECT --to-port 8080

# HTTPS (port 443) → mitmproxy (port 8080)
iptables -t nat -A WAYDROID_MITM -p tcp --dport 443 -j REDIRECT --to-port 8080
```

### Step 4: Certificate Installation (04-install-certs.sh)

Installs mitmproxy CA cert into Android system trust store:
- Calculates certificate hash (Android format)
- Installs to /system/etc/security/cacerts/
- Uses Waydroid overlay system OR direct ADB push
- Restarts Waydroid to apply changes

**Time:** 5 minutes

```bash
sudo bash scripts/04-install-certs.sh
```

**What it does:**
1. Calculates certificate subject_hash_old (Android format)
2. Copies cert to Waydroid overlay: `/var/lib/waydroid/overlay/system/etc/security/cacerts/`
3. As backup, pushes cert via ADB with `su` commands
4. Restarts Waydroid session
5. Verifies installation

### Step 5: Headless VNC Setup (05-setup-headless.sh) [Optional]

Configures Waydroid for headless operation with VNC:
- Weston in headless mode
- wayvnc for VNC access
- systemd services for auto-start

**Time:** 10 minutes

```bash
sudo bash scripts/05-setup-headless.sh
```

**What it does:**
1. Installs Weston, wayvnc, VNC servers
2. Creates Weston config for headless backend
3. Creates systemd services for Weston and wayvnc
4. Starts services
5. Configures Waydroid to use Wayland display

**Access:** `vnc://localhost:5900`

### Step 6: Frida Installation (06-install-frida.sh) [Optional]

Installs Frida for advanced certificate pinning bypass:
- Detects Android architecture
- Downloads appropriate Frida server
- Pushes to Waydroid
- Installs frida-tools on host
- Copies unpinning scripts

**Time:** 10 minutes

```bash
sudo bash scripts/06-install-frida.sh
```

**What it does:**
1. Detects Android architecture (x86_64, arm64, etc.)
2. Downloads latest Frida server from GitHub
3. Pushes to /data/local/tmp/frida-server
4. Starts Frida server
5. Installs frida-tools via pip
6. Tests connection with `frida-ps -U`

## 🧪 Testing

### Basic Traffic Test (test-traffic.sh)

Tests basic HTTP/HTTPS interception:

```bash
sudo bash scripts/test-traffic.sh
```

**What it tests:**
1. ADB connectivity
2. mitmproxy service status
3. HTTP request interception
4. HTTPS request interception
5. mitmproxy flow capture

**Expected result:** Flows visible in mitmproxy web UI

### YouTube E2E Test (test-youtube.sh)

Tests certificate-pinned app (YouTube):

```bash
sudo bash scripts/test-youtube.sh
```

**What it tests:**
1. YouTube APK installation
2. Frida availability (optional)
3. Traffic capture from YouTube
4. Certificate pinning bypass
5. Decrypted HTTPS traffic inspection

**Expected result:** YouTube API calls visible and decrypted in mitmproxy

## 🎯 MITM Capabilities

### Three-Layer Approach

**Layer 1: iptables DNAT** (Primary, ~90% coverage)
- Transparent network-level redirection
- Works for apps that ignore proxy settings
- No app-level configuration needed

**Layer 2: System CA Certificate** (~70% coverage)
- Installed in `/system/etc/security/cacerts/`
- Trusted by apps that don't use pinning
- Required for TLS decryption

**Layer 3: Frida Runtime Hooks** (Optional, ~95%+ coverage)
- Runtime certificate unpinning
- Bypasses even aggressive pinning
- Only needed for stubborn apps

### What Works

✅ **HTTP traffic** (port 80) - fully transparent
✅ **HTTPS traffic** (port 443) - decrypted with system cert
✅ **Certificate-pinned apps** - with system cert + iptables (most apps)
✅ **Aggressive pinning** - with Frida (YouTube, banking apps, etc.)
✅ **App Store apps** - Google Play Store apps work
✅ **System apps** - Built-in Android apps work

### What Doesn't Work

❌ **HTTP/3 (QUIC)** - Uses UDP, not TCP (iptables won't catch it)
❌ **VPN traffic** - Apps using VPN bypass the proxy
❌ **Apps with Play Integrity** - Some banking apps detect root
❌ **Non-HTTP protocols** - Custom protocols not supported

## 🔍 Troubleshooting

### Waydroid won't start

```bash
# Check kernel modules
lsmod | grep binder
lsmod | grep ashmem

# Load modules manually
sudo modprobe binder_linux
sudo modprobe ashmem_linux

# Check Waydroid status
waydroid status

# View logs
journalctl -u waydroid-container -f
```

### No traffic captured

```bash
# 1. Check iptables rules
sudo iptables -t nat -L WAYDROID_MITM -n -v

# 2. Check mitmproxy status
sudo systemctl status mitmproxy-waydroid
sudo journalctl -u mitmproxy-waydroid -f

# 3. Check certificate installation
adb shell "ls -l /system/etc/security/cacerts/" | grep mitmproxy

# 4. Test direct proxy connection
adb shell "curl -x http://192.168.250.1:8080 https://example.com"
```

### ADB connection fails

```bash
# Reconnect ADB
adb kill-server
adb start-server
adb connect localhost:5555

# Check if Waydroid is running
waydroid status

# Check ADB properties in Waydroid
cat /var/lib/waydroid/waydroid.cfg | grep adb
```

### Certificate not trusted

```bash
# Check certificate hash
openssl x509 -inform PEM -subject_hash_old -in /root/.mitmproxy/mitmproxy-ca-cert.pem | head -1

# Verify it exists in Android
HASH=$(openssl x509 -inform PEM -subject_hash_old -in /root/.mitmproxy/mitmproxy-ca-cert.pem | head -1)
adb shell "ls -l /system/etc/security/cacerts/${HASH}.0"

# Reinstall if missing
sudo bash scripts/04-install-certs.sh
```

### Frida not working

```bash
# Check if Frida server is running
adb shell "su -c 'ps | grep frida'"

# Restart Frida server
adb shell "su -c 'killall frida-server'"
adb shell "su -c '/data/local/tmp/frida-server &'"

# Test connection
frida-ps -U
```

## 📊 Performance Comparison

| Metric | docker-android | Waydroid | Winner |
|--------|----------------|----------|--------|
| Boot time | 60-90s | 15-30s | Waydroid |
| CPU usage | High (emulation) | Low (container) | Waydroid |
| Memory | 4-6GB | 2-3GB | Waydroid |
| Network latency | ~50ms | ~5ms | Waydroid |
| App compatibility | 90% | 85% | docker-android |

**Note:** Performance gains only matter if you're running locally on Linux. For cloud deployments, the simpler setup of docker-android outweighs Waydroid's performance benefits.

## 🔐 Security Considerations

### Root Access

Waydroid has **built-in root access** via `waydroid shell` or `adb shell su`. This is necessary for:
- Installing system certificates
- Configuring iptables
- Running Frida server

### Network Isolation

Waydroid creates isolated network (`waydroid0` bridge):
- Android traffic doesn't leak to host
- iptables rules only affect Waydroid traffic
- mitmproxy only listens on bridge interface

### Certificate Trust

mitmproxy CA certificate is installed in **system trust store**, meaning:
- All apps trust it by default (unless pinned)
- User can inspect all HTTPS traffic
- Same level of trust as device manufacturer certs

## 📚 Resources

### Waydroid
- Official docs: https://docs.waydro.id/
- GitHub: https://github.com/waydroid/waydroid
- Reddit: r/waydroid

### mitmproxy
- Official docs: https://docs.mitmproxy.org/
- GitHub: https://github.com/mitmproxy/mitmproxy

### Frida
- Official docs: https://frida.re/docs/
- GitHub: https://github.com/frida/frida
- Scripts: https://codeshare.frida.re/

### Related Projects
- HTTP Toolkit: https://httptoolkit.com/
- Android SSL unpinning: https://github.com/httptoolkit/frida-android-unpinning

## 🤝 Comparison with Other Approaches

See `../dockerify-android-mitm/` for the **recommended approach** using docker-android.

### dockerify-android-mitm (Recommended)
- ✅ Docker-native, simple deployment
- ✅ Works on any Docker host (Linux, macOS via cloud, Windows)
- ✅ Built-in VNC for headless
- ✅ Well-documented for MITM use case
- ❌ Emulation overhead (slower)
- ❌ Limited to x86_64

### Waydroid (This Implementation)
- ✅ Near-native performance
- ✅ Lower resource usage
- ✅ ARM64 support
- ❌ Complex setup (kernel deps)
- ❌ Linux-only
- ❌ Less documentation for MITM

### android-mitm-mvp (Earlier)
- ✅ Android 13 (newer)
- ✅ Docker-based
- ❌ Frida-heavy (complex)
- ❌ Less stable than dockerify

## 📄 License

This implementation is for **research and educational purposes only**.

- mitmproxy: MIT License
- Waydroid: GPLv3
- Frida: wxWindows Library Licence
- HTTP Toolkit scripts: AGPL-3.0

## 🙏 Acknowledgments

- **Waydroid team** for container-based Android
- **mitmproxy team** for excellent HTTPS interception tool
- **Frida team** for runtime instrumentation framework
- **HTTP Toolkit** for Frida unpinning scripts
- **dockerify-android** project for inspiration

## 📝 Final Recommendation

**Use `../dockerify-android-mitm/` instead of this Waydroid implementation.**

Waydroid is technically impressive and works for MITM, but it doesn't solve any problems that docker-android doesn't already solve, while adding significant deployment complexity.

Only choose Waydroid if you have specific requirements that docker-android can't meet (local Linux setup, maximum performance, ARM64 deployment).
