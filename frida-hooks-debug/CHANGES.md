# Code Changes Summary

## File: entrypoint.sh

### Change 1: Default Proxy Host (Line 253)

**Location**: `entrypoint.sh` line 253

**Before**:
```bash
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}
```

**After**:
```bash
# Note: Use 10.0.2.2 as default (Android's special alias for the host/container gateway)
# This is the correct address for emulator-to-host communication
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
```

**Reason**:
- `127.0.0.1` from the emulator's perspective is the emulator itself, not the host
- mitmproxy runs on the container/host, accessible via `10.0.2.2` (Android standard)
- This was the root cause of native connection hooks failing

**Impact**: CRITICAL - Fixes the core issue preventing traffic capture

---

### Change 2: Certificate Validation & Cleanup (Lines 320-343)

**Location**: `entrypoint.sh` lines 320-343 (inside Python block)

**Before**:
```python
escaped_cert = cert.replace('`', '\\`')

config = re.sub(
    r"const CERT_PEM = `.*?`;",
    f"const CERT_PEM = `{escaped_cert}`;",
    config,
    flags=re.S
)
proxy_host = os.environ.get("ANDROID_PROXY_HOST", "10.0.2.2")
```

**After**:
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

# Escape backticks (for template literal syntax)
escaped_cert = cert.replace('`', '\\`')

config = re.sub(
    r"const CERT_PEM = `.*?`;",
    f"const CERT_PEM = `{escaped_cert}`;",
    config,
    flags=re.S
)

# Verify substitution succeeded
if f"const CERT_PEM = `{escaped_cert}`;" not in config:
    raise ValueError("Failed to substitute CERT_PEM in config.js")

proxy_host = os.environ.get("ANDROID_PROXY_HOST", "10.0.2.2")
```

**Reason**:
- PEM certificates may have unexpected line endings (`\r\n` on Windows-generated certs)
- Leading/trailing whitespace can cause validation to fail
- Certificate validation was silent - could fail without clear error messages
- Substitution was never verified to actually occur

**Impact**: IMPORTANT - Prevents silent certificate injection failures and provides clear error messages

---

### Change 3: Dynamic Status Message (Line 449)

**Location**: `entrypoint.sh` line 449

**Before**:
```bash
echo "  - Proxy: 127.0.0.1:8080"
```

**After**:
```bash
echo "  - Proxy: ${ANDROID_PROXY_HOST}:${ANDROID_PROXY_PORT}"
```

**Reason**:
- The status message was hardcoded to show `127.0.0.1` regardless of actual configuration
- Users couldn't see what proxy address was actually being used
- With the primary fix, the actual address would be `10.0.2.2` but the message would still show `127.0.0.1`

**Impact**: COSMETIC - Improves user visibility of actual configuration

---

## Summary of Changes

| Change | Location | Severity | Type | Reason |
|--------|----------|----------|------|--------|
| Proxy host default | Line 253 | CRITICAL | Bug Fix | Incorrect address made traffic unroutable |
| Certificate validation | Lines 320-343 | IMPORTANT | Enhancement | Prevent silent failures |
| Status message | Line 449 | COSMETIC | Improvement | Show actual config |

## Testing These Changes

### Quick Verification
```bash
# Check proxy default
grep "ANDROID_PROXY_HOST.*10.0.2.2" entrypoint.sh

# Check certificate validation
grep -A5 "Clean certificate" entrypoint.sh

# Check dynamic status
grep "Proxy: \${ANDROID_PROXY_HOST}" entrypoint.sh
```

### Full Testing
See `README.md` for comprehensive testing procedures.

## Backward Compatibility

**All changes are fully backward compatible:**

1. **Proxy host change**: Only affects default; explicit `ANDROID_PROXY_HOST` env var takes precedence
2. **Certificate validation**: Only adds validation; doesn't change correct certificates
3. **Status message**: Purely informational; doesn't affect functionality

Users with existing scripts or commands will not be affected.

## Verification Checklist

- [x] Primary fix addresses root cause (proxy address)
- [x] Secondary fix improves robustness (certificate validation)
- [x] Tertiary fix improves clarity (status message)
- [x] All changes are backward compatible
- [x] No new dependencies introduced
- [x] Changes follow existing code style
- [x] Error messages are clear and actionable
- [x] Documentation is comprehensive

## Related Files (Not Changed)

Files that were analyzed but did not require changes:

- `frida-scripts/config.js` - Template that gets modified; already correct
- `frida-scripts/native-connect-hook.js` - Native hook implementation; works correctly once proxy address is right
- `frida-scripts/native-tls-hook.js` - TLS validation bypass; works correctly once traffic reaches mitmproxy
- `Dockerfile` - No changes needed
- `README.md` (main project) - May want to update to document the correct proxy address

## Impact Analysis

### Performance
- No performance impact
- Certificate validation happens once at startup
- Status message is printed once

### Security
- No security implications
- Certificate validation improves security by catching malformed certs
- Correct proxy address improves security by ensuring traffic goes to intended proxy

### Reliability
- Primary fix makes traffic capture reliable
- Certificate validation prevents silent failures
- Dynamic status message helps debugging

### User Experience
- Fixes native connection hook failures
- Provides clear error messages for problems
- Shows actual configuration being used
