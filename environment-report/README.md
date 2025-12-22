# Claude Code for Web Environment

A comprehensive analysis of the coding environment available when running Claude Code on the web.

## Environment Summary

| Resource | Value |
|----------|-------|
| **Operating System** | Ubuntu 24.04.3 LTS (Noble Numbat) |
| **Kernel** | 4.4.0 (gVisor sandboxed) |
| **Architecture** | x86_64 |
| **CPU Cores** | 16 |
| **Memory** | 21 GB |
| **Storage** | 30 GB |
| **User** | root |
| **Shell** | Bash 5.2.21 |

## Programming Languages & Runtimes

| Language | Version | Notes |
|----------|---------|-------|
| **Python** | 3.11.14 | pip 24.0, uv 0.8.17 |
| **Node.js** | 22.21.1 | npm 10.9.4 |
| **Bun** | 1.3.4 | JavaScript runtime |
| **Go** | 1.24.7 | |
| **Rust** | 1.91.1 | Cargo 1.91.1 |
| **Java** | OpenJDK 21.0.9 | |
| **Ruby** | 3.3.6 | |
| **PHP** | 8.4.15 | |
| **GCC** | 13.3.0 | C/C++ compiler |

## Developer Tools

| Tool | Version |
|------|---------|
| **Git** | 2.43.0 |
| **Make** | 4.3 |
| **curl** | 8.5.0 |
| **wget** | 1.21.4 |

## Databases

| Database | Status | Version |
|----------|--------|---------|
| **PostgreSQL** | Client only | 16.11 |
| **Redis** | Available | 7.0.15 |
| **SQLite** | Via Python | 3.45.1 |
| **MySQL** | Not installed | - |
| **MongoDB** | Not installed | - |

## Docker

Docker can be made to work in this environment with some limitations.

### Installation & Setup

Docker is not pre-installed but can be installed:

```bash
apt-get update && apt-get install -y docker.io
```

### Starting Docker Daemon

The standard Docker startup fails due to iptables restrictions. Use this workaround:

```bash
dockerd --iptables=false --ip-forward=false --ip-masq=false --bridge=none &
```

### What Works

- Pulling images from Docker Hub
- Running containers (with `--network=none`)
- Building images with `docker build`
- Volume mounts (`-v /host/path:/container/path`)
- Privileged mode
- Container image management

### What Does NOT Work (By Default)

- **Container networking** - No bridge network, no internet from containers
- **Port publishing** (`-p 8080:80`) - Requires bridge networking
- **Docker Compose** - Not installed by default
- **Overlay filesystem** - Falls back to `vfs` storage driver

### Workaround: Unix Socket Proxy for Container Networking

While containers can't access the network directly, you can work around this using a **Unix socket proxy**. The host runs a proxy server that listens on a Unix socket, which is mounted into the container. The container sends HTTP requests through the socket, and the host proxy fetches from the real network.

```
┌─────────────────┐     Unix Socket      ┌─────────────────┐
│   Container     │ ──────────────────── │   Host Proxy    │ ──── Internet
│ (no networking) │   /tmp/proxy.sock    │ (has network)   │
└─────────────────┘                      └─────────────────┘
```

**Step 1: Start the proxy on the host**

```bash
python3 http_proxy.py &
```

**Step 2: Run container with socket mounted**

```bash
docker run --network=none \
  -v /tmp/http_proxy.sock:/tmp/http_proxy.sock \
  -v $(pwd)/proxy_client.py:/proxy_client.py \
  python:3.11-alpine python3 -c "
from proxy_client import fetch
result = fetch('https://httpbin.org/get')
print(result['status'], result['body'][:100])
"
```

See `http_proxy.py` and `proxy_client.py` in this directory for the implementation.

### Example Usage

```bash
# Simple container
docker run --network=none alpine cat /etc/os-release

# Python execution
docker run --network=none python:3.11-alpine python -c "print('Hello')"

# With volume mount
docker run --network=none -v /tmp/data:/data alpine ls /data

# Build an image
docker build -t myimage .
docker run --network=none myimage
```

## Environment Characteristics

This environment runs inside a **gVisor sandbox** (indicated by `runsc` hostname), which provides:

### Security Benefits
- Strong process isolation
- Limited kernel attack surface
- Restricted system calls

### Limitations
- No `systemd` (services must be started manually)
- No `iptables`/`nftables` support
- No kernel module loading
- File system is virtualized (9p)
- IPv4 forwarding disabled
- Limited cgroup features

## Use Cases

### Ideal For
- Building and testing code
- Running scripts and CLI tools
- Processing data with various languages
- Building Docker images (for offline/unit testing)
- Database operations with SQLite/PostgreSQL client

### Limitations
- No persistent networking services
- Cannot expose ports to external access
- Docker containers are network-isolated

## Quick Reference

```bash
# Check available memory
free -h

# Check storage
df -h

# Check CPU count
nproc

# Start Docker (if needed)
dockerd --iptables=false --bridge=none &

# Run a container
docker run --network=none alpine echo "Hello"
```

---

*Report generated: 2025-12-22*
