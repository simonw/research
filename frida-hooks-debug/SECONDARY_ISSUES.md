# Secondary Issues - Certificate Injection & Config Validation

## Issue 1: PEM Certificate Format Validation

### Current Implementation
In `entrypoint.sh` (line 318-325), the certificate is injected into config.js:

```python
escaped_cert = cert.replace('`', '\\`')

config = re.sub(
    r"const CERT_PEM = `.*?`;",
    f"const CERT_PEM = `{escaped_cert}`;",
    config,
    flags=re.S
)
```

### Potential Problems

1. **Backtick escaping** - Only backticks are escaped, but the certificate may contain other special characters
2. **Newline handling** - PEM certificates have multiple lines; the template literal (backticks) should handle this correctly, but `\r\n` line endings could cause issues
3. **No validation** - The code doesn't verify that the substitution was successful

### Error from Problem Statement
```
Error: Your certificate should be in PEM format, starting & ending with a BEGIN CERTIFICATE & END CERTIFICATE header/footer
```

This error is thrown by `config.js` at line 73 when validating the CERT_PEM. It suggests the certificate doesn't match the expected format after injection.

### Root Cause Analysis

The `pemToDer` function in `config.js` (lines 140-161) expects:
```javascript
pemLines[0] === '-----BEGIN CERTIFICATE-----'
pemLines[pemLines.length - 1] === '-----END CERTIFICATE-----'
```

But after injection, the certificate might have:
1. Leading/trailing whitespace
2. Windows line endings (`\r\n` instead of `\n`)
3. The closing line might not be exactly at the end

### Recommended Fix

Modify `entrypoint.sh` to clean the certificate before injection:

```python
# Clean certificate: strip whitespace and normalize line endings
cert = cert.strip()
cert = cert.replace('\r\n', '\n')
cert = cert.replace('\r', '\n')

# Validate the certificate format
if not cert.startswith('-----BEGIN CERTIFICATE-----'):
    raise ValueError("Certificate missing BEGIN CERTIFICATE header")
if not cert.endswith('-----END CERTIFICATE-----'):
    raise ValueError("Certificate missing END CERTIFICATE footer")

# Escape backticks (template literal syntax)
escaped_cert = cert.replace('`', '\\`')

# Perform substitution
config = re.sub(
    r"const CERT_PEM = `.*?`;",
    f"const CERT_PEM = `{escaped_cert}`;",
    config,
    flags=re.S
)
```

## Issue 2: Socket File Descriptor Validation

### Current Implementation
In `native-connect-hook.js`, after rewriting the address, the debug output calls:

```javascript
const address = Socket.peerAddress(fd);
console.debug(`${this.state} ${sockType} fd ${fd} to ${JSON.stringify(address)} (${retval.toInt32()})`);
```

### Problem
If the socket fd becomes invalid or the connection hasn't completed, `Socket.peerAddress(fd)` may return `null` or an error, which gets logged as `"null (-1)"`.

### Current Status
With the proxy address fix (10.0.2.2), this should resolve itself, but it's worth adding validation:

```javascript
onLeave: function (retval) {
    if (this.state === 'ignored') return;

    // ... SOCKS5 handling ...

    if (DEBUG_MODE && this.state === 'intercepting') {
        const fd = this.sockFd;
        const sockType = Socket.type(fd);

        // Validate fd is still valid before getting peer address
        if (fd > 0) {
            try {
                const address = Socket.peerAddress(fd);
                console.debug(`${this.state} ${sockType} fd ${fd} to ${JSON.stringify(address)} (${retval.toInt32()})`);
            } catch (e) {
                console.debug(`${this.state} ${sockType} fd ${fd} - couldn't get peer address: ${e.message}`);
            }
        } else {
            console.debug(`${this.state} invalid fd ${fd} (${retval.toInt32()})`);
        }
    }
}
```

## Issue 3: IPv6 Address Rewrite Validation

### Current Implementation
Lines 76-80 in `native-connect-hook.js`:

```javascript
const hostBytes = isIPv6
    // 16 bytes offset by 8 (2 for family, 2 for port, 4 for flowinfo):
    ? new Uint8Array(addrData.slice(8, 8 + 16))
    // 4 bytes, offset by 4 (2 for family, 2 for port)
    : new Uint8Array(addrData.slice(4, 4 + 4));
```

And lines 127-131:

```javascript
if (isIPv6) {
    // Skip 8 bytes: 2 family, 2 port, 4 flowinfo
    addrPtr.add(8).writeByteArray(PROXY_HOST_IPv6_BYTES);
} else {
    // Skip 4 bytes: 2 family, 2 port
    addrPtr.add(4).writeByteArray(PROXY_HOST_IPv4_BYTES);
}
```

### Validation Points

1. **Address Data Length** - Should validate that addrData.byteLength is sufficient:
   - IPv4: at least 8 bytes (2+2+4)
   - IPv6: at least 24 bytes (2+2+4+16)

2. **Byte Array Lengths** - Should validate:
   - PROXY_HOST_IPv4_BYTES: exactly 4 bytes
   - PROXY_HOST_IPv6_BYTES: exactly 16 bytes

These appear to be correct in the current implementation, but adding assertions would help catch issues.

## Issue 4: DEBUG_MODE Impact

The Frida config now sets `DEBUG_MODE = true` (line 330), which generates copious logging:

```javascript
const DEBUG_MODE = true;
```

This is good for debugging but may impact performance. The native-connect-hook.js logs:
- Every connection attempt (line 118)
- Every onLeave event with socket details (line 176-178)

Consider whether to keep it enabled or make it configurable.

## Summary of Secondary Fixes

### Must Fix (Correctness)
1. **Certificate injection** - Add validation and cleanup of PEM format

### Should Fix (Robustness)
2. **Socket fd validation** - Add try-catch around Socket.peerAddress()
3. **Debug logging** - Make configurable or add guards to reduce verbosity

### Nice to Have (Defensive)
4. **Address structure validation** - Add length checks before slicing

## Implementation Priority

After merging the primary fix (proxy address), implement in this order:
1. Certificate injection validation (Issue 1)
2. Debug mode toggle (Issue 4)
3. Socket fd validation (Issue 2)
4. Address structure validation (Issue 3)
