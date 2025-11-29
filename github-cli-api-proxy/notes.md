# GitHub CLI API Proxy Investigation

## Objective
Investigate ways to make the GitHub CLI (gh) use a different API host for proxying traffic.

## Investigation Notes

### Setup
- Cloning github.com/cli/cli repository to /tmp for analysis
- Date: 2025-11-23
- GitHub CLI is written in Go
- Uses github.com/cli/go-gh/v2 v2.13.0 as the main HTTP client library

### Initial Findings

#### 1. GH_HOST Environment Variable
- Located in: pkg/cmd/root/help_topic.go:52
- Purpose: Specify the GitHub hostname for commands where a hostname has not been provided
- This changes which GitHub instance to connect to (e.g., GitHub Enterprise Server)
- Does NOT proxy traffic - it changes the target host entirely

#### 2. http_unix_socket Configuration Option
- Located in: internal/config/config.go:28, 634
- Configuration key: `http_unix_socket`
- Description: "the path to a Unix socket through which to make an HTTP connection"
- Default value: "" (empty, meaning use standard HTTP)
- This allows routing ALL HTTP traffic through a Unix socket
- Useful for proxying through local tools like socat, nginx, or custom proxies

#### 3. HTTP Client Implementation
- Main HTTP client code: api/http_client.go
- Uses github.com/cli/go-gh/v2/pkg/api for HTTP client creation
- Authentication tokens added via custom RoundTripper (AddAuthTokenHeader)

#### 4. Standard HTTP Proxy Support (CRITICAL FINDING!)
- Located in: go-gh/pkg/api/http_client.go:59
- Uses `http.DefaultTransport` as the base transport
- Go's `http.DefaultTransport` automatically respects these environment variables:
  - HTTP_PROXY or http_proxy: proxy URL for HTTP requests
  - HTTPS_PROXY or https_proxy: proxy URL for HTTPS requests
  - NO_PROXY or no_proxy: comma-separated list of hosts to exclude from proxying
- This means gh ALREADY supports HTTP proxies out of the box!
- No code changes needed - just set environment variables

#### 5. Custom Transport Override
- The UnixDomainSocket option (line 61-63) creates a custom transport
- Custom transport bypasses DefaultTransport, so proxy env vars won't work with Unix socket
- Unix socket transport on line 203-213: uses net.Dial("unix", socketPath)
- All HTTP traffic goes through the Unix socket when configured

### Summary of Proxy Methods

Three main approaches identified:

1. **Standard HTTP/HTTPS Proxy (Easiest)**
   - Set environment variables: HTTPS_PROXY, HTTP_PROXY
   - Works with any standard HTTP proxy server
   - No gh configuration changes needed
   - Transparent to gh - uses Go's built-in proxy support

2. **Unix Domain Socket (Most Flexible)**
   - Configure via: `gh config set http_unix_socket /path/to/socket`
   - Requires a proxy server listening on a Unix socket
   - Useful for local proxies, debugging, or sandboxing
   - Can be combined with tools like socat, nginx, or custom proxies

3. **GH_HOST (Different Use Case)**
   - Set via: `export GH_HOST=myghes.company.com`
   - Changes the target GitHub instance (not a proxy!)
   - Useful for GitHub Enterprise Server
   - Does not intercept traffic, just redirects to different host

### Code Examples Created

1. `simple-http-proxy.go` - Standard HTTP/HTTPS proxy server
2. `unix-socket-proxy.go` - Unix domain socket proxy server

Both examples log all requests for debugging/monitoring purposes.

### Files Investigated

Key files examined from github.com/cli/cli:
- `api/http_client.go` - Main HTTP client setup (uses go-gh)
- `internal/config/config.go` - Configuration management
- `pkg/cmd/root/help_topic.go` - Environment variables documentation
- `pkg/cmd/factory/remote_resolver.go` - GH_HOST usage

Key files examined from github.com/cli/go-gh:
- `pkg/api/http_client.go` - HTTP client implementation (THE KEY FILE!)
  - Line 59: Uses `http.DefaultTransport` (supports proxy env vars!)
  - Line 61-63: UnixDomainSocket override
  - Line 203-213: Unix socket transport implementation

### Testing Notes

Did not perform live testing as:
1. This is a read-only investigation of the codebase
2. The proxy support is built-in to Go's standard library
3. The code clearly shows the implementation

However, provided complete working example proxy servers for testing.

### Conclusion

GitHub CLI has excellent built-in proxy support:
- **Best method**: Set HTTPS_PROXY environment variable (works immediately)
- **Advanced method**: Use http_unix_socket config for custom proxies
- **Note**: GH_HOST changes target host, not a proxy mechanism

No code modifications to gh are needed for proxying - it's already supported!

