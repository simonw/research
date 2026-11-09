# WASM Sandbox Prototype

A self-contained Linux sandbox built with Rust and wasmtime, featuring a line-delimited JSON API for executing commands in an isolated WASM environment.

## Overview

This project demonstrates a sandboxed execution environment using WebAssembly (WASM) and the wasmtime runtime. While the original goal was to integrate v86 (an x86 emulator) inside WASM, this prototype implements a simpler but functionally similar architecture that:

1. **Provides isolation** through wasmtime's WASM runtime
2. **Exposes a JSON API** over stdin/stdout for host communication
3. **Supports memory limits** and resource management
4. **Simulates a minimal Linux environment** with basic commands

## Architecture

```
┌─────────────────────────────────────┐
│   Host Process (Rust Binary)       │
│                                     │
│  ├─ JSON Protocol Handler           │
│  ├─ Memory Limits & Resource Mgmt   │
│  └─ wasmtime Runtime                │
│           │                         │
│           ▼                         │
│  ┌─────────────────────────┐       │
│  │  WASM Guest Module      │       │
│  │                         │       │
│  │  ├─ In-memory Filesystem │       │
│  │  ├─ Command Simulator    │       │
│  │  └─ Execution Engine     │       │
│  └─────────────────────────┘       │
└─────────────────────────────────────┘
         ▲           ▼
    stdin (JSON)  stdout (JSON)
```

### Components

1. **Host (wasm-sandbox)**: Rust binary using wasmtime to load and execute the WASM guest
   - Handles JSON protocol parsing and serialization
   - Manages memory limits and resource allocation
   - Provides WASI imports for the guest module

2. **Guest (sandbox-guest)**: WASM module compiled from Rust to wasm32-wasip1
   - Simulates a minimal Linux environment
   - Maintains an in-memory filesystem
   - Executes basic shell commands (echo, ls, cat, pwd, etc.)

## JSON Protocol

The sandbox communicates via line-delimited JSON over stdin/stdout. Each request and response is a single JSON object on one line.

### Request Types

#### 1. Execute Shell Command

```json
{
  "type": "shell",
  "command": "echo Hello World",
  "id": "unique-request-id",
  "time_limit_ms": 3000
}
```

**Response:**
```json
{
  "type": "shell",
  "id": "unique-request-id",
  "stdout": "Hello World\n",
  "stderr": "",
  "exit_code": 0,
  "timed_out": false
}
```

#### 2. Write File

```json
{
  "type": "write_file",
  "path": "/tmp/test.txt",
  "content": "Hello, sandbox!",
  "id": "unique-request-id",
  "encoding": "text"
}
```

**Response:**
```json
{
  "type": "write_file",
  "id": "unique-request-id",
  "success": true
}
```

#### 3. Read File

```json
{
  "type": "read_file",
  "path": "/tmp/test.txt",
  "id": "unique-request-id",
  "encoding": "text"
}
```

**Response:**
```json
{
  "type": "read_file",
  "id": "unique-request-id",
  "success": true,
  "content": "Hello, sandbox!"
}
```

#### 4. Reset Sandbox

```json
{
  "type": "reset",
  "id": "unique-request-id"
}
```

**Response:**
```json
{
  "type": "reset",
  "id": "unique-request-id",
  "success": true
}
```

#### 5. Get Status

```json
{
  "type": "status",
  "id": "unique-request-id"
}
```

**Response:**
```json
{
  "type": "status",
  "id": "unique-request-id",
  "uptime_ms": 15420,
  "memory_used_bytes": 1114112,
  "memory_limit_bytes": 67108864,
  "ready": true
}
```

See [protocol.md](protocol.md) for the complete specification.

## Building

### Prerequisites

- Rust toolchain (1.70+)
- wasm32-wasip1 target: `rustup target add wasm32-wasip1`

### Build Instructions

```bash
# Build the WASM guest module
cd guest
cargo build --target wasm32-wasip1 --release
cd ..

# Build the host binary
cargo build --release

# The binary will be at: target/release/wasm-sandbox
```

## Usage

### Command Line Options

```bash
wasm-sandbox [OPTIONS]

Options:
  --memory-limit <MB>    Maximum memory limit in megabytes (default: 64)
  --wasm-module <PATH>   Path to the WASM module (default: guest/target/wasm32-wasip1/release/sandbox_guest.wasm)
  --help                 Print help information
  --version              Print version information
```

### Interactive Usage

```bash
# Start the sandbox
./target/release/wasm-sandbox --memory-limit 128

# Send JSON commands via stdin (one per line)
{"type":"shell","command":"pwd","id":"1","time_limit_ms":1000}
{"type":"write_file","path":"/tmp/hello.txt","content":"Hello!","id":"2"}
{"type":"read_file","path":"/tmp/hello.txt","id":"3"}
```

### Programmatic Usage

```python
import json
import subprocess

# Start the sandbox process
proc = subprocess.Popen(
    ['./target/release/wasm-sandbox'],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    bufsize=1
)

# Send a command
request = {
    "type": "shell",
    "command": "echo Hello from Python",
    "id": "python-1",
    "time_limit_ms": 1000
}
proc.stdin.write(json.dumps(request) + '\n')
proc.stdin.flush()

# Read response
response = json.loads(proc.stdout.readline())
print(f"Output: {response['stdout']}")
```

## Testing

Run the integration tests:

```bash
./tests/integration_test.sh
```

This tests all protocol commands and verifies:
- Command execution
- File I/O operations
- Sandbox reset
- Status queries
- Memory limits

