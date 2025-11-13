# Native Mobile App Traffic Capture: Research Report

## Executive Summary

This research evaluates approaches for capturing HTTPS traffic from native mobile apps (iOS and Android) against three key requirements:

1. ✅ **No App Modification**: Must work on any app without modifying the app itself
2. ✅ **Easy User Experience**: Very easy for users to set up (installing certificates or proxies is bad UX)
3. ✅ **HTTPS Body Capture**: Must be able to read and capture HTTPS request bodies

### Key Finding

**There is no silver bullet solution that perfectly meets all three requirements.** Every approach involves trade-offs between security restrictions, user experience, and coverage.

**The fundamental blocker**: Android 7+ (API 24+) apps do not trust user-installed certificates by default, making traditional MITM proxy approaches ineffective for 80-90% of modern Android apps.

### Recommended Approaches

| Use Case | Recommendation | Coverage | UX Friction | Cost |
|----------|---------------|----------|-------------|------|
| **iOS Only** | Proxyman-style local VPN | ~70% of apps | Moderate | Low |
| **Android Only** | Cloud emulator with system CA | ~95% of apps | Low | High |
| **Both Platforms (Enterprise)** | Sauce Labs / BrowserStack | ~90% of apps | Low | High |
| **Both Platforms (Self-Hosted)** | Cloud device farm (ProxyPin + Docker-Android) | ~90% of apps | Medium | Medium |
| **Metadata Only** | PCAPdroid / VPN-based capture | 100% of apps | Low | Low |

---

## Table of Contents

