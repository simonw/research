Proxying GitHub CLI (`gh`) API traffic can be achieved through standard HTTP/HTTPS proxies or via a Unix domain socket, each suited to different use cases and levels of flexibility. The CLI, implemented in Go, natively supports proxy environment variables (`HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY`), making integration with existing HTTP proxies seamless and requiring no changes to the CLI configuration. For advanced needs like local debugging or custom proxy logic, routing traffic through a Unix domain socket is supported via a configuration option and allows for fine-grained control over requests. Changing the target host (using `GH_HOST`) is not a proxy method but useful for connecting to GitHub Enterprise Server.

Key tools and references:
- [GitHub CLI source code](https://github.com/cli/cli)
- [go-gh library](https://github.com/cli/go-gh)

**Key Findings:**
- Standard proxy integration is easiest (environment variable configuration).
- Unix socket proxying offers advanced flexibility for development, debugging, and custom logic.
- Changing `GH_HOST` allows targeting GitHub Enterprise Server, but does not act as a proxy.
- Example proxy servers (HTTP/HTTPS proxy and Unix socket proxy) are provided for inspection and debugging scenarios.
