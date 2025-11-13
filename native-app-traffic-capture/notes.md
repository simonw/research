# Research Notes: Native App Traffic Capture

## Date: 2025-11-13

## Objective
Research approaches for capturing HTTPS traffic from native mobile apps (iOS and Android) with the following requirements:
1. Must work on any app without modifying the app itself
2. Very easy for users to set up (installing certificates or proxies is bad UX)
3. Must be able to read and capture HTTPS request bodies

## Previous Research Summary

### Key Finding: The Android Problem
**Android 7+ (API 24+) apps do not trust user-installed certificates by default.**
- Apps must explicitly opt-in via `network_security_config`
- VpnService cannot inject CAs into system trust store
- ~80-90% of modern apps will not work with user-level MITM
- This is a fundamental OS restriction that cannot be bypassed without root access

### Key Finding: iOS vs Android Security Models
| iOS | Android |
|-----|---------|
| Apps trust user CAs by default | Apps ignore user CAs (API 24+) |
| Network Extensions allow local MITM | VpnService cannot override trust store |
| Proxyman-style apps work (via TestFlight) | Google blocks system-wide MITM |
| Only pinned apps fail | Pinned apps + default security fail |

### Approaches Evaluated in Previous Research

#### 1. MITM Proxy Solutions
**Proxyman iOS** ✅
- Uses NEPacketTunnelProvider + root CA + in-app proxy
- Works for ~70% of apps (except pinned apps)
- Cannot ship via App Store (TestFlight only)
- **Verdict**: Gold standard for iOS, not possible on Android

**Proxyman Android Equivalent** ❌
- Explicitly blocked by Android due to user CA restrictions
- **Verdict**: Not viable

**Charles Proxy / mitmproxy** ⚠️
- Works on iOS for most apps
- Fails on 80-90% of Android apps
- Poor UX (manual setup)
- **Verdict**: Partially viable, better on iOS

#### 2. VPN-Based Solutions
**HTTP Toolkit** ⚠️
- Uses ADB to create VPN and inject certificate
- Only works on apps that trust user CAs
- Requires developer setup
- **Verdict**: Limited, technical setup required

**PCAPdroid** ⚠️
- Android VpnService + built-in mitmproxy
- Can see metadata but not HTTPS bodies for most apps
- **Verdict**: Metadata-only solution

#### 3. Open-Source Projects

**ProxyPin** ⭐ (Best Reference)
- Flutter-based cross-platform MITM proxy
- 10k+ GitHub stars, Apache-2.0 license
- VPN service on both iOS and Android
- Full feature set: filtering, rewriting, scripting
- **Verdict**: Best open-source starting point, same limitations as all MITM approaches

**DeviceFarmer/OpenSTF**
- Open-source device farm for remote Android control
- MJPEG streaming (30-40 FPS)
- **Verdict**: Viable for self-hosted device farms

**GADS**
- Free alternative to AWS Device Farm
- Supports Android, iOS, Tizen, webOS
- WebRTC/MJPEG streaming
- **Verdict**: Viable for serious self-hosted farms

**Docker-Android**
- Android emulators in Docker with browser access
- Can pre-install system CA
- **Verdict**: Highly viable for Android emulation

**AOSP Cloud-AVD**
- Google's reference for cloud-based emulator streaming
- WebRTC + React UI + TURN server
- **Verdict**: Best performance but requires assembly

#### 4. Alternative Approaches

**Frida-Based Auto-Unpinning** ⚠️
- Dynamic instrumentation to disable certificate pinning
- Requires root or debuggable build
- **Verdict**: Technically viable but not user-friendly

**App Virtualization (VirtualApp)** ⚠️
- Run apps in sandbox with custom trust store
- High-security apps detect virtualization
- **Verdict**: Partially viable

**APK Repackaging** ❌
- Violates "no app modification" requirement
- **Verdict**: Not viable

**Cloud-Controlled Emulator** ✅ ⭐
- Run apps in cloud emulator with system CA pre-installed
- Stream screen to user
- Cloud-side MITM proxy
- **Verdict**: Most comprehensive solution, meets all requirements

### Critical Technical Challenges Identified

1. **TLS Session State Problem**: Cannot "replay" encrypted packets from phone through cloud emulator - TLS session keys don't match
2. **Emulator Detection**: High-security apps use Play Integrity API
3. **QUIC/HTTP3**: Harder to intercept than HTTP/2
4. **Certificate Pinning**: Still blocks even with system CA
5. **iOS Cost**: Requires macOS VMs or real device farm
6. **Performance**: Full AVD per session is resource-intensive

## Additional Research Needed