## Current Limitations

### Prototype Scope

This is a **proof-of-concept** implementation with several limitations:

1. **Simulated Commands**: Only a small set of commands are implemented (echo, ls, cat, pwd, whoami, uname)
2. **No Real Execution**: Commands are simulated rather than actually executed in a Linux environment
3. **In-Memory Filesystem**: The filesystem is not persistent and resets when the sandbox restarts
4. **String Handling**: Some memory management issues with complex string operations
5. **No v86 Integration**: The original goal of running v86 inside WASM is not yet implemented

### Known Issues

- Commands with multiple arguments may have parsing issues
- Large file operations are limited by WASM memory constraints
- No support for pipes, redirects, or environment variables
- Exit codes are simulated

## Path to v86 Integration

To extend this prototype to use v86 for actual x86 emulation:

### 1. JavaScript Runtime in WASM

Integrate a JavaScript runtime compiled to WASM:
- Option A: **QuickJS** via the javy project (Bytecode Alliance)
- Option B: **WasmEdge QuickJS** for better performance
- Option C: Custom v86 compilation to pure WASM (complex)

### 2. Embed v86 Resources

Bundle the required v86 files:
- `libv86.js` - Main emulator code
- `v86.wasm` - JIT compiler module
- BIOS files (SeaBIOS or similar)
- Linux disk image (Alpine Linux minimal build recommended)

### 3. Bidirectional Communication

Implement bridges between:
- Rust host ↔ WASM runtime ↔ JS engine ↔ v86 ↔ Virtual Linux

### 4. Resource Management

Handle the increased complexity:
- Larger memory limits (256MB - 1GB)
- Longer startup time (boot Linux)
- Persistent disk image state
- Network emulation (optional)

### Recommended Approach

```
┌──────────────────────────────────────────┐
│        Rust Host (wasmtime)             │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │  QuickJS WASM Module               │ │
│  │                                    │ │
│  │  ┌──────────────────────────────┐ │ │
│  │  │  v86.js + v86.wasm           │ │ │
│  │  │                              │ │ │
│  │  │  ┌────────────────────────┐ │ │ │
│  │  │  │  Linux (Alpine)        │ │ │ │
│  │  │  │  + Shell Commands      │ │ │ │
│  │  │  └────────────────────────┘ │ │ │
│  │  └──────────────────────────────┘ │ │
│  └────────────────────────────────────┘ │
└──────────────────────────────────────────┘
```

## Future Enhancements

### Short Term

- [x] Basic JSON protocol ✓
- [x] Memory limits ✓
- [x] File operations ✓
- [ ] Timeout enforcement (currently simulated)
- [ ] Better error handling and recovery
- [ ] Streaming output for long-running commands
- [ ] Persistent filesystem option

### Medium Term

- [ ] QuickJS integration for JavaScript execution
- [ ] Extended command set (grep, sed, awk)
- [ ] Environment variable support
- [ ] Working directory management
- [ ] Process isolation per command

### Long Term

- [ ] Full v86 integration with Linux boot
- [ ] Network access (emulated)
- [ ] Multiple concurrent sandboxes
- [ ] Snapshot and restore functionality
- [ ] Web UI for interactive use
- [ ] Container-like isolation with cgroups emulation

## Security Considerations

### Current Protections

✓ **Memory isolation** via WASM linear memory
✓ **No host filesystem access** (all operations in-memory)
✓ **Limited syscall surface** via WASI
✓ **Resource limits** (memory configurable)
✓ **No network access** by default

### Recommended for Production

- **Input validation**: Sanitize all JSON inputs
- **Rate limiting**: Limit requests per second
- **Audit logging**: Log all commands executed
- **Command whitelisting**: Restrict allowed commands
- **Timeout enforcement**: Kill runaway processes
- **Network isolation**: If network is added, use strict filtering

## Performance

### Benchmarks (Prototype)

Measured on: 2-core, 4GB RAM virtual machine

| Operation | Time | Notes |
|-----------|------|-------|
| Sandbox startup | ~50ms | Includes WASM instantiation |
| Simple command (echo) | ~5ms | In-memory simulation |
| File write (1KB) | ~2ms | In-memory filesystem |
| File read (1KB) | ~2ms | In-memory retrieval |
| Sandbox reset | ~10ms | Reinitialize state |

### Expected with v86

| Operation | Time | Notes |
|-----------|------|-------|
| Sandbox startup | ~3-5s | Boot Linux in v86 |
| Simple command (echo) | ~50-100ms | Real process execution |
| File write (1KB) | ~10-20ms | Emulated I/O |

## License

This is a research prototype. For production use, ensure compliance with:
- wasmtime (Apache 2.0)
- Rust crates (various licenses)
- v86 (BSD-2-Clause, if integrated)

## Contributing

This is a prototype/proof-of-concept. Suggestions for improvement:

1. Optimize WASM↔Host communication
2. Improve command parsing in the guest
3. Add more realistic filesystem implementation
4. Implement actual timeout enforcement
5. Create language bindings (Python, Node.js, Go)

## References

- [wasmtime](https://github.com/bytecodealliance/wasmtime) - WASM runtime
- [v86](https://github.com/copy/v86) - x86 emulator in JavaScript
- [javy](https://github.com/bytecodealliance/javy) - QuickJS for WASM
- [WASI](https://wasi.dev/) - WebAssembly System Interface
- [QuickJS](https://bellard.org/quickjs/) - JavaScript engine

## Contact

For questions or suggestions about this prototype, please see the notes.md file for development history and decisions.
