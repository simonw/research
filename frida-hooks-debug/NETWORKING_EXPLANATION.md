# Android Emulator Networking - Technical Deep Dive

## The Problem in 30 Seconds

Android emulator apps tried to connect to `127.0.0.1:8080`, which from the emulator's perspective is the emulator itself, not mitmproxy. mitmproxy is running on the host/container, which the emulator accesses via the special alias `10.0.2.2`.

## Android Emulator Network Architecture

### Special IP Addresses

When running an Android emulator in a container or VM:

| Address | What It Is | Accessible From |
|---------|-----------|-----------------|
| `127.0.0.1` | The emulator itself (localhost) | Emulator apps only |
| `10.0.2.1` | The default gateway | Emulator can reach networks beyond gateway |
| `10.0.2.2` | **The host/container (special alias)** | Emulator can reach host services |
| `192.168.x.x` | Usually not available | Generally not accessible to emulator |

### The Key Insight

**From the emulator's perspective:**
- `127.0.0.1` = "me" (the emulator)
- `10.0.2.2` = "the host machine" (where container services run)

**This is NOT the same as host Docker networking where:**
- `127.0.0.1` = localhost on the host machine
- `docker.host.internal` = the host machine

### Why `10.0.2.2`?

Android documents this as the reserved address that emulator uses to reach the host. This is:
- A Android convention, not specific to Docker
- The same on local Docker Desktop emulators
- The same on GCP VM emulators
- Used in all contexts where an emulator runs in a container

## The Flow: How Traffic Should Work

### Correct Flow (With Fix)

```
Emulator:Chrome App
    |
    | Makes HTTPS request to google.com:443
    v
[Chrome connects to 74.125.132.94:443]
    |
    | Frida native-connect-hook intercepts connect()
    v
[Hook rewrites address to 10.0.2.2:8080]
    |
    | Socket connects to host/container
    v
Container:mitmproxy (listening on 0.0.0.0:8080)
    |
    | Accepts connection
    v
[TLS handshake with app]
    |
    | Frida native-tls-hook validates cert
    v
[mitmproxy MITM proxies connection to real google.com]
    |
    | Response flows back through proxy
    v
Emulator:Chrome App [receives decrypted response]
```

### Broken Flow (Before Fix)

```
Emulator:Chrome App
    |
    | Makes HTTPS request to google.com:443
    v
[Chrome connects to 74.125.132.94:443]
    |
    | Frida native-connect-hook intercepts connect()
    v
[Hook rewrites address to 127.0.0.1:8080]  ← WRONG ADDRESS
    |
    | Tries to connect to 127.0.0.1:8080
    | This is the emulator itself!
    | Nothing listening on :8080 in emulator
    v
[Connection fails, fd becomes null/-1]
    |
    | Socket FD is invalid
    v
Emulator:Chrome App [connection fails]

Note: mitmproxy is still running on container
      but the app never reaches it
```

## The Debugging Clue

The error message was:
```
Manually intercepting tcp6 connection to ::ffff:74.125.132.94:443
intercepting tcp6 fd 102 to null (-1)
```

Translation:
- Line 1: Hook successfully intercepted the connection attempt
- Line 2: After rewriting the address, the socket fd 102 has a null peer address

This means:
1. ✓ The hook is working (it intercepted the connection)
2. ✓ The address rewrite happened (control reached the logging line)
3. ✗ But the socket couldn't connect (fd is null/-1)

The address was valid (parsed correctly), but the TARGET address `127.0.0.1:8080` wasn't reachable.

## Container Network Modes

### Docker Desktop (macOS/Windows)

