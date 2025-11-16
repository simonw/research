# Waydroid MITM Feasibility Assessment

## Executive Summary

**Verdict: YES - Technically Feasible, but NOT Recommended over docker-android for most use cases**

Waydroid is a container-based Android runtime for Linux that can be used for MITM traffic interception of certificate-pinned apps. However, it offers limited advantages over docker-android for this specific use case while introducing deployment complexity, particularly for cloud/remote scenarios.

## What is Waydroid?

### Architecture

Waydroid is a container-based approach to running Android on Linux systems, fundamentally different from traditional virtualization:

- **Container Technology**: Uses Linux namespaces (user, pid, uts, net, mount, ipc) instead of full virtualization
- **Android Base**: Minimal customized Android image based on LineageOS (Android 11/13)
- **Performance**: Near-native performance with direct hardware access
- **Host Integration**: Runs directly on Linux kernel without virtualization overhead

### Key Characteristics

- **No Virtualization**: Container isolation rather than VM-based emulation
- **Linux-Only**: Requires Linux host with specific kernel modules
- **Wayland Requirement**: Needs Wayland compositor for display
- **Python-Based**: Management through Python CLI tool
- **Active Project**: 10.3k GitHub stars, actively maintained

## System Requirements

### Kernel Requirements

Waydroid has strict kernel module dependencies:

**Required Kernel Configurations:**
```
CONFIG_ANDROID=y
CONFIG_ANDROID_BINDER_IPC=m or =y
CONFIG_ANDROID_BINDERFS or CONFIG_ANDROID_BINDER_DEVICES
CONFIG_PSI=y (Process State Information)
CONFIG_BLK_DEV_LOOP=m (Loop devices)
IPv6 networking built-in (even for IPv4 connectivity)
```

**Kernel Versions:**
- Works on: linux, linux-lts, linux-zen kernels
- Custom kernels may require DKMS modules
- Some compatibility issues with kernels 5.18+ (ibt=off workaround needed)

### System Requirements

- **Operating System**: Any GNU/Linux-based platform
- **Display**: Wayland compositor (can use nested session in X11)
- **Network**: Standard Linux networking stack
- **Storage**: Space for Android system image and apps
- **Architecture**: x86/x64 (ARM translation available for ARM apps)

## MITM Capabilities Assessment

### 1. Root/System Access

**Status: EXCELLENT**

Waydroid provides comprehensive system access:

- **Direct Shell Access**: `sudo waydroid shell`
- **ADB Connectivity**: `adb connect <IP>:5555`
- **Host File System Access**: `/var/lib/waydroid/rootfs` and `/var/lib/waydroid/overlay`
- **No Rooting Required**: Full system access by default
- **Persistent Modifications**: Overlay filesystem for system changes

### 2. Certificate Management

**Status: FULLY SUPPORTED**

**System Certificate Installation:**
- Path: `/system/etc/security/cacerts/`
- Host overlay path: `/var/lib/waydroid/overlay/system/etc/security/cacerts/`
- Remount capability: `mount -o remount,rw /var/lib/waydroid/rootfs`

**Android Version Considerations:**
- Android 7+: System-level CA required (user certs not trusted by apps)
- Android 10+: System-as-root approach, use overlay filesystem
- Android 14: May require updated certificate installation techniques

**mitmproxy Integration:**
- Certificate location: `~/.mitmproxy/mitmproxy-ca-cert.pem`
- Conversion to Android format required (hash-based filename)
- Installation via overlay filesystem proven to work

### 3. Network Configuration

**Status: EXCELLENT**

