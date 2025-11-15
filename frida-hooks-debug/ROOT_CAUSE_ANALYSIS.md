# Root Cause Analysis: Frida Native Connection Hooks Failing

## Executive Summary

**Root Cause Identified**: Incorrect proxy host address in Frida native connection hook configuration.

The system is configured to use `127.0.0.1:8080` as the proxy, but this address is **localhost from the emulator's perspective**, NOT the host where mitmproxy is running. The emulator sees `127.0.0.1` as itself, not the container host.

## The Bug

### Location 1: entrypoint.sh (Line 251)
```bash
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}
```

This sets the default proxy host to `127.0.0.1` (localhost in the emulator).

### Location 2: entrypoint.sh (Line 326-327)
```python
proxy_host = os.environ.get("ANDROID_PROXY_HOST", "10.0.2.2")
proxy_port = os.environ.get("ANDROID_PROXY_PORT", "8080")
```

The Python code has its OWN default of `10.0.2.2` in case ANDROID_PROXY_HOST is not set.

### The Problem

Since line 251 EXPORTS `ANDROID_PROXY_HOST`, when the Python code at line 326 runs, the environment variable IS already set (to `127.0.0.1`), so it uses that value instead of falling back to `10.0.2.2`.

Result:
- System proxy configured: `127.0.0.1:8080`
- Frida hook configured: `127.0.0.1:8080`
- Both point to LOCALHOST from the emulator's perspective
- mitmproxy is actually running at the container host, NOT at 127.0.0.1
- The native hook successfully rewrites the address to `127.0.0.1:8080`
- But the emulator can't connect because that's a dead address (the emulator itself)

### Why It Appears to Work Partially

1. **System proxy setting** appears to work because:
   - System HTTP proxy is handled by the Android framework, which internally resolves it somehow
   - OR it fails silently for HTTP proxies

2. **Direct curl test works** because:
   - The `curl` command is run FROM the container host, not from inside the emulator
   - It can reach `127.0.0.1:8080` directly

3. **Android apps fail** because:
   - Apps inside the emulator try to connect to `127.0.0.1:8080`
   - That address doesn't exist in the emulator's network namespace
   - The native hook successfully rewrites the connect() call but to a non-routable address
   - Result: "fd X to null (-1)" - the connection fails

## Evidence

### Error Message
```
Manually intercepting tcp6 connection to ::ffff:74.125.132.94:443
intercepting tcp6 fd 102 to null (-1)
```

Translation: The hook intercepted the connection and rewrote it, but when it tried to actually connect to the new address (127.0.0.1:8080), the connection failed, leaving fd 102 with a null peer address.

### The Correct Address

In Android emulator networking:
- `127.0.0.1` = the emulator itself (localhost within emulator)
- `10.0.2.2` = the host machine/container (special alias in Android)
- `10.0.2.1` = the emulator's gateway

mitmproxy is running at the container level, accessible from the emulator via `10.0.2.2:8080`.

## The Fix

### Option 1: Fix the Default (RECOMMENDED)
Change line 251 in entrypoint.sh to use the correct default:

```bash
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
```

This ensures:
1. System proxy is configured to the correct address: `10.0.2.2:8080`
2. Environment variable is set before Python code runs
3. Frida config uses the same correct address: `10.0.2.2:8080`
4. Both system HTTP proxy and native hooks point to the same reachable address

### Option 2: Remove the Default
Let the environment variable control both:
```bash
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-""}  # No default
```
And rely on the Python code to provide the correct default `10.0.2.2`.

However, this is fragile because if ANDROID_PROXY_HOST is empty, line 255-257 will set an empty proxy.

### Option 3: Use Different Defaults
Set different defaults for system proxy vs. Frida hooks:
```bash
# System proxy (can be more flexible)
SYSTEM_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}

# Frida config (must point to host)
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
```

But this creates inconsistency and confusion.

## Recommended Fix

**Option 1 is the cleanest:**

In `/Users/kahtaf/Documents/workspace_kahtaf/research/native-app-traffic-capture/android-mitm-mvp/entrypoint.sh` at line 251:

BEFORE:
```bash
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}
```

AFTER:
```bash
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
```

This single change ensures:
1. Correct system proxy configuration
2. Correct Frida native hook destination
3. System HTTP proxy and native hooks use the same address
4. Apps can reach mitmproxy

## Secondary Issues to Verify

After fixing the proxy address, verify:

1. **Certificate validity**: The certificate injection may still have issues. Check that `CERT_PEM` in config.js is valid PEM format after substitution.

2. **Socket fd validation**: The native-connect-hook.js tries to get `Socket.peerAddress(fd)` after rewrite. If the fd becomes invalid, this could cause issues. Add validation.

3. **IPv6 address arithmetic**: The native-connect-hook.js has complex byte offset calculations for IPv4 vs IPv6. Verify these are correct:
   - IPv4: offset by 4 (2 bytes family + 2 bytes port), then write 4 bytes
   - IPv6: offset by 8 (2 family + 2 port + 4 flowinfo), then write 16 bytes

4. **PROXY_SUPPORTS_SOCKS5**: Currently set to false. This is fine for direct proxy, but worth noting.

## Testing the Fix

1. Change the default proxy host to `10.0.2.2`
2. Rebuild the container
3. Run with default environment (no ANDROID_PROXY_HOST set)
4. Verify in logs that Frida config shows:
   ```
   const PROXY_HOST = '10.0.2.2';
   ```
5. Check mitmproxy for incoming traffic from emulator
6. Confirm apps can access the network through mitmproxy

## Impact Analysis

This change is:
- **Safe**: Only changes the default when ANDROID_PROXY_HOST is not explicitly set
- **Backward compatible**: Users who explicitly set ANDROID_PROXY_HOST will not be affected
- **Correct**: Aligns with Android emulator's documented special addresses
- **Consistent**: System proxy and native hooks both use the same address
