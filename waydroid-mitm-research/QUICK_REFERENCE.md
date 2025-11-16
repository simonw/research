# Waydroid MITM Quick Reference

## TL;DR

**Verdict**: Waydroid works for MITM but use docker-android instead.

**Why docker-android?**
- Simpler deployment (Docker-native)
- Built-in VNC for headless
- Better for cloud/team/CI-CD
- Same MITM capabilities
- Better documentation

**When to use Waydroid?**
- Only if on Linux locally AND need max performance

## Key Technical Facts

### What is Waydroid?
- Container-based Android on Linux (NOT a VM)
- Uses Linux namespaces, not virtualization
- Near-native performance
- Requires Linux kernel with binder modules
- Requires Wayland compositor

### MITM Capabilities

| Capability | Status | Notes |
|------------|--------|-------|
| Root Access | YES | Built-in, full system access |
| System Certs | YES | Via overlay filesystem |
| Transparent Proxy | YES | iptables on waydroid0 |
| Frida Support | YES | Standard Android environment |
| ARM Apps | YES | Via libhoudini/libndk |
| Docker Deploy | NO | Experimental only |
| Headless | COMPLEX | Requires Wayland + VNC setup |

### Quick Setup (If You Decide to Use It)

```bash
# 1. Install
curl https://repo.waydro.id | sudo bash
sudo apt install waydroid
sudo waydroid init -s GAPPS

# 2. Install mitmproxy cert
hash=$(openssl x509 -inform PEM -subject_hash_old \
  -in ~/.mitmproxy/mitmproxy-ca-cert.pem | head -1)
sudo cp ~/.mitmproxy/mitmproxy-ca-cert.pem \
  /var/lib/waydroid/overlay/system/etc/security/cacerts/${hash}.0
sudo chmod 644 /var/lib/waydroid/overlay/system/etc/security/cacerts/${hash}.0

# 3. Setup transparent proxy
sudo iptables -t nat -A PREROUTING -i waydroid0 -p tcp --dport 443 \
  -j REDIRECT --to-port 8080
sudo sysctl -w net.ipv4.ip_forward=1

# 4. Run mitmproxy
mitmproxy --mode transparent --showhost

# 5. Install Frida (in another terminal)
adb connect $(ip addr show waydroid0 | grep 'inet ' | awk '{print $2}' | cut -d/ -f1 | sed 's/1$/2/'):5555
# Download frida-server, push it, run it

# 6. Bypass pinning
frida --codeshare sowdust/universal-android-ssl-pinning-bypass-2 -U -f com.app.name
```

### Comparison Cheat Sheet

```
Performance:        Waydroid >>> docker-android
Setup Complexity:   docker-android <<< Waydroid
Cloud Deploy:       docker-android <<< Waydroid
Headless:           docker-android <<< Waydroid
Documentation:      docker-android >>> Waydroid
MITM Capability:    Waydroid === docker-android
```

### Critical Blockers

1. **Linux-Only**: Requires Linux host with specific kernel modules
2. **No Docker**: Not Docker-native, experimental support only
3. **Wayland Required**: Needs compositor, complex for headless
4. **Cloud Unfriendly**: Kernel requirements limit cloud options

### Advantages Over docker-android

1. Better performance (no virtualization)
2. Already includes root by default
3. Better app compatibility
4. Native Linux integration

### Disadvantages vs docker-android

1. Complex deployment (kernel deps, Wayland)
2. No built-in VNC
3. Docker support experimental
4. Linux-only
5. Less MITM-focused documentation

## Recommendation

**For MITM use case: Use docker-android**

Waydroid doesn't solve any problems that docker-android has, and adds significant deployment complexity. Your existing docker-android MVP is the right choice.

## Key Resources

- Waydroid proxy setup: https://julien.duponchelle.info/android/use-proxy-with-waydroid/
- waydroid_script (for ARM apps): https://github.com/casualsnek/waydroid_script
- Official docs: https://docs.waydro.id/
