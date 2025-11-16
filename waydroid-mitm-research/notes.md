# Waydroid MITM Research Notes

## Research Goal
Assess whether Waydroid is technically feasible for MITM traffic interception of certificate-pinned Android apps, comparing it to docker-android approaches.

## Research Timeline

### Initial Research Phase

### What is Waydroid? (Initial findings)

**Architecture:**
- Container-based Android on Linux (NOT a VM)
- Uses Linux namespaces: user, pid, uts, net, mount, ipc
- Based on LineageOS (currently Android 11/13 depending on version)
- Direct hardware access unlike traditional virtualization
- Python-based management tool

**Key Technical Points:**
- Runs on any GNU/Linux platform
- Minimal customized Android system image
- Process-level isolation with namespace separation
- More efficient than full VMs due to container approach
- 10.3k GitHub stars - actively maintained project

**Questions to explore:**
- How does namespace isolation affect certificate management?
- Can we get root/system access inside the container?
- Network namespace - can we intercept traffic?
- How does it compare to docker-android for MITM purposes?


### Certificate Management & Root Access

**System Access:**
- Full shell access via `sudo waydroid shell`
- ADB connectivity: `adb connect <IP>:5555`
- Root-level access to Android system
- Can modify system files and certificates

**Certificate Installation:**
- Android 7+ requires system-level CA installation (user certs not trusted by apps)
- Root access available for system cert installation
- Android 14 changes may affect direct cert installation methods
- Firefox has debug menu option: "Use third party CA certificates"

**Key Advantages for MITM:**
- Full root access without rooting limitations
- Can modify /system/etc/security/cacerts/
- System-level certificate trust possible

### Network Configuration

**Proxy Setup:**
- System-wide proxy: `adb shell settings put global http_proxy "172.17.0.1:8888"`
- Remove proxy: `adb shell settings put global http_proxy :0`
- Uses waydroid0 network interface on host
- NAT bridge networking model

**Transparent Proxy with iptables:**
```bash
sudo iptables -t nat -A PREROUTING -i waydroid0 -p tcp --dport 80 -j REDIRECT --to-port $port
sudo iptables -t nat -A PREROUTING -i waydroid0 -p tcp --dport 443 -j REDIRECT --to-port $port
sudo sysctl -w net.ipv4.ip_forward=1
```

