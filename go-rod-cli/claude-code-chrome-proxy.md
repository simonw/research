# Chrome Proxy Authentication in Claude Code Environments

## The Problem

In sandboxed container environments (like Claude Code), outbound HTTP/HTTPS traffic
is routed through an authenticated proxy. The proxy URL is set in environment variables:

```
HTTPS_PROXY=http://container_xxx:jwt_TOKEN@21.0.0.201:15004
HTTP_PROXY=http://container_xxx:jwt_TOKEN@21.0.0.201:15004
```

The proxy uses Basic auth with:
- **Username**: A container identifier (e.g., `container_container_01EYyYD9iomXPtNq4T4Rz4fm--claude_code_remote--094fab`)
- **Password**: A JWT token (`jwt_eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiIs...`)

**Tools like `curl`, `wget`, and Go's `http.Client`** read these env vars automatically
and send `Proxy-Authorization: Basic <base64(user:pass)>` headers. They work out of the box.

**Headless Chrome does NOT.** Chrome has `--proxy-server` to set the proxy address,
but no flag for proxy credentials. When Chrome sends a CONNECT request to the proxy
without auth, the proxy rejects it, and Chrome reports:

```
net::ERR_TUNNEL_CONNECTION_FAILED
```

## Approaches Tried

### 1. Chrome DevTools Protocol `Fetch` domain (FAILED)

Rod's `browser.MustHandleAuth(user, pass)` uses the CDP `Fetch` domain to intercept
`407 Proxy Authentication Required` responses and respond with credentials.

**Why it failed**: The CDP Fetch domain operates at the HTTP layer, but for HTTPS
URLs, the proxy uses the CONNECT method to establish a TCP tunnel. The auth challenge
happens during the CONNECT handshake, which is below the level where CDP can intercept.
Chrome's network stack handles CONNECT directly - the tunnel must succeed before any
CDP-observable HTTP traffic flows.

The MustHandleAuth call hung indefinitely because Chrome never received an
`AuthRequired` event - the connection failed at the tunnel level before reaching
the auth challenge stage.

### 2. `Network.setExtraHTTPHeaders` (FAILED)

Tried setting `Proxy-Authorization` header via CDP's `Network.setExtraHTTPHeaders`.
Same issue - these headers apply to HTTP requests sent through an already-established
connection, not to the CONNECT tunnel establishment itself.

### 3. Embedding credentials in `--proxy-server` URL (FAILED)

Chrome's `--proxy-server` flag only accepts `host:port` or `http://host:port`.
It does not support `http://user:pass@host:port` - the credentials are silently ignored.

### 4. Local forwarding proxy with auth injection (SUCCESS)

**The solution that worked**: Run a local HTTP proxy that:
1. Listens on localhost (no auth required from Chrome)
2. Receives CONNECT requests from Chrome
3. Forwards them to the upstream proxy WITH the `Proxy-Authorization` header
4. Pipes the tunnel bidirectionally once established

```
Chrome --proxy-server=http://127.0.0.1:LOCAL_PORT
         |
         | CONNECT www.example.com:443 (no auth)
         v
Local Auth Proxy (127.0.0.1:LOCAL_PORT)
         |
         | CONNECT www.example.com:443
         | Proxy-Authorization: Basic <base64(user:pass)>
         v
Upstream Proxy (21.0.0.201:15004)
         |
         | TCP tunnel to www.example.com:443
         v
Target Server
```

## Implementation Details

### The auth proxy helper

The `rod-cli` binary doubles as a proxy when invoked with the hidden `_proxy` subcommand:

```bash
rod-cli _proxy <port> <upstream-host:port> <auth-header>
```

This is launched as a background process by `rod-cli start`. It runs in its own
session (`Setsid: true`) so it survives after the parent exits. Its PID is stored
in `~/.rod-cli/state.json` and killed by `rod-cli stop`.

### CONNECT handling (the critical part)

