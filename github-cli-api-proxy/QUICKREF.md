# GitHub CLI Proxy - Quick Reference

## TL;DR

**GitHub CLI already supports proxying! Just set environment variables.**

```bash
export HTTPS_PROXY=http://your-proxy:8080
gh repo view cli/cli
```

## Three Methods

### 1. HTTP/HTTPS Proxy ⭐ Recommended

```bash
# Set proxy
export HTTPS_PROXY=http://proxy.example.com:8080
export HTTP_PROXY=http://proxy.example.com:8080

# Exclude hosts
export NO_PROXY=localhost,127.0.0.1

# Use gh normally
gh repo view cli/cli
```

**Pros:** Works out of the box, no configuration needed
**Cons:** Requires a standard HTTP proxy server

### 2. Unix Domain Socket

```bash
# Start proxy on Unix socket
go run unix-socket-proxy.go /tmp/gh-proxy.sock

# Configure gh
gh config set http_unix_socket /tmp/gh-proxy.sock

# Use gh
gh repo view cli/cli

# Reset
gh config set http_unix_socket ""
```

**Pros:** Maximum flexibility, custom logic
**Cons:** Requires custom proxy implementation

### 3. GH_HOST (NOT a proxy!)

```bash
# Connect to GitHub Enterprise Server
export GH_HOST=github.mycompany.com
gh auth login --hostname github.mycompany.com
```

**Note:** This changes the target host, not a proxy method!

## Testing with Example Proxies

### HTTP Proxy
```bash
# Terminal 1
go run simple-http-proxy.go 8888

# Terminal 2
export HTTPS_PROXY=http://localhost:8888
gh repo view cli/cli
```

### Unix Socket Proxy
```bash
# Terminal 1
go run unix-socket-proxy.go /tmp/gh-proxy.sock

# Terminal 2
gh config set http_unix_socket /tmp/gh-proxy.sock
gh repo view cli/cli
```

## Common Use Cases

| Use Case | Method | Command |
|----------|--------|---------|
| Corporate proxy | HTTP Proxy | `export HTTPS_PROXY=http://proxy:8080` |
| Debug traffic | Either | Use example proxies |
| Custom rate limiting | Unix Socket | Custom proxy + `gh config set` |
| GitHub Enterprise | GH_HOST | `export GH_HOST=ghe.company.com` |
| Combine GHE + Proxy | Both | Set both `GH_HOST` and `HTTPS_PROXY` |

## Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `HTTPS_PROXY` | Proxy for HTTPS (most GitHub API calls) | `http://proxy:8080` |
| `HTTP_PROXY` | Proxy for HTTP | `http://proxy:8080` |
| `NO_PROXY` | Hosts to bypass proxy | `localhost,127.0.0.1` |
| `GH_HOST` | Target GitHub instance | `github.enterprise.com` |
| `GH_DEBUG` | Enable debug logging | `api` or `1` |

## Configuration File

Unix socket setting is stored in `~/.config/gh/config.yml`:

```yaml
http_unix_socket: /tmp/gh-proxy.sock
```

Or set via:
```bash
gh config set http_unix_socket /path/to/socket
```

## Verification

Check if proxy is being used:

```bash
# Method 1: Set debug logging
export GH_DEBUG=api
gh repo view cli/cli
# Look for proxy-related messages

# Method 2: Check proxy logs
# The example proxies log all requests

# Method 3: Use network tools
# tcpdump, wireshark, etc.
```

## Reset Configuration

```bash
# Clear Unix socket
gh config set http_unix_socket ""

# Clear environment variables
unset HTTPS_PROXY
unset HTTP_PROXY
unset NO_PROXY
unset GH_HOST
```

## Source Code References

- HTTP client: `go-gh/pkg/api/http_client.go:59` (uses `http.DefaultTransport`)
- Config: `cli/internal/config/config.go:28` (`http_unix_socket`)
- Env vars: `cli/pkg/cmd/root/help_topic.go:52` (`GH_HOST`)
