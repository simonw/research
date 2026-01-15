# v86box - Python Library for v86 Linux Sandbox

A Python library for executing bash commands in a v86 Linux sandbox running in a Deno process. Similar in design to [denobox](https://github.com/simonw/denobox), but instead of running JavaScript in a Deno sandbox, it boots an entire Linux VM using [v86](https://github.com/copy/v86) and executes bash commands.

## Overview

`v86box` provides a sandboxed Linux environment where you can execute bash commands safely. The Linux VM runs in an x86 emulator (v86) inside Deno, providing multiple layers of isolation:

1. **Emulation layer**: v86 emulates x86 hardware in WebAssembly
2. **Deno sandbox**: The emulator runs in Deno with minimal permissions
3. **Linux isolation**: Commands run inside a minimal Buildroot Linux VM

## Features

- Execute bash commands in a fully sandboxed Linux environment
- No file system access to the host (except explicitly configured paths)
- No network access by default
- JSON-based communication protocol
- Automatic asset downloading on first run
- Both synchronous and async APIs (sync implemented, async TODO)

## Installation

```bash
pip install v86box
```

Or install from source:

```bash
pip install .
```

### Requirements

- Python 3.10+
- The `deno` PyPI package (automatically installed)

## Usage

### Basic Usage

```python
from v86box import V86box

with V86box() as box:
    # Execute a simple command
    result = box.exec("echo 'Hello from Linux!'")
    print(result)  # "Hello from Linux!"

    # Check system info
    result = box.exec("uname -a")
    print(result)  # "Linux (none) 6.8.12 ... i686 GNU/Linux"

    # Read files
    result = box.exec("cat /etc/os-release")
    print(result)  # Shows Buildroot info
```

### Configuration Options

```python
from v86box import V86box

box = V86box(
    # Custom asset paths (optional - auto-downloaded if not specified)
    bios_path="/path/to/seabios.bin",
    vga_bios_path="/path/to/vgabios.bin",
    bzimage_path="/path/to/bzimage.bin",
    wasm_path="/path/to/v86.wasm",

    # Asset storage directory (default: ~/.v86box)
    assets_dir="/custom/assets/dir",

    # Timeouts
    boot_timeout=60.0,     # Max seconds to wait for VM boot
    command_timeout=30.0,  # Default command timeout
)
```

### Error Handling

```python
from v86box import V86box, V86boxError

with V86box() as box:
    try:
        result = box.exec("false")  # Command that returns non-zero
        # Note: exit codes are not captured, only stdout
    except V86boxError as e:
        print(f"Error: {e}")

    # Check status
    status = box.status()
    print(status)  # {'ready': True, 'pending': 0}
```

## Architecture

### Components

1. **V86box (Python)**: Manages the Deno subprocess and NDJSON protocol
2. **worker.js (Deno)**: Runs v86 emulator, handles command execution
3. **v86 (WebAssembly)**: x86 emulator that runs the Linux VM
4. **Buildroot Linux**: Minimal Linux distribution running in the VM

### Communication Protocol

Communication uses newline-delimited JSON (NDJSON) over stdin/stdout:

**Requests:**
```json
{"id": 1, "type": "exec", "command": "echo hello"}
{"id": 2, "type": "status"}
{"id": 3, "type": "shutdown"}
```

**Responses:**
```json
{"type": "ready"}
{"id": 1, "result": "hello"}
{"id": 2, "result": {"ready": true, "pending": 0}}
{"id": 3, "result": true, "shutdown": true}
```

### Required Assets

On first run, v86box automatically downloads:

| Asset | Size | Description |
|-------|------|-------------|
| `seabios.bin` | 128 KB | SeaBIOS firmware |
| `vgabios.bin` | 36 KB | VGA BIOS |
| `buildroot-bzimage68.bin` | ~10 MB | Linux kernel + initramfs |
| `v86.wasm` | ~2 MB | v86 WebAssembly module |
| `libv86.mjs` | ~335 KB | v86 JavaScript library |

These are stored in `~/.v86box/` by default.

## Limitations

- **No network access**: The VM has no network connectivity
- **No persistent storage**: All changes are lost when the VM stops
- **Boot time**: VM boot takes 3-5 seconds
- **x86 32-bit only**: v86 emulates 32-bit x86 (no 64-bit support)
- **Serial console only**: No graphical output

## How It Works

1. Python spawns a Deno subprocess running `worker.js`
2. Worker loads v86 and boots Buildroot Linux
3. Linux boots and presents a shell on serial console
4. Worker monitors serial output for shell prompt
5. When ready, worker signals Python via JSON
6. Python sends commands as JSON, worker sends to VM serial
7. Worker captures output until next prompt, returns to Python

## Development

```bash
# Clone and setup
git clone <repo>
cd v86-python-sandbox

# Run tests
python test_v86box.py
```

## Comparison to Denobox

| Feature | Denobox | v86box |
|---------|---------|--------|
| Language | JavaScript | Bash |
| Runtime | Deno V8 | v86 Linux VM |
| Sandbox | Deno permissions | Emulation + Deno |
| Boot time | Instant | 3-5 seconds |
| Use case | JS/WASM execution | Shell commands |

## License

Apache 2.0 (same as denobox)

## Credits

- [v86](https://github.com/copy/v86) - x86 emulator by Fabian
- [denobox](https://github.com/simonw/denobox) - Inspiration for architecture
- [Buildroot](https://buildroot.org/) - Linux distribution in the VM
- Simon Willison's [v86.html](https://tools.simonwillison.net/v86) - Reference implementation