**Network Architecture:**
- Dedicated network interface: `waydroid0`
- NAT bridge networking model
- Container gets own IP (typically 192.168.250.x range)
- Gateway: 192.168.250.1 (host's waydroid0 interface)
- DNS on port 53, DHCP on port 67

**Proxy Configuration:**

*System-wide proxy:*
```bash
adb shell settings put global http_proxy "172.17.0.1:8888"
```

*Remove proxy:*
```bash
adb shell settings put global http_proxy :0
```

**Transparent Proxy with iptables:**

```bash
# Redirect HTTP/HTTPS traffic to mitmproxy
sudo iptables -t nat -A PREROUTING -i waydroid0 -p tcp --dport 80 -j REDIRECT --to-port 8080
sudo iptables -t nat -A PREROUTING -i waydroid0 -p tcp --dport 443 -j REDIRECT --to-port 8080

# Enable IP forwarding
sudo sysctl -w net.ipv4.ip_forward=1
sudo sysctl -w net.ipv6.conf.all.forwarding=1
sudo sysctl -w net.ipv4.conf.all.send_redirects=0
```

**Advantages:**
- Clean network isolation via dedicated interface
- Full iptables REDIRECT/DNAT support
- Transparent proxy setup proven in production
- Users successfully running mitmproxy with Waydroid

### 4. Certificate Pinning Bypass

**Status: FULLY SUPPORTED**

**Frida Compatibility:**
- Frida server runs on Waydroid (standard Android environment)
- Universal SSL pinning bypass scripts compatible
- HTTPToolkit has Waydroid-tested unpinning scripts
- Same techniques as physical devices/emulators

**Available Tools:**
- **Frida**: Runtime instrumentation for pinning bypass
- **apk-mitm**: Automatic pinning removal from APKs
- **objection**: Frida-based runtime toolkit
- **mitmproxy/android-unpinner**: Automated unpinning tool

**Proven Approach:**
1. Install Frida server in Waydroid
2. Use universal pinning bypass scripts (e.g., sowdust/universal-android-ssl-pinning-bypass-2)
3. Combine with mitmproxy transparent proxy
4. System CA + Frida = comprehensive coverage

### 5. App Installation & Compatibility

**Status: GOOD with caveats**

**APK Installation:**
```bash
# Direct APK install
waydroid app install <package.apk>

# Via package name
waydroid install com.xxx.yyy

# Via ADB
adb install <package.apk>
```

**Google Play Support:**
- Install with GAPPS Android type during setup
- Requires device certification at https://www.google.com/android/uncertified
- Aurora Store (F-droid) as alternative
- Some apps may be marked incompatible with "emulator"

**ARM App Compatibility:**
- Base Waydroid: x86/x64 only
- ARM translation available via libhoudini (Intel) or libndk (AMD)
- Installation via waydroid_script tool:
  ```bash
  git clone https://github.com/casualsnek/waydroid_script
  cd waydroid_script
  python3 -m venv venv
  venv/bin/pip install -r requirements.txt
  sudo venv/bin/python3 main.py install libndk  # or libhoudini
  ```

**Limitations:**
- ARM-only apps require translation layer
- Some games with native libraries may not work
- Play Store may block certain apps on "emulator"
- Architecture mismatch most common compatibility issue

**For MITM Use Case:**
- Most network/communication apps work well
- Target apps typically available via Play Store or APK
- ARM translation handles most compatibility issues
- Better app compatibility than traditional emulators

## Deployment Options

### Local Linux Deployment

**Status: EXCELLENT**

Best-case scenario for Waydroid:
- Native performance on Linux workstation
- Straightforward installation on major distributions
- Full desktop integration
- Easy development and testing workflow

**Installation Example (Ubuntu/Debian):**
```bash
# Install prerequisites
sudo apt install curl ca-certificates

# Add Waydroid repository
curl https://repo.waydro.id | sudo bash

# Install Waydroid
sudo apt install waydroid

# Initialize (choose GAPPS or VANILLA)
sudo waydroid init

# Start service
sudo systemctl start waydroid-container

# Launch UI
waydroid show-full-ui
```

### Cloud/VM Deployment

**Status: COMPLEX**

**Requirements:**
- Linux VM with kernel binder modules
- Nested virtualization or bare metal
- virtio-gpu with 3D acceleration
- Wayland compositor setup
- Remote access configuration

**Challenges:**
- Not all cloud providers support required kernel modules
- Wayland compositor adds complexity
- Remote display requires VNC/RDP setup
- More complex than docker-android

**VM Configuration:**
- EGL-headless for dedicated VM servers
- Qemu with virtio-gpu 3D acceleration
- Nested Wayland session in X11 possible
- Requires careful kernel configuration

### Docker Deployment

**Status: EXPERIMENTAL/LIMITED**

**Current State:**
- NOT designed for Docker containers
- Requires privileged mode + LXC alongside Docker
- Security concerns with privileged containers
- Experimental project: tyzbit/docker-waydroid

**Why Docker is Problematic:**
- Waydroid uses LXC containers internally
- Running LXC inside Docker is complex
- Requires privileged mode for kernel access
- Wayland compositor in Docker adds overhead

**Alternatives:**
- Distrobox: Run Waydroid inside application container
- docker-waydroid: Experimental Docker baseimage with web/VNC access
- Native docker-android: Purpose-built for containers

### Headless/Remote Access

**Status: COMPLEX**

**Wayland Challenge:**
- Waydroid requires Wayland compositor
- Wayland is not a network protocol (unlike X11)
- Remote access requires additional layers

**Solutions:**

*Weston with X11 Backend:*
```bash
export WAYLAND_DISPLAY=mysocket
weston --socket=$WAYLAND_DISPLAY --backend=x11-backend.so --width=1920 --height=1080
waydroid show-full-ui
```

*VNC Access:*
- wayvnc for wlroots-based compositors
- Xvfb + X11VNC for traditional approach
- docker-waydroid provides web browser access

*RDP Access:*
- Ansible role available for RDP configuration
- More complex than VNC

**Comparison:**
- docker-android: Built-in VNC, straightforward
- Waydroid: Requires compositor + VNC layer, complex

## Performance Comparison

### Waydroid Performance

**Advantages:**
- **No Virtualization Overhead**: Container-based, near-native speed
- **Direct Hardware Access**: GPU, CPU, memory without abstraction
- **Efficient Resources**: Lightweight compared to VMs
- **Fast Startup**: Seconds vs minutes for emulators

**Benchmarks:**
- Near-native CPU performance
- Full GPU acceleration support
- Lower memory footprint than VMs
- Faster app launch times

### vs docker-android

| Metric | Waydroid | docker-android |
|--------|----------|----------------|
| CPU Performance | Excellent (native) | Good (KVM acceleration) |
| GPU Performance | Excellent (direct) | Good (with KVM) |
| Memory Overhead | Low | Low-Medium |
| Startup Time | Fast | Medium |
| Resource Usage | Minimal | Moderate |
| Portability | Linux-only | Any Docker host |
| Deployment Complexity | Medium-High | Low |

**Key Difference:**
- Waydroid: Best raw performance on Linux
- docker-android: Better portability and deployment simplicity

## Comparison Matrix: Waydroid vs docker-android

### For MITM Traffic Interception

| Feature | Waydroid | docker-android | Winner |
|---------|----------|----------------|--------|
| **Root Access** | Built-in, full access | Built-in, full access | Tie |
| **System Cert Install** | Via overlay FS | Standard /system | docker-android (simpler) |
| **Network Control** | iptables on waydroid0 | iptables on container | Tie |
| **Transparent Proxy** | Full support | Full support | Tie |
| **Frida Support** | Yes | Yes | Tie |
| **Performance** | Excellent | Good | Waydroid |
| **Setup Complexity** | Moderate-High | Low-Moderate | docker-android |
| **Documentation** | Desktop-focused | MITM/testing-focused | docker-android |
| **Docker Native** | No (experimental) | Yes | docker-android |
| **Headless Mode** | Complex setup | Built-in VNC | docker-android |
| **Cloud Deploy** | Complex | Simple | docker-android |
| **CI/CD Integration** | Difficult | Easy | docker-android |
| **Team Collaboration** | Challenging | Straightforward | docker-android |
| **ARM App Support** | Via translation | Via translation | Tie |
| **App Compatibility** | Good | Good | Tie |
| **Linux Requirement** | Yes (kernel deps) | No | docker-android |

### Use Case Recommendations

**Choose Waydroid When:**
- Working locally on Linux workstation
- Maximum performance is critical
- Single-developer research setup
- Desktop integration desired
- Already familiar with Waydroid

**Choose docker-android When:**
- Cloud/remote deployment needed
- Team collaboration required
- CI/CD integration desired
- Headless automation at scale
- Cross-platform support needed
- Simpler deployment preferred
- Documentation/examples important

## Technical Feasibility for MITM

### Complete MITM Setup (Theoretical)

**1. Install Waydroid:**
```bash
# On Ubuntu/Debian
curl https://repo.waydro.id | sudo bash
sudo apt install waydroid
sudo waydroid init -s GAPPS
```

**2. Install System Certificate:**
```bash
# Convert mitmproxy cert to Android format
openssl x509 -inform PEM -subject_hash_old -in ~/.mitmproxy/mitmproxy-ca-cert.pem | head -1
# Output: 8d6603eb (example hash)

# Copy to overlay
sudo cp ~/.mitmproxy/mitmproxy-ca-cert.pem \
  /var/lib/waydroid/overlay/system/etc/security/cacerts/8d6603eb.0

# Set permissions
sudo chmod 644 /var/lib/waydroid/overlay/system/etc/security/cacerts/8d6603eb.0
```

**3. Configure Transparent Proxy:**
```bash
# Get Waydroid IP range
ip addr show waydroid0

# Setup iptables redirect
sudo iptables -t nat -A PREROUTING -i waydroid0 -p tcp --dport 80 -j REDIRECT --to-port 8080
sudo iptables -t nat -A PREROUTING -i waydroid0 -p tcp --dport 443 -j REDIRECT --to-port 8080

# Enable forwarding
sudo sysctl -w net.ipv4.ip_forward=1
```

**4. Start mitmproxy:**
```bash
# Transparent mode
mitmproxy --mode transparent --showhost
```

**5. Install Frida:**
```bash
# Download Frida server for Android
wget https://github.com/frida/frida/releases/download/16.1.9/frida-server-16.1.9-android-x86_64.xz
unxz frida-server-16.1.9-android-x86_64.xz

# Push to Waydroid
adb push frida-server-16.1.9-android-x86_64 /data/local/tmp/frida-server
adb shell chmod 755 /data/local/tmp/frida-server
adb shell /data/local/tmp/frida-server &
```

**6. Bypass Certificate Pinning:**
```bash
# Use universal pinning bypass
frida --codeshare sowdust/universal-android-ssl-pinning-bypass-2 -U -f com.target.app --no-pause
```

### Potential Blockers

**1. Kernel Module Requirements**
- **Blocker Level**: HIGH for cloud deployment
- **Impact**: Won't run without proper kernel modules
- **Mitigation**: Use Linux VM with appropriate kernel, or choose docker-android

**2. Wayland Compositor Requirement**
- **Blocker Level**: MEDIUM for headless
- **Impact**: Adds complexity to remote/headless setups
- **Mitigation**: Use Weston with X11 backend, or choose docker-android

**3. Docker Incompatibility**
- **Blocker Level**: HIGH for container-based workflows
- **Impact**: Not Docker-native, experimental support only
- **Mitigation**: Use native Linux installation, or choose docker-android

**4. ARM App Compatibility**
- **Blocker Level**: LOW
- **Impact**: Need translation layer for ARM apps
- **Mitigation**: Install libhoudini/libndk (same as docker-android)

**5. Android 14 Certificate Installation**
- **Blocker Level**: MEDIUM for Android 14+
- **Impact**: Traditional cert installation broken
- **Mitigation**: Use updated techniques, or stay on Android 11/13

### Advantages Over docker-android

1. **Superior Performance**
   - No virtualization overhead
   - Direct hardware access
   - Better GPU acceleration
   - Faster than emulator-based solutions

2. **Native Root Access**
   - Full system access by default
   - Direct file system modifications via host
   - No rooting process needed
   - Overlay system for persistent changes

3. **Clean Linux Integration**
   - Runs directly on Linux kernel
   - Clean network isolation (waydroid0)
   - Straightforward iptables integration
   - Better for local development

4. **App Compatibility**
   - More like real device than emulator
   - Better support for some apps
   - Active development and updates

### Disadvantages vs docker-android

1. **Deployment Complexity**
   - Requires Linux host with specific kernel modules
   - Wayland compositor requirement
   - Not Docker-native (experimental only)
   - Headless setup significantly more complex

2. **Cloud/Remote Deployment**
   - docker-android purpose-built for containers
   - docker-android has VNC built-in and ready
   - Waydroid needs nested Wayland session
   - docker-android better for CI/CD pipelines

3. **Portability & Collaboration**
   - Waydroid: Linux-only, kernel-dependent
   - docker-android: Works anywhere Docker runs
   - docker-android easier to share and replicate
   - docker-android better for team environments

4. **Documentation & Community**
   - docker-android: MITM/testing-focused docs
   - Waydroid: Desktop/gaming-focused docs
   - Fewer MITM-specific examples for Waydroid
   - docker-android has established testing patterns

5. **Automation**
   - docker-android integrates easily with CI/CD
   - Waydroid requires more custom scripting
   - docker-android better for automated testing
   - Waydroid better for manual research

## Real-World MITM Examples

### Documented Use Cases

1. **Julien Duponchelle - "Use a proxy with Waydroid"**
   - Blog post demonstrating mitmproxy + Waydroid
   - iptables transparent proxy configuration
   - Certificate installation process
   - Successfully intercepted HTTPS traffic

2. **Community Reports**
   - Users running mitmproxy with Waydroid
   - iptables REDIRECT working as expected
   - Certificate installation via overlay confirmed
   - Frida scripts compatible

3. **Tor Transparent Proxy**
   - GitHub issue #1144: Torify entire Waydroid container
   - 6-8 iptables commands for complete routing
   - Demonstrates network control capabilities

### HTTPToolkit Integration

HTTPToolkit (creators of Frida unpinning scripts) has tested with Waydroid:
- Frida-based interception and unpinning
- Automatic certificate installation
- Transparent proxy setup
- Confirmed compatibility

## Final Recommendations

### For MITM Traffic Interception Use Case

**Recommendation: Use docker-android unless you have specific reasons for Waydroid**

**Why docker-android is Better for Most Scenarios:**

1. **Simpler Deployment**: Docker-native, works anywhere
2. **Better Documentation**: MITM/testing-focused examples
3. **Easier Sharing**: Docker images easily distributed
4. **Headless Built-in**: VNC ready out of box
5. **Cloud-Friendly**: Runs in any cloud environment
6. **Team-Ready**: Standardized, reproducible setup
7. **CI/CD Integration**: Easy to automate
8. **Cross-Platform**: Works on macOS/Windows/Linux Docker hosts

**When Waydroid Makes Sense:**

1. **Local Linux Development**: If you're on Linux and working locally
2. **Performance Critical**: Need absolute best performance
3. **Long-Term Research**: Setting up permanent research workstation
4. **Desktop Integration**: Want Android apps on Linux desktop
5. **Already Using**: If Waydroid is already set up

### Migration Path from docker-android MVP

**If You Have Working docker-android:**
- **Keep it**: It's the better solution for deployment
- **Don't migrate** unless you have specific performance needs
- **Waydroid doesn't solve**: Any problems docker-android has

**If Starting Fresh:**
- **Choose docker-android** for easier deployment
- **Choose Waydroid** only if on Linux and need max performance

### Decision Matrix

**Choose docker-android if any of these are true:**
- Need to deploy to cloud/VM
- Working with a team
- Need headless automation
- Want simpler setup
- Need CI/CD integration
- Working on macOS/Windows (Docker Desktop)

**Choose Waydroid only if all of these are true:**
- Working on Linux workstation
- Local-only deployment
- Single developer
- Performance is critical
- Comfortable with complexity

## Conclusion

**Technical Feasibility: YES**

Waydroid is technically feasible for MITM traffic interception of certificate-pinned Android apps. All required capabilities are present:

- Full root/system access
- System certificate installation
- Network traffic control with iptables
- Transparent proxy support
- Frida compatibility for pinning bypass
- Proven by real-world examples

**Practical Recommendation: Use docker-android**

Despite technical feasibility, Waydroid is NOT recommended over docker-android for MITM use cases because:

1. **No Significant Advantages**: For MITM, both solutions work equally well
2. **Higher Complexity**: Waydroid deployment is significantly more complex
3. **Worse Portability**: Linux-only with kernel dependencies
4. **Headless Challenges**: Built-in VNC in docker-android vs complex setup in Waydroid
5. **Better Documentation**: docker-android has more MITM-focused resources
6. **Proven Track Record**: Your existing docker-android MVP already works

**When Waydroid Excels:**

Waydroid shines for:
- Desktop Android app integration on Linux
- High-performance gaming/graphics applications
- Long-term Linux workstation setups
- Situations where near-native performance matters

**When docker-android Excels:**

docker-android is superior for:
- MITM traffic interception and security testing
- Cloud/VM deployment scenarios
- Team collaboration and sharing
- Automated testing and CI/CD
- Cross-platform development

**Final Verdict:**

For your specific use case of MITM traffic interception of certificate-pinned Android apps, stick with docker-android. Waydroid would add complexity without meaningful benefits. The existing docker-android MVP is the right tool for the job.

## References

### Documentation
- Official Waydroid Docs: https://docs.waydro.id/
- Waydroid GitHub: https://github.com/waydroid/waydroid
- ArchWiki Waydroid: https://wiki.archlinux.org/title/Waydroid

### MITM Resources
- Julien Duponchelle - Waydroid Proxy Setup: https://julien.duponchelle.info/android/use-proxy-with-waydroid/
- mitmproxy Documentation: https://docs.mitmproxy.org/
- Frida Universal Pinning Bypass: https://codeshare.frida.re/@sowdust/universal-android-ssl-pinning-bypass-2/

### Tools
- waydroid_script (ARM translation, Magisk, etc): https://github.com/casualsnek/waydroid_script
- HTTPToolkit Frida Scripts: https://github.com/httptoolkit/frida-interception-and-unpinning
- apk-mitm: https://github.com/mitmproxy/android-unpinner

### Community
- Waydroid GitHub Discussions
- Android MITM Stack Overflow threads
- Linux container communities
