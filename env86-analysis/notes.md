# env86 Analysis Notes

## Initial Investigation

### Repository Information
- **Repository**: https://github.com/progrium/env86
- **Language**: Go (with JavaScript components)
- **Purpose**: VM emulator management tool

## Core Concept Discovery

### v86 Integration
- env86 is a wrapper/management tool around v86
- v86 is a JavaScript x86 PC emulator (https://github.com/copy/v86)
- Runs in browsers via WebAssembly
- Can emulate full x86 PCs with Linux kernels

### Architecture Overview

#### Main Components
1. **env86 CLI** (cmd/env86) - Host-side management tool
2. **guest86** (cmd/guest86) - Guest-side service running inside VM
3. **v86 emulator** - JavaScript/WASM emulator core
4. **Virtual networking** - Custom network stack implementation

#### Key Files Analyzed
- `vm.go` - Core VM management and lifecycle
- `guest.go` - Guest service communication (via serial/COM1)
- `image.go` - VM image loading and state management
- `config.go` - Configuration structures
- `http.go` - HTTP server for console and control
- `network/handler.go` - WebSocket-based networking
- `assets/env86.js` - JavaScript integration layer

## How It Works

### Image Format
Images can be:
1. **Tarball (.tgz)** - Compressed archive containing:
   - `image.json` - VM configuration
   - `fs.json` + `fs/` - Filesystem metadata and files
   - `initial.state` or `initial.state.zst` - VM state snapshot
2. **Directory** - Uncompressed directory with same structure

### Boot Process
1. Load image from local path or global repository (~/.env86/)
2. Mount image filesystem and assets into namespace
3. Start HTTP server on random port
4. Launch desktop window (or headless ChromeDP)
5. Load v86.wasm and boot VM in browser context
6. If initial.state exists: restore from snapshot (fast boot)
7. If cold boot: load kernel, initramfs, and full boot sequence

### Communication Channels
1. **Control WebSocket (/ctl)** - Host <-> v86 emulator
   - VM control (pause, unpause, save, restore)
   - Console control (keyboard, mouse, screen)
   - Configuration exchange
2. **Guest WebSocket (/guest)** - Host <-> guest86 service
   - Command execution in VM
   - Filesystem operations (9P protocol)
   - Network reset
3. **Network WebSocket (/net)** - Virtual network relay
   - QEMU-compatible network protocol
   - Bridges v86 NIC to host network stack

### Guest Service (guest86)
- Binary built for Linux i386
- Runs inside the VM
- Listens on serial port /dev/ttyS1 (COM2)
- Uses CBOR encoding over serial
- Provides RPC endpoints:
  - `vm.Version()` - Get guest service version
  - `vm.Run()` - Execute commands with stdin/stdout streaming
  - `vm.ResetNetwork()` - Reset network interfaces
  - `vm.Mount9P()` - Mount host directories via 9P protocol

### Networking
- Custom virtual network stack (go-netstack)
- Creates 192.168.127.0/24 subnet
- Gateway at 192.168.127.1
- VM typically gets 192.168.127.2
- WebSocket relay bridges browser to host network
- Port forwarding support (e.g., 8080:80)

## CLI Commands Discovered

### boot
- Boot and run a VM interactively
- Options: --net, --save, --cold, --ttyS0, --console-url, --cdp
- Supports port forwarding (-p)

### run
- Execute a command inside the VM (requires guest service)
- Headless by default
- Options: --net, --cdp, -p (port forward), -m (mount)
- Uses PTY for command execution

### pull
- Download images from GitHub repositories
- Format: `github.com/user/repo[@tag]`
- Stores in ~/.env86/ (global images)
- Looks for releases with .tgz assets

### prepare
- Prepare an image for browser deployment
- Bundles assets and splits state files
- Outputs standalone directory

### create
- Create new VM images
- Not fully analyzed yet

### serve
- Serve an image via HTTP
- For browser-based access
- Not fully analyzed yet

### network
- Network debugging/utilities
- Not fully analyzed yet

### assets
- Manage bundled assets
- Not fully analyzed yet

## Technical Details

### Desktop Integration
- Uses tractor.dev/toolkit-go/desktop
- Creates native windows on macOS/Windows/Linux
- Embeds WebView2 (Windows) or WebKit (macOS/Linux)
- Falls back to ChromeDP for headless mode

### State Management
- Can save/restore VM state (pause/resume)
- Initial state allows instant boot
- State can be compressed with zstd
- Large states split into 10MB chunks for browser loading

### Filesystem
- Custom implementations: tarfs, namespacefs, osfs
- 9P protocol for host directory mounting
- Lazy loading via fs.json metadata

### Dependencies (Key Ones)
- `github.com/copy/v86` - The actual emulator (bundled)
- `github.com/progrium/go-netstack` - Virtual networking
- `github.com/hugelgupf/p9` - Plan 9 filesystem protocol
- `tractor.dev/toolkit-go` - Desktop app framework
- `github.com/chromedp/chromedp` - Headless Chrome
- `golang.org/x/sys` - Replaced with WASM-compatible fork

## Build Process

### Assets Building (Makefile)
1. **guest**: Build guest86 for linux/386
2. **kernel**: Build Linux kernel + initramfs in Docker (i386/alpine:3.18.6)
3. **v86**: Build v86.wasm and libv86.js from source

### Binary Building
- Builds for Linux, Windows, macOS (amd64, arm64)
- macOS binaries are code-signed
- Uses goreleaser for releases

## Use Cases Identified

1. **Development Environments**
   - Reproducible Linux environments
   - Can mount host directories
   - Network-enabled for testing

2. **Testing/CI**
   - Headless execution with --cdp
   - Run commands without GUI
   - Fast boot from snapshots

3. **Education**
   - Browser-based Linux access
   - No installation required (prepare + serve)
   - Snapshot-based state management

4. **Sandboxing**
   - Isolated x86 environment
   - Virtual networking
   - No VM escape (JavaScript sandbox)

5. **Legacy Software**
   - Run i386 Linux programs
   - Self-contained images
   - Cross-platform (runs anywhere with browser)

## Interesting Findings

1. **WebAssembly-based emulation** - Entire x86 PC in browser
2. **Serial-based RPC** - Creative use of COM ports for host-guest communication
3. **Desktop app with embedded browser** - Not a typical web app
4. **Snapshot-based boot** - Can boot Linux in seconds
5. **9P filesystem** - Plan 9 protocol for directory sharing
6. **Image distribution via GitHub releases** - Simple package management
7. **Modified syscall package** - WASM compatibility fork

## Questions Resolved

- **Why both env86 and guest86?** - env86 manages the VM from outside, guest86 provides services from inside
- **Why desktop app?** - v86 is browser-based, needs WebView or Chrome
- **How does networking work?** - Virtual network stack with WebSocket relay
- **Image format?** - Tarball or directory with specific structure
- **Fast boot mechanism?** - Save/restore VM state snapshots
- **Host-guest file sharing?** - 9P protocol over network

## Architecture Diagram (Mental Model)

```
┌─────────────────────────────────────────────────────────┐
│ Host System (env86 binary)                              │
│                                                          │
│  ┌────────────┐      ┌──────────────┐                  │
│  │ HTTP Server│◄─────┤ Desktop      │                  │
│  │ :random    │      │ Window       │                  │
│  └─────┬──────┘      │ (WebView)    │                  │
│        │             └──────┬───────┘                  │
│        │                    │                           │
│        │ WebSocket          │                           │
│        │ Channels           │ Renders                   │
│        │                    │                           │
│  ┌─────▼──────────────────────┐                        │
│  │ v86 Emulator (WASM)        │                        │
│  │  ┌──────────────────────┐  │                        │
│  │  │ i386 Linux           │  │                        │
│  │  │  ┌────────────────┐  │  │                        │
│  │  │  │ guest86 service│  │  │                        │
│  │  │  │ (on /dev/ttyS1)│  │  │                        │
│  │  │  └────────────────┘  │  │                        │
│  │  └──────────────────────┘  │                        │
│  └─────────────────────────────┘                        │
│                                                          │
│  ┌────────────────────┐                                 │
│  │ Virtual Network    │                                 │
│  │ 192.168.127.0/24   │                                 │
│  └────────────────────┘                                 │
└─────────────────────────────────────────────────────────┘
```

## Potential Improvements Noted (Not Implemented)

- zstd compression compatibility issue mentioned in code
- TODO: git diff saving for modified repos
- TODO: NetworkPipe() not implemented
- Initial state symlinks handling
- Better error messages in some areas