When running on Docker Desktop with emulator container:
- `localhost` = host machine (from host perspective)
- Container sees host at `10.0.2.2` (via Docker's special alias)
- mitmproxy listening on `0.0.0.0:8080` inside container
- Accessible to emulator via `10.0.2.2:8080`

### GCP VM (Linux)

When running in GCP VM with Docker:
- `localhost` = the VM itself
- Container sees VM at `172.17.0.1` (default Docker gateway)
- BUT emulator inside container still uses `10.0.2.2` (Android convention)
- So we need to route `10.0.2.2` to `172.17.0.1` or use `10.0.2.2` directly

### Local Linux with Docker

Similar to GCP VM:
- `127.0.0.1` = host machine
- Container sees host at `172.17.0.1` (or custom bridge IP)
- Emulator inside container uses `10.0.2.2` (which Docker could map to host)

### The Solution: Use 10.0.2.2

The `10.0.2.2` address works correctly in ALL contexts because:
1. It's the Android standard (documented)
2. Android emulator always uses it
3. Container systems are configured to route it correctly
4. It's independent of the underlying container network

## Verifying Network Connectivity

### From Inside Emulator

```bash
# SSH into running container
docker exec -it <container> adb shell

# From emulator shell, test connectivity:
ping 10.0.2.2  # Should reach container/host
ping 10.0.2.1  # Should reach gateway
ping 127.0.0.1 # Should only reach emulator itself
curl http://10.0.2.2:8080  # Test mitmproxy connection
```

### From Container, Verify mitmproxy is Listening

```bash
# Check mitmproxy is bound to all interfaces
docker exec -it <container> netstat -tlnp | grep 8080
# Should show: 0.0.0.0:8080 (or :8080)

# Or use ss
docker exec -it <container> ss -tlnp | grep 8080
```

## Why HTTP vs HTTPS Matters

### System HTTP Proxy
- Android system proxy settings configure the OS-level HTTP proxy
- Works for unencrypted HTTP traffic automatically
- For HTTPS, the system proxy setting is used but app must validate the MITM certificate
- This works if certificate is properly installed

### Native Connection Hooks
- Intercepts ALL TCP connections at the libc level
- Doesn't care if it's HTTP or HTTPS
- Doesn't care if it's port 80, 443, or any other port
- More comprehensive than system proxy
- But requires redirecting the socket to the proxy's address

### The Combination
- System proxy + certificate injection = works for HTTP and HTTPS (mostly)
- Native hooks + TLS bypass = works for ALL apps including those that ignore proxy settings
- Together they provide comprehensive interception

## Frida Native Hook Address Rewriting

### The Hook Code (Simplified)

In `native-connect-hook.js` line 127:

```javascript
if (isIPv6) {
    addrPtr.add(8).writeByteArray(PROXY_HOST_IPv6_BYTES);
} else {
    addrPtr.add(4).writeByteArray(PROXY_HOST_IPv4_BYTES);
}
```

This:
1. Takes the address structure passed to `connect()`
2. Finds the IP address part (offset by family and port)
3. Overwrites it with the proxy IP address
4. Returns control, and the `connect()` call proceeds to proxy instead of original

### Why Offset by 4 for IPv4?

The `sockaddr_in` structure for IPv4:
```c
struct sockaddr_in {
    sa_family_t sin_family;  // 2 bytes - address family (AF_INET)
    uint16_t sin_port;       // 2 bytes - port number
    struct in_addr sin_addr; // 4 bytes - IP address
    char sin_zero[8];        // 8 bytes - unused padding
};
```

So to write the IP address:
- Skip 2 bytes (family)
- Skip 2 bytes (port) <- Actually offset is 4 total
- Write 4 bytes (the IP address)

### Why Offset by 8 for IPv6?

The `sockaddr_in6` structure for IPv6:
```c
struct sockaddr_in6 {
    sa_family_t sin6_family;    // 2 bytes
    uint16_t sin6_port;         // 2 bytes
    uint32_t sin6_flowinfo;     // 4 bytes
    struct in6_addr sin6_addr;  // 16 bytes (IPv6 address)
    uint32_t sin6_scope_id;     // 4 bytes
};
```

So to write the IP address:
- Skip 2 bytes (family)
- Skip 2 bytes (port)
- Skip 4 bytes (flowinfo)
- Total offset: 8 bytes
- Write 16 bytes (the IPv6 address)

## IPv4-Mapped IPv6 Addresses

When Frida sees IPv6 connections, they might be IPv4-mapped:

```javascript
const IPv6_MAPPING_PREFIX_BYTES = [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xff, 0xff];
const PROXY_HOST_IPv6_BYTES = IPv6_MAPPING_PREFIX_BYTES.concat(PROXY_HOST_IPv4_BYTES);
```

This creates an IPv6 address like: `::ffff:127.0.0.1`

When printed, this becomes: `::ffff:127.0.0.1` (as seen in the error log)

## The Bottom Line

The fundamental networking issue was simple:
- **Emulator apps** needed to reach **mitmproxy on host**
- **Wrong address**: `127.0.0.1` (emulator itself)
- **Correct address**: `10.0.2.2` (host/container gateway)
- **Fix**: Change the default from `127.0.0.1` to `10.0.2.2`

This is a networking fundamentals issue, not a Frida issue. The Frida hooks were working perfectly - they just had the wrong destination address.