1. [Approach Comparison Matrix](#approach-comparison-matrix)
2. [Detailed Analysis by Category](#detailed-analysis-by-category)
3. [Platform-Specific Challenges](#platform-specific-challenges)
4. [Technical Deep Dives](#technical-deep-dives)
5. [Implementation Recommendations](#implementation-recommendations)
6. [Cost Analysis](#cost-analysis)
7. [Future Outlook](#future-outlook)

---

## Approach Comparison Matrix

### Legend
- ✅ Fully meets requirement
- ⚠️ Partially meets requirement
- ❌ Does not meet requirement
- 💰 Cost indicator (💰 = low, 💰💰💰 = high)

### All Approaches

| Approach | No App Mod | Easy UX | HTTPS Bodies | iOS Support | Android Support | Coverage | Cost |
|----------|------------|---------|--------------|-------------|-----------------|----------|------|
| **Local MITM Proxies** ||||||||
| Proxyman iOS | ✅ | ⚠️ Cert + VPN | ✅ | ✅ 70% | ❌ 10% | iOS: 70% | 💰 |
| Charles Proxy | ✅ | ❌ Manual setup | ✅ | ⚠️ 70% | ❌ 10% | Mixed: 40% | 💰 |
| HTTP Toolkit | ✅ | ❌ ADB required | ⚠️ Limited | ⚠️ | ❌ 10% | Low | 💰 |
| **VPN-Based Tools** ||||||||
| PCAPdroid | ✅ | ✅ One-click | ❌ Metadata only | ❌ | ✅ 100% | Metadata | 💰 |
| ProxyPin | ✅ | ⚠️ Cert + VPN | ✅ | ⚠️ 70% | ❌ 10% | Mixed: 40% | 💰 |
| **Cloud Solutions** ||||||||
| Cloud Emulator | ✅ | ✅ Browser access | ✅ | ⚠️ Expensive | ✅ 95% | High | 💰💰💰 |
| BrowserStack | ⚠️ Re-sign | ✅ Turnkey | ✅ | ✅ 90% | ✅ 90% | 90% | 💰💰💰 |
| LambdaTest | ⚠️ Re-sign | ✅ Turnkey | ✅ | ✅ 90% | ✅ 90% | 90% | 💰💰💰 |
| Sauce Labs | ✅ MITM proxy | ✅ Turnkey | ✅ | ✅ 95% | ✅ 95% | 95% | 💰💰💰 |
| AWS Device Farm | ✅ | ✅ | ❌ No support | ❌ | ❌ | 0% | 💰💰 |
| **Advanced Techniques** ||||||||
| Frida Unpinning | ✅ | ❌ Root/debug | ✅ | ⚠️ Jailbreak | ⚠️ Root | 95%+ | 💰 |
| App Virtualization | ✅ | ⚠️ Setup | ✅ | ❌ | ⚠️ 50% | Low | 💰 |
| APK Repackaging | ❌ Modifies | ❌ Technical | ✅ | ❌ | ⚠️ Per-app | Manual | 💰 |
| **Self-Hosted Farms** ||||||||
| Docker-Android | ✅ | ✅ Browser | ✅ | ❌ | ✅ 95% | Android: 95% | 💰💰 |
| GADS | ✅ | ⚠️ Complex setup | ✅ | ✅ | ✅ 95% | 95% | 💰💰 |
| DeviceFarmer/OpenSTF | ✅ | ⚠️ USB setup | ⚠️ Manual CA | ✅ | ✅ 90% | 90% | 💰💰 |

---

## Detailed Analysis by Category

### Category 1: Local MITM Proxy Solutions

#### Proxyman iOS ⭐ (Best for iOS)

**How it works:**
- NEPacketTunnelProvider creates local VPN
- User installs root CA certificate
- In-app proxy engine (SwiftNIO) intercepts and decrypts HTTPS
- Traffic displayed in real-time UI

**Requirements Assessment:**
- ✅ **No App Modification**: Works on any app
- ⚠️ **Easy UX**: Requires one-time certificate + VPN profile installation (moderate friction)
- ✅ **HTTPS Bodies**: Full request/response inspection

**Coverage:**
- iOS: ~70% (fails on certificate-pinned apps like banking apps)
- Android: ~10% (fundamentally blocked by Android 7+ security model)

**Pros:**
- Gold standard for iOS traffic inspection
- Local processing (privacy-friendly)
- Rich feature set (filtering, scripting, breakpoints)
- No recurring costs
- Available via TestFlight

**Cons:**
- Cannot ship via App Store (requires TestFlight or Enterprise)
- Certificate installation triggers security warnings
- Fails on pinned apps
- iOS only (Android version not viable)

**Cost:** Free or ~$99 one-time purchase

**Verdict:** **Recommended for iOS-only use cases**. Best local solution for iOS, but Android equivalent is not viable due to OS restrictions.

---

#### Charles Proxy / mitmproxy

**How it works:**
- Traditional desktop proxy application
- User configures device Wi-Fi to point to proxy IP:port
- User installs root CA certificate
- Desktop app shows traffic

**Requirements Assessment:**
- ✅ **No App Modification**: Works on any app
- ❌ **Easy UX**: Manual proxy configuration + certificate installation (poor UX)
- ✅ **HTTPS Bodies**: Full inspection where trusted

**Coverage:**
- iOS: ~70% (works for most apps except pinned)
- Android: ~10% (blocked by user CA restrictions)

**Pros:**
- Industry standard tools
- Rich debugging features
- Cross-platform (macOS, Windows, Linux)
- Established documentation

**Cons:**
- Poor UX (multi-step manual setup)
- Breaks when switching networks
- Same Android 7+ limitations
- Requires desktop computer running proxy

**Cost:** Charles: $50, mitmproxy: Free (open source)

**Verdict:** **Not recommended for consumer use**. Too much friction for end users. Better suited for developers debugging their own apps.

---

#### HTTP Toolkit

**How it works:**
- "Android Device via ADB" mode
- Automatically creates VPN via ADB
- Injects certificate via ADB
- Routes traffic through desktop proxy

**Requirements Assessment:**
- ✅ **No App Modification**: Works on any app
- ❌ **Easy UX**: Requires ADB setup, USB cable, developer mode
- ⚠️ **HTTPS Bodies**: Only for apps that trust user CAs

**Coverage:**
- Android: ~10-20% (user CA restrictions)
- iOS: Limited (similar limitations)

**Pros:**
- Automated setup via ADB
- Open source
- Active development
- Good documentation

**Cons:**
- Requires technical setup (ADB, developer mode)
- USB cable required
- Same user CA limitations
- Not suitable for non-technical users

**Cost:** Free (open source)

**Verdict:** **Developer tool, not consumer-ready**. Good for testing your own apps, not for general-purpose traffic capture.

---

### Category 2: VPN-Based Solutions

#### PCAPdroid ⭐ (Best for Android Metadata)

**How it works:**
- Android VpnService captures all packets
- Built-in mitmproxy add-on attempts TLS decryption
- Stores logs locally, exports PCAP files

**Requirements Assessment:**
- ✅ **No App Modification**: Works on any app
- ✅ **Easy UX**: Install from Play Store, one-click enable
- ❌ **HTTPS Bodies**: Can see metadata (domains, IPs, timing) but NOT encrypted payloads for most apps

**Coverage:**
- Android: 100% for metadata, ~10% for HTTPS bodies
- iOS: Not available

**Pros:**
- Excellent UX (one-click)
- No root required
- Available on Play Store
- Open source (GPL)
- Rich metadata capture
- PCAP export for Wireshark analysis

**Cons:**
- Cannot decrypt HTTPS for most apps
- Same user CA limitations
- Android only
- Metadata-only for 90% of apps

**Cost:** Free (open source)

**Verdict:** **Recommended for network analysis and debugging**, but does NOT meet the HTTPS body capture requirement. Excellent for seeing which domains apps contact, connection timing, bandwidth usage, etc.

---

#### ProxyPin ⭐ (Best Open-Source Reference)

**How it works:**
- Flutter-based cross-platform app
- VpnService (Android) / NEPacketTunnelProvider (iOS)
- Local MITM proxy with certificate installation
- Rich UI with filtering, rewriting, scripting

**Requirements Assessment:**
- ✅ **No App Modification**: Works on any app
- ⚠️ **Easy UX**: Requires certificate + VPN setup
- ✅ **HTTPS Bodies**: Full inspection where trusted

**Coverage:**
- iOS: ~70% (same as Proxyman)
- Android: ~10% (user CA restrictions)

**Pros:**
- Open source (Apache 2.0)
- 10,000+ GitHub stars
- Cross-platform (Flutter)
- Full feature set comparable to Proxyman
- Excellent reference implementation
- Active development

**Cons:**
- Same Android limitations as all local MITM approaches
- Cannot ship on App Store / Play Store
- Certificate installation UX friction
- Not a silver bullet

**Cost:** Free (open source)

**Verdict:** **Best open-source starting point for building a custom solution**. Excellent for understanding how to implement VPN-based MITM on both platforms, but faces same fundamental limitations.

---

### Category 3: Cloud-Based Solutions

#### Cloud Emulator with System CA ⭐⭐ (Best Technical Solution)

**Architecture:**
```
User's Phone (Companion App)
  ↓ VPN Tunnel (WireGuard/WebSocket)
Cloud Infrastructure
  ├── Android AVD (system CA pre-installed)
  ├── mitmproxy (intercepts traffic)
  ├── Storage (PostgreSQL/ClickHouse)
  └── WebRTC Streaming (screen to user)
```

**How it works:**
1. User installs companion app
2. App creates VPN tunnel to cloud
3. App runs in cloud emulator with system-level CA (not user CA)
4. mitmproxy decrypts all HTTPS traffic
5. User sees screen streamed back via WebRTC
6. Traffic logs stored in database

**Requirements Assessment:**
- ✅ **No App Modification**: Apps run unmodified in emulator
- ✅ **Easy UX**: One-time VPN setup, then browser access
- ✅ **HTTPS Bodies**: Full capture with system CA

**Coverage:**
- Android: ~95% (system CA bypasses user restrictions, Frida can handle pinning)
- iOS: ~95% (requires macOS VMs or real device farm with MDM supervision)

**Pros:**
- Bypasses all local device restrictions
- Works for nearly all apps (system CA trusted)
- Can combine with Frida for pinning bypass
- No certificate installation on user's device
- Centralized logging and analytics
- Scalable infrastructure

**Cons:**
- Complex infrastructure (Kubernetes, device orchestration)
- High operational cost (compute, storage, streaming)
- iOS requires macOS VMs (expensive)
- Latency from streaming
- High-security apps may detect emulation (Play Integrity API)
- Resource-intensive (full AVD per session)

**Implementation Stack (Android):**
- **Emulator**: Docker-Android or AOSP cloud-AVD
- **MITM**: mitmproxy
- **Orchestration**: Kubernetes + Helm
- **Streaming**: WebRTC or noVNC
- **Optional**: Frida for certificate pinning bypass
- **Storage**: PostgreSQL for structured data, ClickHouse for time-series

**Implementation Stack (iOS):**
- **Devices**: macOS VMs + iOS Simulator or MDM-supervised real devices
- **MITM**: mitmproxy
- **Orchestration**: Kubernetes (macOS VMs)
- **Streaming**: WebRTC
- **Cost**: ~$100-300/month per macOS instance

**Cost:**
- Android: ~$0.10-0.50 per hour per active session
- iOS: ~$0.50-2.00 per hour per active session
- Infrastructure: $500-5,000/month depending on scale

**Verdict:** **Most comprehensive technical solution**. Meets all requirements and works for nearly all apps, but requires significant engineering investment and operational costs. Best for companies building a commercial product.

---

#### BrowserStack ⭐

**How it works:**
- Re-signs app to embed custom proxy certificate
- Runs app on real devices in BrowserStack's device farm
- Routes traffic through network logging service
- Web UI displays captured traffic

**Requirements Assessment:**
- ⚠️ **No App Modification**: App is re-signed (code unchanged but signature changes)
- ✅ **Easy UX**: Upload app, select device, automatic setup
- ✅ **HTTPS Bodies**: Full request/response capture

**Coverage:**
- iOS: ~90% (fails on apps with anti-tampering)
- Android: ~90% (re-signing bypasses user CA restrictions)

**Pros:**
- Turnkey SaaS solution
- 3,500+ real devices
- No infrastructure management
- Real-time network inspection
- HAR file export
- Video recordings, screenshots
- Network throttling simulation

**Cons:**
- App re-signing may trigger anti-tamper detection
- Expensive (enterprise pricing)
- Apps from App Store/Play Store need re-uploading
- Pinned apps still fail
- Privacy concerns (app uploaded to third party)

**Cost:** Enterprise pricing (typically $1,000-5,000+/month)

**Verdict:** **Good for enterprise testing**, but re-signing approach may cause issues with security-sensitive apps.

---

#### LambdaTest ⭐

**How it works:**
- Similar to BrowserStack (app re-signing + proxy)
- 10,000+ real devices
- Web UI for traffic inspection

**Requirements Assessment:**
- ⚠️ **No App Modification**: App is re-signed
- ✅ **Easy UX**: Turnkey web interface
- ✅ **HTTPS Bodies**: Full capture with domain filtering

**Coverage:**
- iOS: ~90%
- Android: ~90%
- Note: Apps from stores may not support network capture

**Pros:**
- Largest device fleet (10,000+)
- Real-time HTTP/S capture
- HAR export
- Domain-based filtering
- CI/CD integration

**Cons:**
- Only on Pro Plans
- App Store/Play Store apps limited support
- Pinned apps fail unless hosts excluded
- Enterprise pricing

**Cost:** Pro Plan: $99+/month, Enterprise: Custom pricing

**Verdict:** **Similar to BrowserStack**, good device coverage but same re-signing limitations.

---

#### Sauce Labs ⭐⭐⭐ (Best Commercial Solution)

**How it works:**
- **Sauce Labs Forwarder v1.1.0** with MITM support
- Does NOT re-sign apps (true MITM proxy)
- Runs apps on real devices with proxy configuration
- Network Capture on Real Device Cloud

**Requirements Assessment:**
- ✅ **No App Modification**: True MITM without re-signing
- ✅ **Easy UX**: Turnkey SaaS platform
- ✅ **HTTPS Bodies**: Full encrypted traffic inspection via MITM

**Coverage:**
- iOS: ~95% (best-in-class)
- Android: ~95% (MITM Forwarder bypasses user CA)
- Supports: Espresso, XCUITest, Appium, Flutter, React Native, Cordova

**Pros:**
- **Only commercial SaaS with true MITM** (not re-signing)
- Best-in-class network capture capabilities
- Large device fleet
- Cross-framework support
- HAR export
- CI/CD integration
- Enterprise-grade

**Cons:**
- Most expensive option
- Still faces some pinning challenges
- Privacy concerns (traffic routed through Sauce)

**Cost:** Enterprise pricing (typically $5,000-15,000+/month)

**Verdict:** **Best commercial solution** for enterprises that need reliable HTTPS capture across both platforms. Only SaaS with true MITM capabilities (Forwarder v1.1.0).

---

#### AWS Device Farm ❌

**How it works:**
- Real device testing platform
- **Does NOT support network traffic inspection**

**Requirements Assessment:**
- ✅ **No App Modification**: N/A
- ✅ **Easy UX**: N/A
- ❌ **HTTPS Bodies**: Not supported

**Verdict:** **Not suitable** for traffic capture. Use BrowserStack, LambdaTest, or Sauce Labs instead.

---

### Category 4: Advanced Techniques

#### Frida-Based Certificate Unpinning ⚠️

**How it works:**
- Dynamic instrumentation framework
- Hooks certificate validation functions at runtime
- Disables pinning checks
- Allows MITM proxy to work

**Requirements Assessment:**
- ✅ **No App Modification**: App binary unchanged
- ❌ **Easy UX**: Requires root (Android) or jailbreak (iOS), technical setup
- ✅ **HTTPS Bodies**: Full capture once pinning disabled

**Coverage:**
- iOS: ~95% with jailbreak + Frida
- Android: ~95% with root + Frida

**Pros:**
- Can bypass even certificate-pinned apps
- Works with any MITM proxy
- Open source
- HTTP Toolkit provides ready-made scripts

**Cons:**
- Requires root/jailbreak (dealbreaker for consumer use)
- Or requires debuggable app build
- Scripts need updates for new libraries
- Not user-friendly
- Violates app store terms

**Cost:** Free (open source)

**Verdict:** **Excellent for security research and testing**, but not viable for consumer products. Requires device modification (root/jailbreak).

---

#### App Virtualization (VirtualApp, Shelter) ⚠️

**How it works:**
- Runs apps in sandboxed virtual environment
- Virtual environment has custom certificate trust store
- MITM works within virtual environment

**Requirements Assessment:**
- ✅ **No App Modification**: App runs unmodified in sandbox
- ⚠️ **Easy UX**: User must move apps into virtual environment
- ✅ **HTTPS Bodies**: Full capture within sandbox

**Coverage:**
- Android: ~50% (many apps detect virtualization)
- iOS: Not available

**Pros:**
- No root required
- Can customize certificate trust
- Works for some apps

**Cons:**
- Banking apps, Google apps, social apps detect virtualization
- Resource-heavy
- Poor compatibility
- Android only

**Cost:** Free (VirtualApp open source) or paid versions

**Verdict:** **Partially viable** for non-security-sensitive apps. Many important apps detect and block virtualization.

---

#### APK Repackaging ❌

**How it works:**
- Decompile APK
- Modify Network Security Config to trust user CAs
- Strip pinned certificates
- Re-sign APK

**Requirements Assessment:**
- ❌ **No App Modification**: Violates this requirement
- ❌ **Easy UX**: Highly technical, per-app process
- ✅ **HTTPS Bodies**: Full capture after modification

**Pros:**
- Can make any app work with MITM
- Tools like apk-mitm automate some steps

**Cons:**
- **Violates "no app modification" requirement**
- Labor-intensive
- Breaks automatic updates
- Each app needs individual treatment
- May violate app terms of service

**Cost:** Free (tools are open source)

**Verdict:** **Not viable** under stated requirements. Only suitable for testing your own apps or security research.

---

### Category 5: Self-Hosted Device Farms

#### Docker-Android ⭐⭐ (Best Self-Hosted Android)

**How it works:**
- Android AVDs in Docker containers
- System CA pre-installed in emulator image
- Browser-based access via noVNC
- ADB over network

**Requirements Assessment:**
- ✅ **No App Modification**: Apps run unmodified
- ✅ **Easy UX**: Browser access, no local setup
- ✅ **HTTPS Bodies**: System CA enables full capture

**Coverage:**
- Android: ~95%
- iOS: Not supported

**Pros:**
- Easy deployment (Docker Compose)
- Multiple device profiles
- Can pre-install system CA
- No user CA restrictions
- ADB over network
- Open source

**Cons:**
- VNC streaming slower than WebRTC
- Emulator detection by some apps
- Android only
- Requires server infrastructure

**Cost:** Server costs only ($50-500/month depending on scale)

**Verdict:** **Highly recommended for self-hosted Android testing**. Best balance of ease and capability for Android emulation.

---

#### GADS (Generic Android Device System) ⭐

**What it is:**
- Free alternative to AWS Device Farm
- Supports Android, iOS, Tizen, webOS
- MJPEG/WebRTC streaming

**Requirements Assessment:**
- ✅ **No App Modification**: Real devices or emulators
- ⚠️ **Easy UX**: Complex setup, requires technical knowledge
- ✅ **HTTPS Bodies**: Can install system CA on supervised iOS or emulator

**Coverage:**
- Both platforms: ~90-95%

**Pros:**
- Cross-platform (Android and iOS)
- Real devices and emulators
- WebRTC streaming option
- Active community

**Cons:**
- Backend open source (AGPL), but UI is closed-source
- Complex setup
- iOS requires Mac + Xcode
- Requires physical device farm or VMs

**Cost:** Free (AGPL) + infrastructure costs

**Verdict:** **Good for serious self-hosted farms** that need both Android and iOS. More complex than Docker-Android.

---

#### DeviceFarmer / OpenSTF ⭐

**What it is:**
- Open-source device farm for real Android devices
- MJPEG streaming (30-40 FPS)
- Browser-based control

**Requirements Assessment:**
- ✅ **No App Modification**: Real devices
- ⚠️ **Easy UX**: Requires USB-connected devices, manual CA install
- ⚠️ **HTTPS Bodies**: Possible with CA installation but still faces user CA restrictions

**Coverage:**
- Android: ~90% with CA, ~10% without
- iOS: Not supported

**Pros:**
- Real Android devices (not emulators)
- Open source
- Active fork (DeviceFarmer) maintained
- Good for device testing labs

**Cons:**
- Requires physical devices
- USB connectivity required
- Manual certificate installation still needed
- OpenSTF original project unmaintained
- Android only

**Cost:** Free (open source) + device costs

**Verdict:** **Good for testing labs with physical devices**. Better for functional testing than traffic capture due to CA restrictions.

---

## Platform-Specific Challenges

### Android: The Fundamental Blocker

**Android 7+ (API 24+) User Certificate Restrictions**

Since Android 7 (released 2016), apps targeting API level 24 or higher **do not trust user-installed CA certificates by default**.

**Why this matters:**
- ~80-90% of modern apps target API 24+
- All MITM proxies rely on user-installed certificates
- This is an intentional security feature, not a bug
- Cannot be bypassed without:
  - Root access (to install system CA)
  - App modification (Network Security Config)
  - Running in controlled environment (emulator/VM)

**Network Security Config Example:**
```xml
<network-security-config>
    <base-config>
        <trust-anchors>
            <certificates src="system" />
            <certificates src="user" />  <!-- App must opt-in -->
        </trust-anchors>
    </base-config>
</network-security-config>
```

**Android 14 Made It Worse:**
- System certificates moved to `/apex/com.android.conscrypt/cacerts`
- Fully immutable with per-process mount namespacing
- Even root access doesn't help with traditional methods
- New workarounds exist but are complex

**Result:** Local MITM proxies (Proxyman-style) are fundamentally blocked on Android for consumer use.

---

### iOS: Balanced Security Model

**iOS Certificate Trust Model**

iOS apps trust user-installed CA certificates by default, making MITM proxies viable.

**Why this works:**
- User CA installation is intentionally designed to work
- Settings → General → VPN & Device Management → Trust Root CA
- Apps can still use certificate pinning to opt-out
- ~30% of apps use pinning (banking, social, enterprise)

**iOS Restrictions:**
- **App Store Distribution**: Apps using NEPacketTunnelProvider face scrutiny
  - Must use TestFlight or Enterprise distribution
  - VPN entitlement requires special review
- **Certificate Warnings**: Users see security warnings during install
- **Supervision**: Real devices need MDM supervision for full control

**Result:** Local MITM proxies work reasonably well on iOS (~70% coverage).

---

### Certificate Pinning (Both Platforms)

**What is Certificate Pinning?**
Apps embed the server's certificate or public key and validate against it, bypassing the system trust store.

**Apps that commonly use pinning:**
- Banking apps (Chase, Bank of America)
- Social apps (Facebook, Twitter/X, Instagram)
- Enterprise apps (Slack, Zoom, Microsoft Teams)
- Payment apps (PayPal, Venmo)

**Bypassing pinning requires:**
- Frida instrumentation (needs root/jailbreak)
- APK repackaging (violates "no modification")
- Running in controlled environment with custom binary patches

**Coverage impact:**
- iOS: ~30% of apps use pinning
- Android: ~40% of apps use pinning

---

## Technical Deep Dives

### Deep Dive 1: Why Proxyman Works on iOS But Not Android

#### iOS Implementation (✅ Works)

```
[App]
  ↓ HTTPS Request
[System Network Stack]
  ↓ Routes through active VPN
[NEPacketTunnelProvider Extension]
  ↓ Intercepts packets
[Local Proxy (SwiftNIO)]
  ↓ TLS handshake with user CA cert
[App trusts user CA] ✅
  ↓ Connection established
[Proxy sees plaintext]
```

**Key:** iOS apps trust user CAs by default.

#### Android Implementation (❌ Blocked)

```
[App targeting API 24+]
  ↓ HTTPS Request
[System Network Stack]
  ↓ Routes through VPN
[VpnService]
  ↓ Intercepts packets
[Local Proxy]
  ↓ TLS handshake with user CA cert
[App checks trust store]
  ↓ User CA not in trusted set
❌ Connection rejected (SSLHandshakeException)
```

**Key:** Android 7+ apps ignore user CAs by default.

**Workaround:** Run app in environment where you control the system trust store (emulator, rooted device).

---

### Deep Dive 2: Cloud Emulator Architecture

**Full Architecture Diagram:**

```
┌─────────────────────────────────────────────────────────────┐
│                        User's Phone                          │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ Companion App                                          │ │
│  │  - VPN Interface (NEPacketTunnelProvider / VpnService)│ │
│  │  - WebSocket/WireGuard Client                         │ │
│  │  - Screen Streaming Receiver (WebRTC)                 │ │
│  │  - Traffic Log Viewer                                 │ │
│  └────────────────────────────────────────────────────────┘ │
└──────────────────────┬──────────────────────────────────────┘
                       │ Encrypted Tunnel
                       │ (WireGuard / WebSocket)
                       ↓
┌─────────────────────────────────────────────────────────────┐
│                    Cloud Infrastructure                      │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Tunnel Endpoint (WireGuard Server)                   │  │
│  └────────────────────┬─────────────────────────────────┘  │
│                       ↓                                      │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Android AVD / iOS Simulator                          │  │
│  │  - System CA pre-installed (mitmproxy cert)          │  │
│  │  - Optional: Frida for pinning bypass                │  │
│  │  - App runs here (NOT on user's phone)               │  │
│  └────────────────────┬─────────────────────────────────┘  │
│                       ↓                                      │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ mitmproxy                                            │  │
│  │  - Intercepts all traffic from AVD/Simulator         │  │
│  │  - Decrypts with system-trusted CA                   │  │
│  │  - Logs requests/responses                           │  │
│  └────────────────────┬─────────────────────────────────┘  │
│                       ↓                                      │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Storage Layer                                        │  │
│  │  - PostgreSQL (structured data)                      │  │
│  │  - ClickHouse (time-series)                          │  │
│  │  - S3 (HAR files, recordings)                        │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Streaming Layer                                      │  │
│  │  - WebRTC Server                                     │  │
│  │  - TURN Server (NAT traversal)                       │  │
│  │  - Screen capture from AVD/Simulator                 │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**Key Insight:** The app runs in the cloud, not on the user's phone. The phone is just a remote control and display.

**This bypasses:**
- User CA restrictions (system CA installed)
- Certificate installation UX (no user action)
- Platform security models (you control the environment)

**Trade-offs:**
- High infrastructure cost
- Latency from streaming
- Emulator detection possible
- Complex orchestration

---

### Deep Dive 3: eBPF on Android

**What eBPF Can Do on Android:**
```
┌──────────────────────────────────────────┐
│           Android Application            │
└────────────────┬─────────────────────────┘
                 │ socket()
                 ↓
┌──────────────────────────────────────────┐
│           Kernel Network Stack           │
│  ┌────────────────────────────────────┐  │
│  │ eBPF Programs                      │  │
│  │  - cg_skb_ingress/egress          │  │
│  │  - Socket tagging                  │  │
│  │  - Per-UID firewall                │  │
│  │  - Traffic accounting              │  │
│  └────────────────────────────────────┘  │
└──────────────────┬───────────────────────┘
                   │
          ┌────────┴────────┐
          │                 │
    [Metadata]        [Encrypted
     collected         Payload]
          │                 │
          ✅               ❌
     Can capture      Cannot decrypt
```

**What eBPF Cannot Do:**
- Decrypt HTTPS payloads (encryption happens in userspace)
- Access TLS session keys
- Modify certificate validation

**Use Cases:**
- Network monitoring (which apps contact which domains)
- Bandwidth accounting
- Firewall rules
- Traffic shaping
- Performance analysis

**Verdict:** Complementary technology, not a solution for HTTPS body capture.

---

## Implementation Recommendations

### Recommendation 1: iOS-Only Product (Consumer)

**Approach:** Proxyman-style local VPN with modern Network.framework

**Architecture:**
```swift
// NEPacketTunnelProvider for VPN
class ProxyTunnelProvider: NEPacketTunnelProvider {
    private var proxyServer: NIOTSConnectionBootstrap?

    override func startTunnel() {
        // Use Network.framework for proxy server
        let tlsOptions = NWProtocolTLS.Options()
        tlsOptions.setLocalIdentity(generatedCertificate)

        // Start local proxy with SwiftNIO + Network.framework
        proxyServer = NIOTSConnectionBootstrap(group: eventLoopGroup)
            .tlsOptions(tlsOptions)
            // ... proxy implementation
    }
}
```

**Stack:**
- **Language:** Swift
- **VPN:** NEPacketTunnelProvider
- **Proxy:** SwiftNIO + Network.framework
- **Storage:** SQLite + FileManager
- **UI:** SwiftUI

**Distribution:**
- TestFlight for beta testing
- Enterprise distribution for organizations
- Cannot use App Store (VPN restrictions)

**Expected Coverage:** ~70% of apps

**Cost:** $20-50K development, ~$0 operational cost (runs locally)

**Timeline:** 3-4 months

---

### Recommendation 2: Android-Only Product (Self-Hosted)

**Approach:** Docker-Android with system CA + mitmproxy

**Architecture:**
```yaml
# docker-compose.yml
version: '3'
services:
  android-emulator:
    image: budtmo/docker-android:latest
    volumes:
      - ./ca-cert:/system/etc/security/cacerts/
    environment:
      - DEVICE="Samsung Galaxy S23"
      - APPIUM=true
    ports:
      - "6080:6080"  # noVNC
      - "5555:5555"  # ADB

  mitmproxy:
    image: mitmproxy/mitmproxy:latest
    volumes:
      - ./mitmproxy:/home/mitmproxy/.mitmproxy
    command: mitmweb --web-host 0.0.0.0
    ports:
      - "8080:8080"  # Proxy
      - "8081:8081"  # Web UI
```

**Setup Steps:**
1. Generate mitmproxy CA certificate
2. Convert to Android system cert format (hash.0)
3. Mount into Docker-Android at `/system/etc/security/cacerts/`
4. Configure emulator to use proxy
5. User accesses via browser (noVNC on port 6080)

**Expected Coverage:** ~95% of apps

**Cost:** $500-2,000/month (server infrastructure)

**Timeline:** 2-3 months

---

### Recommendation 3: Cross-Platform (Enterprise SaaS)

**Approach:** Sauce Labs or build cloud device farm

**Option A: Use Sauce Labs**
- Fastest time to market
- $5,000-15,000/month
- 95% coverage both platforms
- No development required

**Option B: Build Custom Cloud Farm**

**Android Stack:**
- Kubernetes cluster
- Docker-Android for emulators
- mitmproxy for HTTPS interception
- WebRTC for streaming (better than VNC)
- PostgreSQL for logs
- S3 for HAR files

**iOS Stack:**
- macOS VMs on AWS EC2 Mac instances
- iOS Simulator with system CA
- mitmproxy
- WebRTC streaming
- MDM profiles for supervised devices (if using real devices)

**Cost (Build):**
- Development: $150-300K (6-12 months)
- Operations: $5,000-20,000/month depending on scale

**Cost (Buy):**
- Sauce Labs: $5,000-15,000/month, immediate

**Recommendation:** Use Sauce Labs unless you need specific customization or have >100K monthly sessions.

---

### Recommendation 4: Consumer Cross-Platform (Hybrid)

**Approach:** iOS local + Android metadata

**iOS Component:**
- Proxyman-style local VPN
- ~70% coverage for HTTPS bodies

**Android Component:**
- PCAPdroid-style metadata capture
- 100% coverage for metadata (domains, timing)
- 10% coverage for HTTPS bodies

**User Experience:**
1. Install companion app
2. iOS: One-time certificate + VPN setup
3. Android: One-click VPN enable
4. View traffic logs in app
5. Note: Android shows metadata only for most apps

**Cost:** $50-100K development, minimal operational cost

**Timeline:** 4-6 months

**Trade-off:** Accept limited Android capability to avoid infrastructure costs.

---

## Cost Analysis

### Total Cost of Ownership (TCO) Comparison

| Approach | Dev Cost | Monthly Op Cost | Per-User Cost | Setup Time |
|----------|----------|-----------------|---------------|------------|
| **Local iOS (Proxyman-style)** | $50K | $100 | $0 | 3-4 months |
| **Local Android (impossible)** | N/A | N/A | N/A | N/A |
| **PCAPdroid (metadata only)** | $30K | $50 | $0 | 2-3 months |
| **Docker-Android (self-hosted)** | $40K | $500-2K | $0.10-0.50/session | 2-3 months |
| **Cloud Device Farm (custom)** | $200K+ | $5K-20K | $0.50-2/session | 6-12 months |
| **BrowserStack (buy)** | $0 | $3K-10K | Included | Immediate |
| **LambdaTest (buy)** | $0 | $99-5K | Included | Immediate |
| **Sauce Labs (buy)** | $0 | $5K-15K | Included | Immediate |
| **ProxyPin (fork & customize)** | $30-50K | $100 | $0 | 2-4 months |

### Break-Even Analysis

**Build vs. Buy Decision:**

If you need **cross-platform** HTTPS capture:
- **< 1,000 users**: Use Sauce Labs ($5K-10K/month)
- **1,000-10,000 users**: Build custom cloud farm (breaks even ~12-18 months)
- **> 10,000 users**: Definitely build custom (much cheaper at scale)

If you only need **iOS**:
- Always build local solution (Proxyman-style)
- No operational costs, better privacy

If you only need **Android**:
- **Metadata only**: Build local (PCAPdroid-style)
- **HTTPS bodies**: Use cloud emulator (self-hosted or Sauce Labs)

---

## Future Outlook

### What Might Change

#### Positive Developments Possible
1. **Android API changes**: Google could add opt-in for traffic inspection (unlikely)
2. **zkTLS adoption**: Zero-knowledge TLS could provide provable data without MITM
3. **Better virtualization**: Improved Android virtualization that's harder to detect
4. **Browser-based approach**: Focus on web APIs instead of native apps

#### Negative Developments Likely
1. **More certificate pinning**: Apps increasingly adopting pinning
2. **Anti-debugging**: Apps detecting emulators, root, Frida
3. **Android restrictions tighten**: User CA restrictions extended to more areas
4. **Play Integrity API**: Better detection of modified environments

### The Reality

**There is no magic solution coming.** The security restrictions exist for good reasons (user privacy, app security). Any approach will involve trade-offs.

**Most realistic paths forward:**

1. **Accept limitations**:
   - iOS: Local MITM works for ~70%
   - Android: Metadata-only for consumer use
   - Both: Cloud emulator for high coverage

2. **Change requirements**:
   - Instead of capturing all apps, focus on web APIs
   - Use browser extensions or web-based data capture
   - Partner with app providers for authorized data access

3. **zkTLS approach**:
   - Research protocols like TLSNotary, Reclaim, Opacity Network
   - User proves they accessed data without MITM
   - No traffic capture needed, just cryptographic proofs
   - More complex but better privacy guarantees

---

## Conclusions

### Key Takeaways

1. **No perfect solution exists** that meets all three requirements (no mod, easy UX, HTTPS bodies) on both platforms

2. **Android is the blocker**: User CA restrictions make local MITM unviable for 80-90% of apps

3. **iOS is tractable**: Local VPN + user CA works for ~70% of apps with moderate UX friction

4. **Cloud emulators are the workaround**: Running apps in controlled environment bypasses all restrictions but costs $$

5. **Commercial SaaS works**: Sauce Labs, BrowserStack, LambdaTest provide turnkey solutions for enterprises

### Recommendations by Use Case

| Use Case | Best Approach | Why |
|----------|---------------|-----|
| iOS-only consumer app | Proxyman-style local VPN | Works for 70%, no op costs |
| Android-only consumer app | PCAPdroid-style metadata | Only viable local approach |
| Cross-platform consumer | iOS local + Android metadata | Balances capability and cost |
| Enterprise testing | Sauce Labs | Best commercial solution |
| High-volume SaaS | Custom cloud device farm | Cost-effective at scale |
| Security research | Frida + rooted devices | Maximum control and coverage |
| Self-hosted testing | Docker-Android + ProxyPin | Open source, full control |

### Final Verdict

If you must have **HTTPS body capture on Android without modification and with easy UX**, your only viable option is:

**☁️ Cloud-based emulator/simulator with system CA pre-installed**

Everything else is either:
- Limited to iOS ✅
- Requires app modification ❌
- Captures metadata only ⚠️
- Requires root/jailbreak ❌
- Expensive commercial SaaS 💰💰💰

**There is no magical local solution for Android.** Accept this reality and choose the trade-off that fits your requirements and budget.

---

## Additional Resources

### Open Source Projects
- **ProxyPin**: https://github.com/wanghongenpin/network_proxy_flutter
- **PCAPdroid**: https://github.com/emanuele-f/PCAPdroid
- **Docker-Android**: https://github.com/budtmo/docker-android
- **mitmproxy**: https://mitmproxy.org/
- **Frida**: https://frida.re/
- **HTTP Toolkit**: https://httptoolkit.com/

### Commercial Solutions
- **Sauce Labs**: https://saucelabs.com/
- **BrowserStack**: https://www.browserstack.com/
- **LambdaTest**: https://www.lambdatest.com/
- **Proxyman**: https://proxyman.io/

### Technical Documentation
- **Android Network Security Config**: https://developer.android.com/training/articles/security-config
- **iOS Network Extension**: https://developer.apple.com/documentation/networkextension
- **Apple SimpleTunnel**: https://developer.apple.com/library/archive/samplecode/SimpleTunnel/
- **eBPF on Android**: https://source.android.com/docs/core/data/ebpf-traffic-monitor

### Research Papers
- **TLSNotary**: https://tlsnotary.org/
- **DECO (Chainlink)**: https://arxiv.org/pdf/1909.00938
- **Reclaim Protocol**: https://docs.reclaimprotocol.org/

---

**Report Date:** 2025-11-13
**Research Scope:** Native mobile app HTTPS traffic capture
**Platforms:** iOS and Android
**Primary Sources:** Previous research log, web search, documentation review
**Status:** Comprehensive analysis complete
