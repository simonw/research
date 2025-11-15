# Visual Explanation - Frida Hooks Fix

## Network Architecture Comparison

### BEFORE THE FIX (BROKEN)

```
┌─────────────────────────────────────────────────────┐
│                   Container / Host                   │
│  ┌──────────────────────────────────────────────┐   │
│  │ mitmproxy running on 0.0.0.0:8080            │   │
│  │ Accessible from outside at:                  │   │
│  │  - 127.0.0.1:8080 (host localhost)           │   │
│  │  - 10.0.2.2:8080  (from emulator)            │   │
│  └──────────────────────────────────────────────┘   │
│                                                       │
│  ┌──────────────────────────────────────────────┐   │
│  │ Android Emulator                              │   │
│  │ ┌────────────────────────────────────────┐   │   │
│  │ │ Chrome App                              │   │   │
│  │ │ Tries to reach: 74.125.132.94:443      │   │   │
│  │ └────────────────────────────────────────┘   │   │
│  │           ↓ (Frida Hook Intercepts)          │   │
│  │ ┌────────────────────────────────────────┐   │   │
│  │ │ Redirected to: 127.0.0.1:8080          │   │   │
│  │ │ (THIS IS THE EMULATOR ITSELF!)          │   │   │
│  │ │ ✗ Connection fails → "fd X to null"    │   │   │
│  │ └────────────────────────────────────────┘   │   │
│  │                                               │   │
│  │ mitmproxy is NOT at 127.0.0.1 inside         │   │
│  │ the emulator!                                 │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

### AFTER THE FIX (WORKING)

```
┌─────────────────────────────────────────────────────┐
│                   Container / Host                   │
│  ┌──────────────────────────────────────────────┐   │
│  │ mitmproxy running on 0.0.0.0:8080            │   │
│  │ Accessible from outside at:                  │   │
│  │  - 127.0.0.1:8080 (host localhost)           │   │
│  │  - 10.0.2.2:8080  (from emulator)            │   │
│  └──────────────────────────────────────────────┘   │
│         ↑                                            │
│         │ (Traffic from emulator)                    │
│  ┌──────────────────────────────────────────────┐   │
│  │ Android Emulator                              │   │
│  │ ┌────────────────────────────────────────┐   │   │
│  │ │ Chrome App                              │   │   │
│  │ │ Tries to reach: 74.125.132.94:443      │   │   │
│  │ └────────────────────────────────────────┘   │   │
│  │           ↓ (Frida Hook Intercepts)          │   │
│  │ ┌────────────────────────────────────────┐   │   │
│  │ │ Redirected to: 10.0.2.2:8080           │   │   │
│  │ │ (HOST/CONTAINER GATEWAY!)               │   │   │
│  │ │ ✓ Connection succeeds → reaches proxy  │   │   │
│  │ │ ✓ Traffic is captured and decrypted    │   │   │
│  │ └────────────────────────────────────────┘   │   │
│  └──────────────────────────────────────────────┘   │
│                                                       │
│ Android IP Addresses from Emulator Perspective:     │
│ - 127.0.0.1 = the emulator itself                   │
│ - 10.0.2.1  = default gateway                       │
│ - 10.0.2.2  = the host/container                    │
└─────────────────────────────────────────────────────┘
```

## The One-Line Fix

```diff
- export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}
+ export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
```

## Traffic Flow Diagram

### What Happens With The Fix

```
App HTTPS Request
         ↓
    TCP Connect to google.com:443
         ↓
    [Frida native-connect-hook intercepts]
         ↓
    Rewrites address to 10.0.2.2:8080
         ↓
    [Socket connects to mitmproxy]
         ↓
    mitmproxy MITM Handshake
         ↓
    [Frida native-tls-hook validates cert]
         ↓
    Decrypted HTTPS traffic → mitmproxy captures it
         ↓
    Response flows back through proxy
         ↓
    App receives decrypted response
```

## Address Space Visualization

### Inside the Emulator

```
Network Namespace: Emulator

127.0.0.1          10.0.2.1           10.0.2.2
   │                  │                   │
   │                  │                   │
