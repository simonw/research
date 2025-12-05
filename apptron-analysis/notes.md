# Apptron Analysis Notes

## Initial Setup
- Cloned https://github.com/tractordev/apptron into /tmp
- Deleted README.md as instructed
- Starting code exploration

## Repository Structure
```
apptron/
├── boot.go              # Main WASM entry point (Go→WebAssembly)
├── Dockerfile           # Multi-stage build for kernel, v86, rootfs, worker
├── Makefile             # Build orchestration
├── wrangler.toml        # Cloudflare Workers configuration
├── worker/              # Cloudflare Worker TypeScript code
│   ├── src/
│   │   ├── worker.ts    # Main worker entry, routing
│   │   ├── r2fs.ts      # R2 bucket as filesystem (novel!)
│   │   ├── projects.ts  # Project management
│   │   ├── auth.ts      # Authentication (Hanko)
│   │   └── ...
├── system/              # Linux guest system components
│   ├── cmd/aptn/        # Go CLI tool compiled for Linux x86
│   │   ├── main.go      # CLI entry
│   │   ├── exec.go      # WASM executor (novel!)
│   │   ├── fuse.go      # FUSE filesystem mount
│   │   ├── shm9p.go     # Shared memory 9P server
│   │   └── ports.go     # Port monitoring
│   ├── bin/             # Shell scripts
│   │   ├── init         # Linux init process
│   │   ├── start        # Shell startup
│   │   ├── rebuild      # Environment rebuild
│   │   └── wexec        # WASM execution wrapper
│   └── etc/             # Configuration files
├── extension/           # VS Code extensions
│   ├── system/          # Core apptron extension
│   │   ├── src/web/
│   │   │   ├── extension.ts  # Extension entry
│   │   │   └── bridge.ts     # WanixFS filesystem provider
│   │   └── src/wanix/fs.js   # Wanix filesystem client
│   └── preview/         # Preview extension
└── assets/              # Frontend assets
    ├── lib/apptron.js   # Client-side setup
    └── wanix.min.js     # Wanix runtime
```

## Key Findings

### Technology Stack
- **Go**: Core logic (both WASM and native Linux binaries)
- **TypeScript**: Cloudflare Worker + VS Code extensions
- **WebAssembly**: Browser-side Go runtime
- **v86**: x86 PC emulator in JavaScript
- **9P Protocol**: Plan 9 filesystem protocol for host-guest communication
- **Cloudflare Workers + Containers**: Serverless compute with persistent containers
- **R2**: Object storage used as a full filesystem

### Novel Architectural Elements

1. **Browser-based x86 Linux VM** - Runs full Alpine Linux in browser via v86 emulator
2. **9P over virtio** - Uses Plan 9 filesystem protocol for host↔guest communication
3. **WASM execution from Linux** - Linux guest can execute WASM binaries (both WASI and Go-JS)
4. **R2 as Full Filesystem** - Cloudflare R2 with custom HTTP filesystem semantics
5. **Bidirectional filesystem sync** - Local IDBFS cache synchronized with remote R2
6. **Shared Memory IPC** - High-performance shared memory pipe between host and guest
7. **VS Code integration** - Full editor running in browser with custom filesystem provider
8. **Custom environment builds** - Users can customize the Linux rootfs

### Architecture Flow
1. User navigates to `<uuid>.apptron.dev`
2. Cloudflare Worker loads assets and bootstraps Wanix runtime
3. Go WASM boots, sets up virtual filesystems:
   - tarfs for bundled assets
   - IDBFS for local cache
   - syncfs for remote storage
   - ramfs for temporary files
4. v86 boots Linux with 9P rootfs from the WASM host
5. Linux registers WASM binfmt handler
6. VS Code extension provides filesystem access to VM contents
7. User gets full development environment in browser

## Questions/Areas Investigated
- ✓ What is the main purpose? → Browser-based cloud IDE with full Linux environment
- ✓ What technologies/languages are used? → Go, TypeScript, WASM, v86, 9P, Cloudflare
- ✓ How does the architecture work? → Multi-layered: Browser → WASM → v86 → Linux
- ✓ What are the novel aspects? → R2 filesystem, WASM-in-Linux execution, 9P bridging
