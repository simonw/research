# GitHub CLI API Proxy Investigation

This investigation explores methods to proxy GitHub CLI (`gh`) traffic through a different API host or proxy server.

## Key Findings

The GitHub CLI supports multiple methods for proxying traffic:

### 1. Standard HTTP/HTTPS Proxy (Recommended)

**This is the easiest method and works out of the box!**

The GitHub CLI is built with Go and uses `http.DefaultTransport`, which automatically respects standard proxy environment variables.

#### Usage

```bash
# Set proxy for HTTPS requests (most GitHub API calls)
export HTTPS_PROXY=http://proxy.example.com:8080

# Set proxy for HTTP requests (if needed)
export HTTP_PROXY=http://proxy.example.com:8080

# Exclude certain hosts from proxying
export NO_PROXY=localhost,127.0.0.1,.internal.company.com

# Now use gh normally - all traffic goes through the proxy
gh repo view cli/cli
gh pr list
```

#### How It Works

- Located in: `go-gh/pkg/api/http_client.go:59`
- Uses Go's `http.DefaultTransport` as the base transport
- Go's standard library automatically reads `HTTP_PROXY`, `HTTPS_PROXY`, and `NO_PROXY` environment variables
- No configuration changes to `gh` needed
- Works with any standard HTTP proxy server (Squid, mitmproxy, corporate proxies, etc.)

### 2. Unix Domain Socket Proxy

**For more advanced use cases, custom proxies, or debugging.**

GitHub CLI can route all HTTP traffic through a Unix domain socket, which can be connected to a custom proxy server.

#### Usage

```bash
# Start a proxy server listening on a Unix socket
go run unix-socket-proxy.go /tmp/gh-proxy.sock

# Configure gh to use the Unix socket
gh config set http_unix_socket /tmp/gh-proxy.sock

# Now all gh traffic goes through the Unix socket
gh repo view cli/cli

# To reset
gh config set http_unix_socket ""
```

#### How It Works

- Configuration key: `http_unix_socket` (in `~/.config/gh/config.yml`)
- Located in: `go-gh/pkg/api/http_client.go:61-63`
- Creates a custom transport that dials the Unix socket for ALL connections
- Bypasses standard HTTP proxy environment variables
- Useful for:
  - Local debugging and traffic inspection
  - Custom proxy logic
  - Sandboxing/isolation
  - Integration with tools like `socat`, `nginx`

### 3. GH_HOST Environment Variable

**Note: This is NOT a proxy method!**

This changes which GitHub instance you're connecting to, but doesn't intercept traffic.

#### Usage

```bash
# Connect to GitHub Enterprise Server instead of github.com
export GH_HOST=github.enterprise.company.com

gh repo view myorg/myrepo
```

#### How It Works

- Located in: `pkg/cmd/root/help_topic.go:52`
- Changes the target hostname for API requests
- Useful for GitHub Enterprise Server deployments
- Does not intercept or proxy traffic - just redirects to a different host

## Example Proxy Servers

This investigation includes two example proxy servers written in Go:

### 1. simple-http-proxy.go

A standard HTTP/HTTPS proxy server that logs all requests.

```bash
# Build and run
go run simple-http-proxy.go 8888

# In another terminal
export HTTPS_PROXY=http://localhost:8888
gh repo view cli/cli
```

Features:
- Handles both HTTP and HTTPS (CONNECT tunneling)
- Logs all requests for debugging
- Works with the `HTTPS_PROXY` environment variable

### 2. unix-socket-proxy.go

A Unix domain socket proxy server that logs all requests.

```bash
# Build and run
go run unix-socket-proxy.go /tmp/gh-proxy.sock

# In another terminal
gh config set http_unix_socket /tmp/gh-proxy.sock
gh repo view cli/cli
```

Features:
- Listens on a Unix domain socket
- Handles both HTTP and HTTPS
- Logs all requests for debugging
- Works with `gh config set http_unix_socket`

## Comparison of Methods

| Method | Ease of Use | Flexibility | Configuration Required |
|--------|-------------|-------------|------------------------|
| HTTP/HTTPS Proxy | ⭐⭐⭐⭐⭐ Easiest | ⭐⭐⭐ Good | Environment variable only |
| Unix Socket | ⭐⭐⭐ Moderate | ⭐⭐⭐⭐⭐ Highest | gh config + proxy server |
| GH_HOST | ⭐⭐⭐⭐⭐ Easiest | ⭐ Limited | Environment variable only |

## Use Cases

### Corporate Proxy
Use **HTTP/HTTPS Proxy** method:
```bash
export HTTPS_PROXY=http://corporate-proxy.company.com:3128
gh auth login
```

### Traffic Inspection/Debugging
Use **Unix Socket** or **HTTP/HTTPS Proxy** with a logging proxy:
```bash
# Option 1: Unix socket
go run unix-socket-proxy.go /tmp/gh-proxy.sock &
gh config set http_unix_socket /tmp/gh-proxy.sock

# Option 2: HTTP proxy
go run simple-http-proxy.go 8888 &
export HTTPS_PROXY=http://localhost:8888
```

### API Rate Limiting Proxy
Use **Unix Socket** with custom logic:
```bash
# Build a custom proxy with rate limiting logic
# Listen on Unix socket
# Configure gh to use it
gh config set http_unix_socket /tmp/rate-limited-proxy.sock
```

### GitHub Enterprise Server
Use **GH_HOST**:
```bash
export GH_HOST=github.mycompany.com
gh auth login --hostname github.mycompany.com
```

## Source Code References

- GitHub CLI: https://github.com/cli/cli
- go-gh library: https://github.com/cli/go-gh
- HTTP client implementation: `go-gh/pkg/api/http_client.go`
- Config implementation: `cli/internal/config/config.go`
- Environment variables documentation: `cli/pkg/cmd/root/help_topic.go`

## Technical Details

### HTTP Client Creation Flow

1. `api/http_client.go` (in cli/cli) calls `ghAPI.NewHTTPClient()`
2. `pkg/api/http_client.go` (in go-gh) creates the client
3. Base transport is `http.DefaultTransport` (line 59)
4. If `UnixDomainSocket` is set, creates custom Unix socket transport (line 61-63)
5. Wraps transport with:
   - Sanitizer (ASCII sanitization)
   - Cache (if enabled)
   - Logger (if GH_DEBUG is set)
   - Header injector (auth tokens, user-agent, etc.)

### Proxy Environment Variables

Go's `http.DefaultTransport` uses `http.ProxyFromEnvironment` which checks:
- `HTTP_PROXY` / `http_proxy` - proxy for HTTP URLs
- `HTTPS_PROXY` / `https_proxy` - proxy for HTTPS URLs
- `NO_PROXY` / `no_proxy` - comma-separated list of hosts to bypass
- `CGI_PROXY` - used in CGI environments

Uppercase variables take precedence over lowercase.

## Recommendations

1. **For most use cases**: Use the standard `HTTPS_PROXY` environment variable
2. **For custom logic**: Build a Unix socket proxy with your specific requirements
3. **For debugging**: Use either method with a logging proxy server
4. **For GitHub Enterprise**: Use `GH_HOST` to change the target, optionally with a proxy

## Conclusion

The GitHub CLI has excellent proxy support built-in through Go's standard HTTP library. For most use cases, simply setting the `HTTPS_PROXY` environment variable is sufficient. For advanced scenarios, the Unix socket option provides maximum flexibility for custom proxy implementations.
