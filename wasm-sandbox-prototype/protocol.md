# WASM Sandbox JSON Protocol Specification

## Overview
The WASM sandbox communicates via line-delimited JSON over stdin/stdout. Each request and response is a single JSON object on one line.

## Request Types

### 1. Shell Command
Execute a shell command in the sandbox.

**Request:**
```json
{
  "type": "shell",
  "command": "ls /",
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "time_limit_ms": 3000
}
```

**Response:**
```json
{
  "type": "shell",
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "stdout": "bin\nboot\ndev\netc\nhome\n...",
  "stderr": "",
  "exit_code": 0,
  "timed_out": false
}
```

**Fields:**
- `command` (string, required): The shell command to execute
- `id` (string, required): Unique identifier for correlating request/response
- `time_limit_ms` (number, optional): Maximum execution time in milliseconds (default: 5000)

**Response Fields:**
- `stdout` (string): Standard output from the command
- `stderr` (string): Standard error from the command
- `exit_code` (number): Exit code of the command
- `timed_out` (boolean): Whether the command exceeded the time limit

### 2. Write File
Write data to a file in the sandbox filesystem.

**Request:**
```json
{
  "type": "write_file",
  "path": "/tmp/test.txt",
  "content": "Hello, world!",
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "mode": "0644"
}
```

**Response:**
```json
{
  "type": "write_file",
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "success": true,
  "error": null
}
```

**Fields:**
- `path` (string, required): Path where the file should be written
- `content` (string, required): File content (can be base64-encoded for binary)
- `id` (string, required): Unique identifier
- `mode` (string, optional): File permissions in octal notation (default: "0644")
- `encoding` (string, optional): "text" or "base64" (default: "text")

### 3. Read File
Read a file from the sandbox filesystem.

**Request:**
```json
{
  "type": "read_file",
  "path": "/tmp/test.txt",
  "id": "550e8400-e29b-41d4-a716-446655440002",
  "encoding": "text"
}
```

**Response:**
```json
{
  "type": "read_file",
  "id": "550e8400-e29b-41d4-a716-446655440002",
  "content": "Hello, world!",
  "success": true,
  "error": null
}
```

**Fields:**
- `encoding` (string, optional): "text" or "base64" (default: "text")

### 4. Reset Sandbox
Reset the sandbox to its initial state, clearing all filesystem changes.

**Request:**
```json
{
  "type": "reset",
  "id": "550e8400-e29b-41d4-a716-446655440003"
}
```

**Response:**
```json
{
  "type": "reset",
  "id": "550e8400-e29b-41d4-a716-446655440003",
  "success": true
}
```

### 5. Get Status
Query the current status of the sandbox.

**Request:**
```json
{
  "type": "status",
  "id": "550e8400-e29b-41d4-a716-446655440004"
}
```

**Response:**
```json
{
  "type": "status",
  "id": "550e8400-e29b-41d4-a716-446655440004",
  "uptime_ms": 15420,
  "memory_used_bytes": 4194304,
  "memory_limit_bytes": 67108864,
  "ready": true
}
```

## Error Handling

If an error occurs, the response will include an error field:

```json
{
  "type": "shell",
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "error": "Command execution failed: timeout exceeded",
  "stdout": "",
  "stderr": "",
  "exit_code": null,
  "timed_out": true
}
```

## CLI Options

The sandbox binary accepts the following options:

```bash
wasm-sandbox [OPTIONS]

Options:
  --memory-limit <MB>    Maximum memory limit in megabytes (default: 64)
  --disk-image <PATH>    Path to the disk image file (required for v86 mode)
  --mode <MODE>          Sandbox mode: "simple" or "v86" (default: "simple")
  --help                 Print help information
  --version              Print version information
```

## Example Usage

```bash
# Start the sandbox
./wasm-sandbox --memory-limit 128

# Send commands via stdin (one JSON object per line)
{"type":"shell","command":"echo 'Hello'","id":"1","time_limit_ms":1000}
{"type":"write_file","path":"/tmp/test","content":"data","id":"2"}
{"type":"shell","command":"cat /tmp/test","id":"3","time_limit_ms":1000}
{"type":"reset","id":"4"}
```

## Protocol Design Decisions

1. **Line-delimited JSON**: Simple to parse, one request/response per line
2. **Request IDs**: Allow async operations and correlation
3. **Time limits**: Prevent runaway processes
4. **Base64 encoding**: Support binary file operations
5. **Exit codes**: Preserve shell semantics
6. **Reset capability**: Enable sandbox reuse without restart
