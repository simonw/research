# Quick Start - Frida Native Hooks Fix

## The Problem (30 seconds)
Frida native connection hooks were failing because they tried to redirect traffic to `127.0.0.1:8080` (localhost within emulator), but mitmproxy is on the host accessible via `10.0.2.2`.

## The Solution (One Line Change)
**File**: `entrypoint.sh` line 251
```bash
# BEFORE:
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}

# AFTER:
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
```

## Why It Works
- `10.0.2.2` = Android's special alias for the host/container gateway
- `127.0.0.1` = the emulator itself (not the host)
- Apps now connect to the correct mitmproxy address

## Additional Improvements
Also added:
1. Certificate validation (catch format errors early)
2. Dynamic status message (shows actual proxy address)

## Files Changed
- `/native-app-traffic-capture/android-mitm-mvp/entrypoint.sh` (3 changes, fully backward compatible)

## Testing
1. Rebuild Docker image
2. Run container
3. Check status message shows "Proxy: 10.0.2.2:8080"
4. Open Chrome, navigate to https://www.google.com
5. Check mitmproxy UI for captured HTTPS traffic

## More Details
- `README.md` - Full testing guide
- `ROOT_CAUSE_ANALYSIS.md` - Detailed root cause analysis
- `NETWORKING_EXPLANATION.md` - Android networking details
- `CHANGES.md` - All code changes documented

## Questions?
See `README.md` FAQ section or `NETWORKING_EXPLANATION.md` for deep technical details.
