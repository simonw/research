# Frida Native Connection Hooks - Debugging & Fixes

## Summary

This investigation identified and fixed the root cause of Frida native connection hooks failing to intercept Android app traffic in the Android MITM MVP environment.

### Root Cause
The Frida native hooks were configured to redirect traffic to `127.0.0.1:8080` (localhost within the emulator), but mitmproxy is running on the container host, which the emulator accesses via `10.0.2.2:8080`.

### Impact
- Android apps couldn't reach the proxy
- All native connections failed with "fd X to null (-1)"
- Certificate pinning bypass didn't work because traffic wasn't being intercepted

## Fixes Applied

### Primary Fix: Proxy Address (Critical)

**File**: `entrypoint.sh` line 253
**Change**: Default proxy host from `127.0.0.1` to `10.0.2.2`

```bash
# BEFORE:
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}

# AFTER:
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
```

**Why**: Android emulator uses `10.0.2.2` as the special alias for the host/container gateway. This is where mitmproxy is actually running.

**Effect**:
- System proxy configuration now points to correct address
- Frida native hooks now redirect to correct address
- Both paths use the same, reachable proxy endpoint

### Secondary Fix: Certificate Validation (Important)

**File**: `entrypoint.sh` lines 320-343
**Change**: Added certificate validation and cleanup before injection

```python
# Clean certificate: strip whitespace and normalize line endings
cert = cert.strip()
cert = cert.replace('\r\n', '\n')
cert = cert.replace('\r', '\n')

# Validate the certificate format before injection
if not cert.startswith('-----BEGIN CERTIFICATE-----'):
    raise ValueError("Certificate missing BEGIN CERTIFICATE header")
if not cert.endswith('-----END CERTIFICATE-----'):
    raise ValueError("Certificate missing END CERTIFICATE footer")

# Verify substitution succeeded
if f"const CERT_PEM = `{escaped_cert}`;" not in config:
    raise ValueError("Failed to substitute CERT_PEM in config.js")
```

**Why**: The certificate injection was not validating the PEM format, which could lead to silent failures. Unexpected line endings or whitespace could cause the Frida config validation to fail.

**Effect**:
- Catches certificate format issues early with clear error messages
- Normalizes line endings to prevent platform-specific issues
- Validates substitution actually occurred

### Tertiary Fix: Status Message (Polish)

**File**: `entrypoint.sh` line 432
**Change**: Dynamic proxy display instead of hardcoded value

```bash
# BEFORE:
echo "  - Proxy: 127.0.0.1:8080"

# AFTER:
echo "  - Proxy: ${ANDROID_PROXY_HOST}:${ANDROID_PROXY_PORT}"
```

**Why**: The final status message was showing a hardcoded address that didn't reflect actual configuration.

**Effect**:
- Users see the actual proxy address being used
- Makes it clear what environment variables took effect

## Technical Details

### Android Emulator Networking

In Android emulator containers:
- `127.0.0.1` = the emulator itself (localhost within emulator)
- `10.0.2.1` = the emulator's gateway/default route
- `10.0.2.2` = **the host machine / container gateway** (special Android alias)
- `192.168.x.x` = usually unavailable in emulator

All host services (including mitmproxy) running on the container are accessible via `10.0.2.2` from the emulator's perspective.

### Frida Native Connect Hook Flow

1. App initiates TLS connection to external server (e.g., `74.125.132.94:443`)
2. Frida `native-connect-hook.js` intercepts the `connect()` syscall
3. Hook rewrites the target address to `PROXY_HOST:PROXY_PORT`
4. Socket connects to proxy instead of original destination
5. Proxy sees the connection, performs MITM handshake
6. App's TLS certificate validation is bypassed by `native-tls-hook.js`

**With the fix**:
- Step 3: Address rewritten to `10.0.2.2:8080` (reachable from emulator)
- Step 4: Socket successfully connects to mitmproxy
- Steps 5-6: MITM works as designed

**Before the fix**:
- Step 3: Address rewritten to `127.0.0.1:8080` (dead address from emulator)
- Step 4: Socket connection fails
- Result: "fd X to null (-1)" in logs, no traffic captured

## Testing the Fixes

### Prerequisites
- The fixes have been applied to `/entrypoint.sh`
- Container needs to be rebuilt: `docker build -t android-mitm-mvp .`

### Test Case 1: Verify Proxy Configuration

```bash
# Run the container
docker run -it --rm \
  --privileged \
  -p 6080:6080 \
  -p 8081:8081 \
  -e APP_PACKAGE=com.android.chrome \
  android-mitm-mvp

# Check the output shows correct proxy:
# Should show: "- Proxy: 10.0.2.2:8080" (NOT 127.0.0.1)
```

### Test Case 2: Verify Frida Config

```bash
# In container, check the generated Frida config
docker exec <container> grep "const PROXY_HOST" /frida-scripts/config.js

# Should output:
# const PROXY_HOST = '10.0.2.2';
```