**Network Architecture:**
- Container gets its own IP (e.g., 192.168.250.x)
- Gateway: 192.168.250.1 (host's waydroid0 interface)
- DNS on port 53, DHCP on port 67
- Traffic routing fully controllable via iptables

**Key Advantages:**
- Clean network isolation via waydroid0 interface
- iptables REDIRECT/DNAT fully supported
- Transparent proxy setup proven to work
- Users successfully running mitmproxy with Waydroid

### Certificate Pinning Bypass

**Frida Compatibility:**
- Frida server should run on Waydroid (standard Android)
- Can use universal SSL pinning bypass scripts
- HTTPToolkit has Waydroid-tested scripts
- Same Frida techniques as physical devices/emulators

**Proven Approach:**
1. Install Frida server in Waydroid
2. Use universal pinning bypass scripts
3. Combine with mitmproxy transparent proxy
4. System CA + Frida = comprehensive coverage


### App Installation & Compatibility

**APK Installation:**
- Direct APK install: `waydroid app install <package.apk>`
- Command line: `waydroid install com.xxx.yyy`
- Via ADB after connecting to Waydroid IP

**Google Play Support:**
- Can install with GAPPS Android type during setup
- Requires device certification at https://www.google.com/android/uncertified
- Aurora Store (F-droid) as alternative to Play Store
- Some apps marked incompatible with "emulator"

**ARM Compatibility:**
- Base Waydroid: x86/x64 only
- ARM translation via libhoudini (Intel) or libndk (AMD)
- Install via waydroid_script: `python3 main.py install libndk`
- casualsnek/waydroid_script also adds Magisk, OpenGapps
- Different apps work better on different translation layers

**Limitations:**
- ARM-only apps require translation layer
- Some games with native libs may not work
- Google Play may block some apps on "emulator"
- Architecture mismatch most common issue

**For MITM Use Case:**
- Most network apps should work fine
- Target apps likely available via Play Store or APK
- ARM translation handles most compatibility issues
- Better than emulator for app compatibility

### Deployment Options

**Current State:**
- NOT designed for Docker (requires privileged mode + LXC)
- Experimental docker-waydroid project exists (tyzbit/docker-waydroid)
- Can run in VM with virtio-gpu 3d acceleration
- Distrobox as alternative container approach

**Headless/Remote Access:**
- Waydroid requires Wayland compositor
- Headless: Use Weston with X11 backend + Xvfb
- VNC: wayvnc for wlroots compositors
- RDP: Ansible role exists for RDP configuration
- More complex than docker-android for headless

**Cloud/VM Deployment:**
- Can run in VM with proper GPU setup
- Requires nested virtualization or bare metal
- EGL-headless possible for dedicated VM servers
- More complex than docker-android

**Practical Reality:**
- Best suited for Linux desktop/laptop deployment
- Can work in VM but requires additional setup
- Headless possible but not straightforward
- Docker support is experimental and limited

**Comparison to docker-android:**
- docker-android: Purpose-built for containers, VNC built-in
- Waydroid: Desktop-focused, requires Wayland compositor
- docker-android wins for cloud/headless deployment
- Waydroid wins for local Linux development


### Performance & Resource Comparison

**Waydroid Performance:**
- Container-based: No virtualization overhead
- Near-native performance (direct hardware access)
- GPU acceleration supported
- More efficient than traditional emulators
- Lightweight compared to full VMs

**docker-android Performance:**
- Requires --privileged for KVM acceleration
- Lightweight, doesn't require VT-X/AMD-V
- Ideal for cloud environments
- Good for CI/CD pipelines
- More overhead than Waydroid but more portable

**Use Case Fit:**
- Waydroid: Best performance on Linux workstation
- docker-android: Best portability, remote deployment

### Kernel & System Requirements

**Required Kernel Modules:**
- CONFIG_ANDROID=y
- CONFIG_ANDROID_BINDER_IPC=m or =y
- CONFIG_ANDROID_BINDERFS (newer) or BINDER_DEVICES (older)
- CONFIG_PSI=y (Process State Information)
- CONFIG_BLK_DEV_LOOP=m (Loop devices)
- IPv6 networking built-in (even for IPv4)

**Kernel Versions:**
- Works on linux, linux-lts, linux-zen
- Custom kernels may need DKMS modules
- Some issues with kernels 5.18+ (ibt=off workaround)

**Critical for VM/Cloud:**
- Host kernel must have binder modules
- Nested Wayland session possible in X11
- VM needs proper kernel configuration
- May not work in all cloud environments
- Requires Linux host (won't work in Docker on macOS/Windows)

**Blockers for Cloud Deployment:**
- Requires Linux kernel with binder support
- Wayland compositor requirement
- Not designed for containerization
- More complex than docker-android for remote deployment


### System Modifications & Certificate Installation

**File System Access:**
- Host access: `/var/lib/waydroid/rootfs` and `/var/lib/waydroid/overlay`
- Overlay system: Write to `/var/lib/waydroid/overlay/system/` for persistent changes
- Remount rootfs: `mount -o remount,rw /var/lib/waydroid/rootfs`
- Android 10+ uses system-as-root (can't remount /system directly)

**Certificate Installation Path:**
- System certs: `/system/etc/security/cacerts/`
- Use overlay approach: `/var/lib/waydroid/overlay/system/etc/security/cacerts/`
- mitmproxy cert location on host: `~/.mitmproxy/`
- Need to convert PEM to Android format (hash-based filename)

**Real-World MITM Examples:**
- Julien Duponchelle blog post: "Use a proxy with Waydroid"
- Users successfully running mitmproxy + Waydroid
- Certificate installation confirmed working
- iptables transparent proxy confirmed functional

**Certificate Pinning Bypass:**
- apk-mitm: Automatic pinning removal from APKs
- objection (Frida-based): Runtime pinning bypass
- Frida scripts: Universal SSL pinning bypass
- Same techniques as regular Android devices

### Advantages Over docker-android

1. **Performance:**
   - No virtualization overhead
   - Direct hardware access
   - Better GPU acceleration
   - Faster than emulator-based solutions

2. **Root Access:**
   - Full system access by default
   - Direct file system modifications via host
   - No rooting process needed
   - Overlay system for persistent changes

3. **Native Linux Integration:**
   - Runs directly on Linux kernel
   - Clean network isolation (waydroid0)
   - iptables integration straightforward
   - Better performance for local development

4. **App Compatibility:**
   - ARM translation available (libhoudini/libndk)
   - Google Play support with GAPPS
   - Better than emulator for some apps
   - More like real device

### Disadvantages vs docker-android

1. **Deployment Complexity:**
   - Requires Linux host with specific kernel modules
   - Wayland compositor requirement
   - Not Docker-native (experimental support only)
   - Headless setup more complex

2. **Cloud/Remote Deployment:**
   - docker-android purpose-built for containers
   - docker-android has VNC built-in
   - Waydroid needs nested Wayland session
   - docker-android better for CI/CD

3. **Portability:**
   - Waydroid: Linux-only
   - docker-android: Works anywhere Docker runs
   - docker-android easier to automate
   - docker-android better for teams

4. **Documentation:**
   - docker-android more focused on automation
   - Waydroid more desktop-oriented
   - Less MITM-specific documentation for Waydroid
   - docker-android has established patterns


### Final Technical Assessment Summary

**MITM Feasibility: YES - Technically Feasible**

Waydroid CAN be used for MITM traffic interception of certificate-pinned Android apps with the following approach:

1. System CA installation via overlay filesystem
2. iptables transparent proxy on waydroid0 interface
3. Frida-based certificate pinning bypass
4. mitmproxy for traffic interception

**Best Use Cases for Waydroid:**
- Local Linux workstation development
- Performance-critical applications
- Testing that needs near-native performance
- Single-developer research setup

**NOT Recommended for:**
- Cloud/remote deployment (use docker-android)
- CI/CD pipelines (use docker-android)
- Team environments requiring standardization
- Headless automation at scale

**Comparison Matrix:**

| Aspect | Waydroid | docker-android |
|--------|----------|----------------|
| Performance | Excellent (native) | Good (virtualized) |
| MITM Setup | Moderate | Easy |
| Root Access | Built-in | Built-in |
| Network Control | iptables (clean) | iptables (clean) |
| Certificate Install | Via overlay | Standard /system |
| Frida Support | Yes | Yes |
| Cloud Deploy | Complex | Simple |
| Headless Mode | Complex | Built-in VNC |
| Docker Support | Experimental | Native |
| Linux Requirement | Yes (kernel deps) | No (any Docker host) |
| Setup Complexity | Moderate-High | Low-Moderate |

**Recommendation:**
For the specific use case of MITM traffic interception:
- If deploying locally on Linux: Waydroid is excellent
- If deploying to cloud/VM/Docker: Use docker-android
- If team collaboration needed: Use docker-android
- If maximum performance needed: Use Waydroid

The existing docker-android MVP is likely the better choice for most scenarios due to:
- Easier deployment and sharing
- Better documentation for MITM use case
- Simpler headless/remote operation
- More standardized and reproducible

Waydroid is a viable alternative but adds complexity without significant advantages for the MITM use case specifically.

