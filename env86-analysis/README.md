# env86 Analysis Report

## What is env86?

env86 is a Go-based management tool for running x86 Linux virtual machines using the v86 JavaScript emulator. It provides a CLI and desktop application for creating, distributing, and running reproducible Linux environments that execute in a browser context via WebAssembly.

In essence, env86 wraps the [v86 x86 emulator](https://github.com/copy/v86) with:
- A native desktop application that embeds a browser (WebView/Chrome)
- A comprehensive CLI for VM lifecycle management
- A virtual networking stack with WebSocket relay
- Host-guest communication via serial ports and 9P filesystem protocol
- Snapshot-based state management for instant boot
- Image distribution system via GitHub releases

## Core Architecture

### Three-Layer System

1. **Host Layer (env86)**: Go binary that manages VMs, networking, and desktop windows
2. **Emulation Layer (v86)**: JavaScript/WASM x86 emulator running in browser context
3. **Guest Layer (guest86)**: Optional i386 Linux service running inside the VM

### Communication Channels

```
┌─────────────────────────────────────────────────────────────┐
│ Host System                                                  │
│                                                              │
│  env86 Binary ──[HTTP Server]──► Desktop Window             │
│       │                                │                     │
│       │                                ▼                     │
│       │                    ┌──────────────────────┐          │
│       │                    │ v86 Emulator (WASM)  │          │
│       │                    │                      │          │
│       │  WebSocket /ctl ──►│  ┌────────────────┐ │          │
│       │  (VM control)      │  │ Linux i386     │ │          │
│       │                    │  │                │ │          │
│       │  WebSocket /guest ─┼─►│  guest86 ◄───┐ │ │          │
│       │  (command exec)    │  │  (ttyS1)     │ │ │          │
│       │                    │  └──────────────┘ │ │          │
│       │  WebSocket /net ───┼─►  Virtual NIC    │ │          │
│       │  (networking)      │                   │ │          │
│       │                    └───────────────────┘ │          │
│       │                                           │          │
│  Virtual Network Stack                            │          │
│  (192.168.127.0/24)                               │          │
└───────────────────────────────────────────────────┘          │
```

### Key Technologies

- **v86**: x86 PC emulator compiled to WebAssembly
- **go-netstack**: User-space TCP/IP stack (no root required)
- **9P Protocol**: Plan 9 filesystem protocol for directory mounting
- **CBOR**: Binary serialization for RPC over serial ports
- **Desktop Framework**: Native window management (WebView2/WebKit)
- **ChromeDP**: Headless Chrome automation for CLI mode

## How It Works

### Image Format

Images are distributed as tarballs or directories containing:

```
image-name.tgz (or directory)
├── image.json           # v86 configuration
├── fs.json              # Filesystem metadata
├── fs/                  # Filesystem files (lazy-loaded)
│   ├── bin/
│   ├── etc/
│   └── ...
└── initial.state[.zst]  # VM state snapshot (optional)
```

The `image.json` specifies:
- Memory size (default 512MB)
- Kernel/initramfs URLs (for cold boot)
- Initial state URL (for fast boot)
- Guest service availability
- Filesystem configuration

### Boot Sequence

#### Fast Boot (with initial.state)
1. Load image from local path or `~/.env86/`
2. Start HTTP server on random port
3. Launch desktop window with embedded browser
4. Load v86.wasm
5. **Restore VM state from snapshot** ← Boots in seconds
6. Establish WebSocket connections
7. Ready for interaction

#### Cold Boot (without initial.state)
1. Same steps 1-4
2. Load BIOS (seabios.bin)
3. Load VGA BIOS (vgabios.bin)
4. Load Linux kernel (vmlinuz.bin)
5. Load initramfs (initramfs.bin)
6. Boot Linux ← Takes 30-60 seconds
7. Mount 9P filesystem
8. Start guest86 service (if included)
9. Ready for interaction

### State Management

env86 can save and restore complete VM state:

```bash
# Boot with --save flag
env86 boot myimage --save

# On exit (Ctrl-D), VM state is saved to image/initial.state
# Next boot will use snapshot for instant startup
```

This enables:
- Instant boot times (2-3 seconds vs 30+ seconds)
- Pre-configured environments (logged in, services running)
- Reproducible states for testing

### Guest Service (guest86)

The guest86 binary runs inside the VM and provides:

1. **Command Execution**: Run programs with stdin/stdout streaming
2. **Network Reset**: Reconfigure interfaces after boot
3. **9P Mounting**: Mount host directories into VM

Communication uses CBOR-encoded RPC over serial port `/dev/ttyS1` (COM2).

Example internal flow when running `env86 run myimage sh`:
1. env86 boots VM in headless mode
2. Connects to guest86 via WebSocket
3. Sends RPC call: `vm.Run({Name: "sh", PTY: true})`
4. guest86 executes `sh` and streams output
5. Host terminal connected to stdin/stdout
6. On exit, VM is stopped

### Virtual Networking

env86 includes a full virtual network stack:

- **Subnet**: 192.168.127.0/24
- **Gateway**: 192.168.127.1 (host)
- **VM Address**: 192.168.127.2 (typically)
- **Protocol**: Custom WebSocket relay using QEMU format

Network traffic flows:
```
VM process → v86 virtual NIC → WebSocket /net →
host go-netstack → real network
```

Port forwarding maps host ports to VM ports:
```bash
env86 boot myimage --net -p 8080:80
# Host :8080 → VM 192.168.127.2:80
```

### Filesystem Access

#### Read-Only Image Filesystem
The image's `fs/` directory is mounted read-only via v86's lazy-loading filesystem. Files are fetched over HTTP as needed.

#### Read-Write Host Mounts
Host directories can be mounted using 9P protocol:
```bash
env86 run myimage -m .:/mnt/host sh
# Current directory available at /mnt/host in VM
```

This uses:
1. 9P server on host (random port)
2. 9P client in VM (guest86)
3. Proxy over WebSocket channel

## How to Use It

### Installation

```bash
# Build from source
git clone https://github.com/progrium/env86
cd env86
make build
./env86 --help

# Or download from releases
# (Look for env86_VERSION_OS_ARCH.zip)
```

### Pulling Images

```bash
# Pull an image from GitHub
env86 pull github.com/progrium/alpine

# Stored in ~/.env86/github.com/progrium/alpine/latest/

# Pull specific version
env86 pull github.com/progrium/alpine@3.18
```

Images are distributed as GitHub releases with `.tgz` assets.

### Booting VMs

```bash
# Interactive boot (opens desktop window)
env86 boot github.com/progrium/alpine

# With networking enabled
env86 boot github.com/progrium/alpine --net

# Port forwarding
env86 boot myimage --net -p 8080:80

# Cold boot (ignore saved state)
env86 boot myimage --cold

# Save state on exit
env86 boot myimage --save

# Headless mode
env86 boot myimage --cdp --console-url
```

### Running Commands

```bash
# Execute single command (requires guest service)
env86 run myimage sh -c "uname -a"

# Interactive shell
env86 run myimage sh

# With networking
env86 run myimage --net curl https://example.com

# Mount current directory
env86 run myimage -m .:/work sh
```

### Creating Images

Based on code analysis (not fully tested):

```bash
# Create image from Alpine rootfs
env86 create alpine-custom ./alpine-rootfs

# Directory structure created:
# alpine-custom/
#   ├── image.json
#   ├── fs.json
#   └── fs/...
```

### Preparing for Web Distribution

```bash
# Prepare image for static hosting
env86 prepare myimage output/

# output/ can be served via any HTTP server
# Users access via browser without installing env86
```

### Serving Images

```bash
# Serve image via HTTP
env86 serve myimage

# Access via browser at displayed URL
```

## Where It Can Be Useful

### 1. Development Environments

**Use Case**: Reproducible, shareable Linux environments

```bash
# Create environment with specific tools
env86 run myenv --net -m ~/project:/work sh

# Team members run identical environment
# No "works on my machine" issues
```

**Advantages**:
- Fast boot from snapshots
- Mount source code from host
- Network access for package installation
- Cross-platform (Windows/macOS/Linux all run same VM)

### 2. Testing and CI/CD

**Use Case**: Automated testing in isolated environments

```bash
# Headless execution in CI
env86 run test-env --cdp -m .:/src sh -c "cd /src && make test"
```

**Advantages**:
- No Docker/VM driver required
- Runs anywhere (even in browsers)
- Snapshot-based for fast test startup
- Complete isolation

### 3. Education and Training

**Use Case**: Browser-based Linux access

```bash
# Prepare course environment
env86 prepare course-vm output/

# Host on static file server
# Students access via browser, no installation
```

**Advantages**:
- Zero installation for students
- Consistent environment for everyone
- State snapshots for checkpoint/restore
- Works on Chromebooks, tablets, etc.

### 4. Security Sandboxing

**Use Case**: Run untrusted code safely

```bash
# Execute in isolated VM
env86 run sandbox --no-net sh -c "./untrusted-script"
```

**Advantages**:
- JavaScript sandbox (browser security model)
- No host access without explicit mounts
- Optional network isolation
- x86 emulation adds another isolation layer

### 5. Legacy Software

**Use Case**: Run old i386 Linux software

```bash
# Run 32-bit Linux applications
env86 run legacy-env /opt/old-app/bin/program
```

**Advantages**:
- Full i386 Linux environment
- No need for multilib or containers
- Self-contained images
- Cross-platform hosting

### 6. Demos and POCs

**Use Case**: Quick environment demos

```bash
# Create demo with pre-configured state
env86 boot demo --save

# Share demo/ directory
# Others boot to exact same state
```

**Advantages**:
- Instant boot to demo state
- No complex setup instructions
- Portable across platforms
- Can be web-hosted

### 7. Embedded/IoT Development

**Use Case**: Emulate i386 embedded Linux

```bash
# Test embedded system software
env86 run embedded-target --net -p 5000:5000 sh
```

**Advantages**:
- Fast iteration without hardware
- Network simulation
- State snapshots for testing scenarios
- Debug with host tools via mounts

## Comparison with Alternatives

### vs Docker
- **Docker**: Container runtime, shares host kernel
- **env86**: Full x86 emulation, runs anywhere with browser
- **env86 advantages**: Works in browser, no kernel dependency, portable
- **Docker advantages**: Better performance, mature ecosystem

### vs VirtualBox/VMware
- **VirtualBox/VMware**: Full system virtualization
- **env86**: x86 emulation in browser
- **env86 advantages**: No installation, works on all platforms, web-hostable
- **VirtualBox advantages**: Better performance, more features

### vs QEMU
- **QEMU**: System emulator (C/C++)
- **env86**: x86 emulator (JavaScript/WASM) + management
- **env86 advantages**: Browser-based, easier distribution, desktop UI
- **QEMU advantages**: Better performance, more architectures

### vs WSL/Lima
- **WSL/Lima**: Linux on Windows/macOS
- **env86**: Cross-platform x86 emulation
- **env86 advantages**: Consistent everywhere, web-hostable, snapshots
- **WSL/Lima advantages**: Better performance, native feel

## Technical Deep Dive

### Why Desktop App + Browser?

v86 is fundamentally a browser-based emulator (JavaScript/WASM). env86 bridges this to a native experience:

1. **Desktop Window**: Provides native app feel
2. **HTTP Server**: Serves v86 assets and handles WebSocket
3. **WebView**: Embeds browser engine (WebView2 on Windows, WebKit on macOS)
4. **ChromeDP Option**: Headless Chrome for CLI/CI use

This architecture enables both GUI (desktop window) and CLI (headless) modes.

### Serial Port Communication

guest86 uses serial port `/dev/ttyS1` for host communication. Why?

1. **No Network Dependency**: Works before network is configured
2. **Simple Protocol**: CBOR over serial is straightforward
3. **Always Available**: Serial ports are basic x86 hardware
4. **Multiplexed**: Single serial port carries multiple RPC channels

The v86 emulator bridges serial bytes to WebSocket, creating a transparent channel.

### Network Stack Implementation

env86 uses `go-netstack`, a user-space TCP/IP implementation:

1. **No Root Required**: Pure Go, no kernel modules
2. **Portable**: Works on Windows/macOS/Linux
3. **Virtual**: Creates 192.168.127.0/24 subnet
4. **NAT**: VMs can access host network via gateway

Traffic flow: `VM → v86 NIC → WebSocket → go-netstack → host OS network`

### State Snapshots

v86 can serialize entire VM state to binary blob:
- CPU registers
- Memory contents
- Device states

env86 saves this to `initial.state`, enabling:
- Instant boot (restore state instead of booting)
- Pre-configured environments
- Testing checkpoints

Large states are split into 10MB chunks for browser loading.

### Image Distribution

Images are distributed via GitHub releases:

1. Create image repository (e.g., `github.com/user/myimage-env86`)
2. Create releases with tagged versions
3. Attach `.tgz` file as release asset (e.g., `myimage-1.0.tgz`)
4. Users run `env86 pull github.com/user/myimage@1.0`

This provides:
- Version management
- Simple distribution (GitHub CDN)
- No custom registry needed

### Cross-Platform Compatibility

env86 runs on Windows, macOS, and Linux via:

1. **Go Cross-Compilation**: Single codebase, multiple targets
2. **Desktop Framework**: Platform-specific webview (tractor.dev/toolkit-go)
3. **WebAssembly**: v86 runs identically everywhere
4. **Modified syscall**: WASM-compatible `golang.org/x/sys` fork

## Limitations and Considerations

### Performance
- **Emulation Overhead**: v86 is slower than native/virtualization
- **JavaScript/WASM**: Not as fast as native code
- **Use Case**: Suitable for development/testing, not production workloads

### Architecture Support
- **x86 Only**: v86 emulates i386, no ARM/x64 guests
- **32-bit Linux**: Most images are i386 Alpine Linux

### Networking
- **Virtual Only**: go-netstack, not direct hardware access
- **NAT Mode**: VMs behind gateway, not bridged
- **Performance**: User-space network slower than kernel

### Browser Dependency
- **Requires Browser**: Desktop app embeds one, but still needed
- **ChromeDP for CLI**: Headless Chrome adds overhead

### Ecosystem
- **Young Project**: Not as mature as Docker/VirtualBox
- **Smaller Community**: Fewer pre-built images

## Conclusion

env86 is a creative solution for running reproducible x86 Linux environments anywhere, leveraging browser technology for cross-platform compatibility. It excels in scenarios requiring:

- **Portability**: Run identical VMs on any platform
- **Distribution**: Share environments via GitHub or web
- **Quick Start**: Snapshot-based instant boot
- **Isolation**: Sandboxed execution
- **Accessibility**: Browser-based, no installation

While not a replacement for Docker or traditional VMs in performance-critical scenarios, env86 fills a unique niche: **lightweight, distributable, reproducible Linux environments that run anywhere a browser can**.

The project demonstrates sophisticated engineering:
- WebAssembly for x86 emulation
- Serial port RPC for host-guest communication
- User-space networking stack
- Snapshot-based state management
- Desktop app with embedded browser
- GitHub-based image distribution

For developers, educators, and security researchers, env86 offers a compelling alternative to traditional virtualization, especially when portability and ease of distribution are priorities.

## References

- **env86 Repository**: https://github.com/progrium/env86
- **v86 Emulator**: https://github.com/copy/v86
- **go-netstack**: https://github.com/progrium/go-netstack
- **tractor.dev/toolkit-go**: Desktop framework used by env86

## Further Exploration

To dive deeper:

1. **Build an Image**: Create a custom environment image
2. **Web Deployment**: Use `env86 prepare` to create browser-accessible VM
3. **Network Services**: Run web server in VM with port forwarding
4. **9P Mounts**: Mount host directories for development workflows
5. **State Snapshots**: Create pre-configured environments with `--save`

env86 represents an innovative approach to virtualization, proving that with WebAssembly and creative engineering, complex systems like x86 emulation can be made accessible, portable, and easy to distribute.
