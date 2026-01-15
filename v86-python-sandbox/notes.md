# v86 Python Sandbox Investigation Notes

## Goal
Build a Python library similar to Denobox that can execute bash commands in a v86 sandbox running in a Deno process.

## Investigation Steps

### 1. Cloning repositories
- Cloned simonw/denobox to /tmp/denobox
- Cloned simonw/tools to /tmp/tools

### 2. Examining denobox structure
- Denobox uses NDJSON protocol over stdin/stdout to communicate with a Deno subprocess
- Python spawns Deno, sends JSON requests, receives JSON responses
- Key files: sync_box.py, async_box.py, worker.js

### 3. Examining v86.html
- Uses v86 JavaScript library to run Linux (Buildroot) in browser
- Serial console for input/output via serial0_send and serial0-output-byte listener
- Detects shell prompt with `% $` pattern
- Required files: seabios.bin, vgabios.bin, bzimage, v86.wasm

### 4. Testing v86 in Node.js/Deno
- v86 npm package (0.5.301) works in both Node.js and Deno
- Successfully booted Linux and executed commands
- Key findings:
  - wasm_fn option lets us provide pre-compiled WASM
  - Files can be loaded via `url` option (local paths work)
  - Boot time ~3-5 seconds in testing
  - Shell prompt pattern: `% $`

### 5. Architecture for Python library
- Similar to Denobox: Python spawns Deno subprocess
- Deno runs v86 and boots Linux
- NDJSON protocol for command execution
- Python sends bash commands, receives output

### 6. Implementation
Created v86box library with:
- `v86box/__init__.py` - Package init
- `v86box/sync_box.py` - Synchronous Python wrapper
- `v86box/worker.js` - Deno worker script
- `pyproject.toml` - Python package configuration

Key implementation decisions:
- Use environment variables to pass file paths to Deno worker
- Support both npm import and local file import for v86 library
- Auto-download assets to ~/.v86box on first run
- Shell prompt detection using `% $` pattern for Buildroot

### 7. Testing Results
Successfully tested:
- Basic echo command
- System info (uname -a)
- File reading (cat /etc/os-release)
- Directory listing (ls -la /)
- Status check

Boot time: ~3-5 seconds
Linux version: 6.8.12 (Buildroot 2024.05.2)

### 8. Key Learnings
- v86 npm package works in both Node.js and Deno
- Need to use local file import in Deno due to npm registry certificate issues in some environments
- Serial console interaction requires careful prompt detection
- Multiple output parsing needed to strip echoed commands and prompts