```go
func proxyConnect(w http.ResponseWriter, r *http.Request, upstream, authHeader string) {
    // 1. Dial the upstream proxy
    upstreamConn, _ := net.DialTimeout("tcp", upstream, 30*time.Second)

    // 2. Send CONNECT with auth to upstream
    connectReq := fmt.Sprintf(
        "CONNECT %s HTTP/1.1\r\nHost: %s\r\nProxy-Authorization: %s\r\n\r\n",
        r.Host, r.Host, authHeader)
    upstreamConn.Write([]byte(connectReq))

    // 3. Read upstream response (should be "HTTP/1.1 200 OK")
    buf := make([]byte, 4096)
    n, _ := upstreamConn.Read(buf)
    // verify response[9:12] == "200"

    // 4. Hijack the client connection
    clientConn, _, _ := w.(http.Hijacker).Hijack()

    // 5. Tell Chrome the tunnel is up
    clientConn.Write([]byte("HTTP/1.1 200 Connection Established\r\n\r\n"))

    // 6. Bidirectional pipe
    go io.Copy(upstreamConn, clientConn)
    go io.Copy(clientConn, upstreamConn)
}
```

### Certificate errors

Once the proxy tunnel is working, there's a secondary issue: `ERR_CERT_AUTHORITY_INVALID`.
This happens because the proxy infrastructure may perform TLS inspection, or the
container lacks certain root CAs. The fix is to launch Chrome with
`--ignore-certificate-errors` when using the proxy.

### Proxy detection

The `start` command automatically detects authenticated proxies:

```go
func detectProxy() (server, user, pass string, needed bool) {
    // Check HTTPS_PROXY, https_proxy, HTTP_PROXY, http_proxy
    // Parse URL, extract user:pass from URL
    // Return true only if credentials are present
}
```

If no authenticated proxy is detected, Chrome launches normally without the helper.

### Full startup sequence with proxy

```
rod-cli start
  1. detectProxy() -> finds HTTPS_PROXY with auth
  2. Find free port for local proxy
  3. Launch: rod-cli _proxy <port> <upstream> <authHeader> (background, detached)
  4. Store proxy PID in state
  5. Launch Chrome with --proxy-server=http://127.0.0.1:<port>
                        --ignore-certificate-errors
  6. Store Chrome PID and debug URL in state

rod-cli open https://www.example.com/
  1. Load state, connect to Chrome via WebSocket
  2. Navigate page (Chrome -> local proxy -> upstream proxy -> target)
  3. Return page title

rod-cli stop
  1. Close Chrome via CDP
  2. Kill proxy helper by PID
  3. Remove state file
```

## Key Observations

### The upstream proxy (envoy-based)

The proxy responds with envoy headers, suggesting it's an Envoy-based egress gateway:

```
HTTP/1.1 200 OK
date: Mon, 09 Feb 2026 19:54:55 GMT
server: envoy
```

Chrome also makes background requests to Google services through the proxy:
- `safebrowsingohttpgateway.googleapis.com:443` (Safe Browsing)
- `www.google.com:443`
- `accounts.google.com:443`

All are tunneled successfully through our auth proxy.

### JWT token expiration

The JWT in the proxy credentials has `exp` (expiration) claims. For long-running
Chrome sessions, the token may expire. Current behavior: connections fail silently.
A future improvement could monitor the proxy env vars for token rotation and restart
the auth proxy helper.

### NO_PROXY is respected

The environment includes:
```
NO_PROXY=localhost,127.0.0.1,169.254.169.254,metadata.google.internal,...
```

Chrome respects `NO_PROXY` for the `--proxy-server` flag, so local URLs bypass the
proxy automatically. This is why `rod-cli open http://127.0.0.1:18080/` always worked
even before adding proxy support.

## Testing

Verified with:
```bash
rod-cli start                                    # Proxy auto-detected and started
rod-cli open https://www.example.com/            # Page loaded successfully
rod-cli title                                    # "Example Domain"
rod-cli text h1                                  # "Example Domain"
rod-cli js 'document.querySelector("p").textContent'  # Full paragraph text
rod-cli screenshot /tmp/example.png              # 19KB PNG of the page
rod-cli stop                                     # Chrome and proxy cleaned up
```

## Summary

Getting Chrome to work through an authenticated proxy requires a local forwarding
proxy because Chrome cannot send credentials during CONNECT tunnel establishment.
The CDP-level auth interception (Fetch domain) doesn't help because it operates above
the tunnel layer. The local proxy pattern is the standard solution used by tools like
`cntlm` and `px-proxy` in corporate environments.