### Test Case 3: Verify Traffic Capture

1. Wait for setup to complete (~2-3 minutes)
2. Open noVNC: http://localhost:6080
3. Open mitmproxy: http://localhost:8081 (password: mitmproxy)
4. In noVNC, open Chrome and navigate to https://www.google.com
5. Check mitmproxy UI for captured HTTPS traffic
6. Should see requests/responses decrypted

### Test Case 4: Verify Certificate Injection

```bash
# In container, check certificate was properly injected
docker exec <container> grep -A2 "const CERT_PEM" /frida-scripts/config.js | head -5

# Should show the actual certificate content (not placeholder text)
# Should NOT show error about BEGIN CERTIFICATE header
```

### Test Case 5: Verify with Custom Proxy Address

```bash
# Test that explicit ANDROID_PROXY_HOST env var still works
docker run -it --rm \
  --privileged \
  -p 6080:6080 \
  -p 8081:8081 \
  -e APP_PACKAGE=com.android.chrome \
  -e ANDROID_PROXY_HOST=10.0.2.2 \
  android-mitm-mvp

# Should use the explicit value, not override it
```

## Verification Checklist

- [ ] Build container with fixes applied
- [ ] Container starts without errors
- [ ] Proxy shown in status is `10.0.2.2:8080`
- [ ] Frida config.js shows `const PROXY_HOST = '10.0.2.2'`
- [ ] No certificate format errors in Frida app log
- [ ] Chrome (or other app) starts successfully
- [ ] mitmproxy shows incoming connections from emulator
- [ ] HTTPS traffic appears decrypted in mitmproxy UI
- [ ] Can navigate websites in noVNC and see traffic captured

## Logs to Check

### mitmproxy Log
```bash
docker exec <container> tail -f /var/log/mitmproxy.log
```
Expected: Incoming connections from emulator and decrypted HTTPS traffic

### Frida Server Log
```bash
docker exec <container> tail -f /var/log/frida-server.log
```
Expected: Server started successfully, ready for clients

### Frida App Log
```bash
docker exec <container> tail -f /var/log/frida-app.log
```
Expected:
- Certificate validated and injected
- Native hooks initialized
- Connection interception messages with `10.0.2.2:8080`
- NO "null (-1)" errors (or they resolve now)

## Performance Implications

The fixes have minimal impact:
- **Primary fix**: Changes default value, no performance change
- **Certificate fix**: Adds validation at startup (once per container run), negligible overhead
- **Status message fix**: Cosmetic only, zero impact

## Backward Compatibility

All changes are backward compatible:
- Users can still explicitly set `ANDROID_PROXY_HOST` and `ANDROID_PROXY_PORT`
- The defaults only apply when environment variables are not set
- Existing Docker run commands that use environment variables will continue to work

## Files Modified

1. `/Users/kahtaf/Documents/workspace_kahtaf/research/native-app-traffic-capture/android-mitm-mvp/entrypoint.sh`
   - Line 253: Changed default proxy host to `10.0.2.2`
   - Lines 320-343: Added certificate validation and cleanup
   - Line 432: Made status message dynamic

## Related Files (Not Modified)

- `frida-scripts/config.js` - Contains the template that gets modified
- `frida-scripts/native-connect-hook.js` - Contains the native hook logic
- `frida-scripts/native-tls-hook.js` - Contains the TLS validation bypass

These files don't need changes because they are generic implementations that work correctly once the configuration is right.

## Future Improvements

### Recommended
1. Make DEBUG_MODE configurable via environment variable
2. Add validation of fd after address rewrite in native-connect-hook.js
3. Add more detailed logging of successful proxy redirects

### Optional
1. Support for SOCKS5 proxy (currently disabled)
2. Custom certificate validation rules
3. Per-app configuration overrides

## References

- Android Emulator Networking: https://developer.android.com/studio/run/emulator-networking
- Frida Documentation: https://frida.re/docs/
- HTTP Toolkit Frida Scripts: https://github.com/httptoolkit/frida-interception-and-unpinning
- mitmproxy Documentation: https://docs.mitmproxy.org/

## Questions

**Q: Why was the proxy address set to 127.0.0.1 in the first place?**
A: This was likely copied from a non-emulator context where 127.0.0.1 would be correct. In a containerized environment with an emulator, the special alias 10.0.2.2 is required.

**Q: Will this work on non-container environments?**
A: Yes - the 10.0.2.2 address is an Android emulator standard that works in all contexts (local Docker, GCP VM, etc.). It's the correct address to use.

**Q: What if someone needs to use a different proxy address?**
A: They can explicitly set the `ANDROID_PROXY_HOST` environment variable, which will override the default.

**Q: Why not use 10.0.2.1 (the gateway)?**
A: 10.0.2.1 is the emulator's default gateway, but services on the host (like mitmproxy) are accessed via the special alias 10.0.2.2.