Emulator         Gateway              HOST
 App           (Default Route)       (Container)
(Localhost)                      (WHERE MITM IS!)
   │                  │                   │
   └────────┬─────────┴───────┬──────────┘
            │                 │
        ✗ WRONG         ✓ CORRECT
    (localhost)      (host gateway)
    "dead end"      (connects to mitmproxy)
```

## Configuration Update

### Environment Variables

Before and after the fix:

```
BEFORE:
ANDROID_PROXY_HOST=127.0.0.1
  ↓ (used by entrypoint.sh)
  ├─ System proxy config: 127.0.0.1:8080 ✗
  └─ Frida config: 127.0.0.1:8080 ✗
    (Both point to unreachable address)

AFTER:
ANDROID_PROXY_HOST=10.0.2.2
  ↓ (used by entrypoint.sh)
  ├─ System proxy config: 10.0.2.2:8080 ✓
  └─ Frida config: 10.0.2.2:8080 ✓
    (Both point to mitmproxy on host)
```

## Symptom-to-Root-Cause Flow

```
Symptom: "fd 102 to null (-1)"
         ↑
         │ What does this mean?
         │
    Socket connection returned -1 (error)
         ↑
         │ Why would socket fail to connect?
         │
    Target address is unreachable
         ↑
         │ What address was being used?
         │
    127.0.0.1:8080
         ↑
         │ Where does the emulator see that?
         │
    The emulator itself (localhost)
         ↑
         │ But where is mitmproxy actually?
         │
    On the host/container at 10.0.2.2:8080
         ↑
         │ FIX: Use 10.0.2.2 as default
         │
    ✓ Connections now reach mitmproxy
```

## The Key Insight

**SAME ADDRESS, DIFFERENT MEANING**

```
When you say "127.0.0.1:8080"...

From the Host perspective:
  ↓
  "Listen on my own localhost"
  mitmproxy can listen here ✓

From the Emulator perspective:
  ↓
  "Listen on the emulator itself"
  Nothing's listening there! ✗
  (mitmproxy is on the host, not emulator)

When you say "10.0.2.2:8080"...

From the Host perspective:
  ↓
  "Don't listen here"
  (This is for external access)

From the Emulator perspective:
  ↓
  "That's the host/container gateway"
  (Where services like mitmproxy are) ✓
```

## Before and After Status Message

### Before
```
========================================
🎉 Setup complete!
========================================

Traffic Capture Status:
  - App: com.android.chrome
  - Proxy: 127.0.0.1:8080  ← HARDCODED, WRONG
  - Frida: ✓ Active with certificate unpinning
  - Capabilities: Can capture pinned apps
```

Result: User doesn't see that proxy is wrong

### After
```
========================================
🎉 Setup complete!
========================================

Traffic Capture Status:
  - App: com.android.chrome
  - Proxy: 10.0.2.2:8080  ← DYNAMIC, CORRECT
  - Frida: ✓ Active with certificate unpinning
  - Capabilities: Can capture pinned apps
```

Result: User sees correct proxy address being used

## Implementation Details

### What Gets Modified

```
entrypoint.sh (line 253)
    ↓ Sets environment variable
ANDROID_PROXY_HOST=10.0.2.2
    ↓ Used in shell commands
adb shell settings put global http_proxy "10.0.2.2:8080"
    ↓ Also used in Python code
proxy_host = os.environ.get("ANDROID_PROXY_HOST", "10.0.2.2")
    ↓ Injected into config.js via regex
const PROXY_HOST = '10.0.2.2';
    ↓ Used by native-connect-hook.js
Frida redirects connections to 10.0.2.2:8080
    ↓
Traffic reaches mitmproxy ✓
```

## Summary

**The problem**: Using `127.0.0.1` (emulator localhost) instead of `10.0.2.2` (host gateway)

**The fix**: Change one default from `127.0.0.1` to `10.0.2.2`

**The result**: Traffic can now reach mitmproxy and be captured

**The impact**: Frida native hooks now work correctly for all apps