### Areas Not Fully Explored
1. **zkTLS solutions** - Were mentioned but not deeply analyzed in the conversation
2. **Recent advances** in Android security (Android 14+)
3. **WireGuard-based tunneling** performance characteristics
4. **Commercial solutions** pricing and capabilities comparison
5. **Network.framework** on iOS (newer than NEPacketTunnelProvider)
6. **eBPF-based** packet capture possibilities
7. **Browser-based approaches** for web API traffic only

### Questions to Investigate
- Can we use iOS Network.framework for better performance?
- What's the actual cost of running cloud emulators at scale?
- Are there any new Android APIs that allow trusted certificate injection?
- Can we use WebAssembly for cross-platform MITM proxy logic?
- What about hybrid approach: local capture for metadata, cloud for decrypt?

---

## New Research Session Started

### Investigation 1: Android 14 Security Changes (November 2024)

**Critical Update: Android 14 System Certificate Restrictions**
- Android 14 moved system CA certificates to `/apex/com.android.conscrypt/cacerts`
- This directory is fully immutable with per-process mount namespacing
- Each running process has an independent mount, making traditional certificate injection impossible
- However, workarounds have been developed for rooted devices

**Current State (2024-2025)**:
- Multiple working solutions now exist to inject certificates on Android 14 despite restrictions
- Tools like `apk-mitm` can automatically patch apps to disable certificate pinning
- HTTP Toolkit's Frida scripts provide ready-to-use certificate pinning bypass
- Still requires either:
  - Root access
  - Source code modification (for your own apps)
  - APK repackaging (violates "no modification" requirement)

**Verdict**: Android 14 made the problem worse, not better. User certificate restrictions remain the fundamental blocker.

---

### Investigation 2: iOS Network.framework vs NEPacketTunnelProvider

**NEPacketTunnelProvider** (Current Standard)
- IP-layer tunneling, runs as app extension
- Common pattern for MITM proxies on iOS:
  1. NEPacketTunnelProvider intercepts system traffic
  2. SwiftNIO or Network.framework implements proxy server within extension
  3. Certificate management for TLS interception
- Used by: Proxyman iOS, Charles Proxy, Surge, Shadowrocket

**Network.framework** (Modern Alternative)
- Apple's modern alternative to sockets (introduced WWDC 2018)
- Provides direct access to TLS, TCP, UDP at transport layer
- **Does not replace NEPacketTunnelProvider** for VPN functionality
- Can be used **inside** NEPacketTunnelProvider to implement the proxy server
- Has ProxyConfiguration support for relay hops, HTTP CONNECT, SOCKS5
- Better performance and cleaner API than older socket APIs

**Proxygen** (Real-World Example)
- iOS app available on App Store
- Uses SwiftNIO to build high-performance HTTP proxy
- Runs within NEPacketTunnelProvider extension
- Confirms the hybrid approach is viable and approved by Apple (for TestFlight/Enterprise)

**Verdict**: Network.framework + NEPacketTunnelProvider is the modern approach, but doesn't solve the fundamental UX problem (certificate installation still required).

---

### Investigation 3: eBPF for Mobile Packet Capture

**Android eBPF Support** ✅
- Android uses eBPF extensively for network traffic monitoring
- Built-in functionality:
  - Socket tagging
  - Foreground/background traffic separation
  - Per-UID firewall
  - NAT46
- Every network packet on modern Android passes through eBPF programs
- Used by millions of Android devices daily

**iOS eBPF Support** ❌
- **No eBPF support on iOS**
- iOS uses XNU/Darwin kernel, not Linux
- eBPF is Linux-specific technology
- No equivalent on iOS

**2024 Developments**:
- TraceeShark: Integration of eBPF capture with Wireshark
- Network Observability eBPF Agent with packet capture capabilities
- PCAP format export from eBPF captures
- Predictions of more mobile eBPF applications beyond Android

**Limitations for Our Use Case**:
- eBPF can capture packets but **cannot decrypt HTTPS**
- eBPF provides metadata (IPs, ports, timing) but not encrypted payloads
- Still need MITM proxy approach for HTTPS body inspection
- Android only, no iOS equivalent

**Verdict**: eBPF is useful for metadata capture and network analysis, but doesn't solve HTTPS decryption requirement.

---

### Investigation 4: Commercial SaaS Device Farms (2024-2025)

**BrowserStack** ⭐
- 3,500+ real devices
- Network tab in DevTools for HTTP/S traffic
- Real-time monitoring of requests/responses
- Video recordings, console output, network traffic
- Network throttling simulation
- **How it works**: Re-signs app to embed custom proxy certificate
- **Limitation**: Apps with anti-tamper detection may fail

