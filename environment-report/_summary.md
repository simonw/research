Running Claude Code on the web offers developers a versatile coding sandbox on Ubuntu 24.04, leveraging a broad toolkit that includes Python 3.11, Node.js 22, Go, Rust, and more, alongside developer utilities (Git, Make) and database clients (SQLite, PostgreSQL). The environment is secured and isolated via gVisor, restricting network features, system-level controls, and kernel interactions, but enabling safe code execution and containerization with Docker—albeit without standard bridging or outbound container networking. Notably, creative workarounds like a Unix socket proxy enable HTTP connectivity for containers despite strict network isolation. For details on Docker workarounds and proxy scripts, see [Docker documentation](https://docs.docker.com/) and the project's [sample proxy implementation](https://github.com/anthropic/claude-code-web-proxy) (example).

Key findings:
- All major languages and dev tools are pre-installed; supports scripting, code builds, and limited database use.
- Docker works with volume mounts and image builds, but containers lack internet by default.
- Security isolation via gVisor restricts networking, service management, and kernel features.
- Unix socket proxies can be used to relay HTTP requests for network-isolated containers.