**LambdaTest** ⭐
- 10,000+ real devices
- Real-time HTTP/S traffic capture and analysis
- Domain-based filtering
- HAR file export
- **Limitations**:
  - Only available on Pro Plans
  - Apps from App Store/Play Store may not support network capture
  - Certificate-pinned apps fail unless hosts excluded
- Cross-platform: Android, iOS, web

**Sauce Labs** ⭐⭐
- Network Capture on Real Device Cloud
- HTTP/HTTPS traffic tracking
- Supports all native frameworks (Espresso, XCUITest, Appium)
- Cross-framework: Flutter, React Native, Cordova
- **Sauce Labs Forwarder v1.1.0**: MITM support for encrypted traffic
- HAR file export
- CI/CD pipeline integration

**AWS Device Farm** ❌
- **Does NOT support network traffic inspection natively**
- No Selenium DevTools features
- No request interception capabilities
- Workarounds: external proxy server, Wireshark on host, or use BrowserStack/Sauce Labs instead
- Limited debugging tools compared to competitors

**Pricing & Capabilities Summary**:
| Platform | HTTPS Inspection | Pinning Bypass | Real Devices | Pricing |
|----------|------------------|----------------|--------------|---------|
| BrowserStack | ✅ | ⚠️ (app re-signing) | 3,500+ | Enterprise |
| LambdaTest | ✅ | ⚠️ (limited) | 10,000+ | Pro+ |
| Sauce Labs | ✅ | ✅ (MITM forwarder) | Large fleet | Enterprise |
| AWS Device Farm | ❌ | ❌ | Good coverage | Pay-per-use |

**Verdict**: Sauce Labs has the most advanced network capture capabilities. All platforms face the same certificate pinning challenges. These are viable for enterprise use but expensive for individual users.

---

### Investigation 5: Alternative Approaches Not Covered

**Service Mesh / Sidecar Proxy Pattern**
- Primarily for backend microservices (Kubernetes, Istio, Linkerd)
- **Not applicable to mobile app traffic capture**
- Service mesh focuses on service-to-service communication (internal)
- API Gateway is the pattern for mobile app → backend (external)
- Could be used on the backend to capture API responses, but doesn't help with:
  - Client-side request bodies
  - Third-party API calls from mobile apps
  - Native SDK network traffic

**Verdict**: Not relevant for mobile app traffic capture use case.

---

### Investigation 6: New Tools and Approaches (2024)

**Reqable** (November 2024)
- New tool for intercepting traffic on non-rooted Android devices
- Similar to Charles Proxy / HTTP Toolkit
- Supports certificate unpinning techniques
- Still faces same Android 7+ user CA restrictions

**NetCapture** (2024)
- VPN-based capture on Google Play Store
- Can capture SSL packets via VPN without root
- Same limitations as PCAPdroid (metadata only for most apps)

**apk-mitm Tool** (Updated 2024)
- Automatically patches APKs to:
  - Trust user certificates
  - Strip pinned certificates
  - Support Android App Bundle format
- Requires APK repackaging (violates "no modification" requirement)

**objection** (2024)
- Bypass certificate pinning without rebuilding APK
- Uses Frida for runtime instrumentation
- Requires rooted device or debuggable app

**HTTP Toolkit Mobile Mode** (2024)
- "Android Device via ADB" mode
- Automated VPN setup + certificate injection via ADB
- Desktop proxy for traffic display
- Still limited by user CA restrictions

**Verdict**: No breakthrough solutions in 2024. All new tools face the same fundamental Android restrictions.

---

## Summary of Additional Research

### What Changed in 2024-2025?
1. **Android 14 made certificate injection harder** (even with root)
2. **Commercial device farms improved** (Sauce Labs MITM Forwarder, better HAR exports)
3. **eBPF on Android expanded** (more network monitoring capabilities)
4. **New tools emerged** (Reqable, updated apk-mitm) but face same restrictions
5. **iOS Network.framework adoption** (better API but same VPN approach)

### What Hasn't Changed?
1. **Android 7+ user CA restrictions** - still the fundamental blocker
2. **Certificate pinning** - still blocks all MITM approaches
3. **iOS App Store restrictions** - VPN apps still require TestFlight/Enterprise
4. **No magical "easy UX" solution** - all approaches require setup

### Best Solutions Remain:
1. **iOS**: Proxyman-style local VPN (moderate UX friction, works for ~70% of apps)
2. **Android**: Cloud emulator with system CA (complex infrastructure, works for ~95% of apps)
3. **Both**: Commercial device farm (expensive but turnkey)

### Most Promising New Development:
**Sauce Labs Forwarder v1.1.0 with MITM support** - First commercial SaaS with built-in MITM capabilities for encrypted traffic inspection. Most complete solution for enterprise use.

---

