# Vibium Python Client Investigation

This investigation explores the [Vibium](https://github.com/VibiumDev/vibium) browser automation project and implements a Python client library compatible with the existing Go binary and Node.js client.

## Overview

Vibium is browser automation infrastructure built for AI agents. It provides:
- A single Go binary ("clicker") that handles browser lifecycle and WebDriver BiDi protocol
- A Node.js client library for JavaScript developers
- MCP server for LLM agent integration

This investigation adds a **Python client library** with both sync and async APIs.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         LLM / Agent                         │
│          (Claude Code, Codex, Gemini, Local Models)         │
└─────────────────────────────────────────────────────────────┘
                      ▲
                      │ MCP Protocol (stdio)
                      ▼
           ┌─────────────────────┐
           │   Vibium Clicker    │
           │     (Go Binary)     │
           │                     │
           │  ┌───────────────┐  │
           │  │  MCP Server   │  │
           │  └───────▲───────┘  │         ┌──────────────────┐
           │          │          │         │                  │
           │  ┌───────▼───────┐  │WebSocket│                  │
           │  │  BiDi Proxy   │  │◄───────►│  Chrome Browser  │
           │  └───────────────┘  │  BiDi   │                  │
           │                     │         │                  │
           └─────────────────────┘         └──────────────────┘
                      ▲
                      │ WebSocket BiDi :9515
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              Client Libraries (JS/TS, Python)               │
└─────────────────────────────────────────────────────────────┘
```

### Go Binary Components

The clicker binary (`clicker/`) is the core automation engine:

| Component | Path | Description |
|-----------|------|-------------|
| **CLI** | `cmd/clicker/main.go` | Cobra-based CLI with commands: serve, mcp, navigate, click, etc. |
| **BiDi Client** | `internal/bidi/` | WebDriver BiDi protocol implementation |
| **Browser Launcher** | `internal/browser/` | Chrome/chromedriver management |
| **Proxy Server** | `internal/proxy/` | WebSocket server routing messages between client and browser |
| **MCP Server** | `internal/mcp/` | JSON-RPC 2.0 over stdio for LLM integration |
| **Actionability** | `internal/features/` | Auto-wait and visibility checks |

### Protocol: Node/Python Client → Go Binary

The client communicates with the clicker via WebSocket using WebDriver BiDi protocol:

**1. Start clicker serve process:**
```bash
clicker serve --headless
# Outputs: Server listening on ws://localhost:9515
```

**2. Connect WebSocket and send commands:**

```json
// Get browsing context
{"id": 1, "method": "browsingContext.getTree", "params": {}}

// Navigate to URL
{"id": 2, "method": "browsingContext.navigate", "params": {
  "context": "<context-id>",
  "url": "https://example.com",
  "wait": "complete"
}}
```

**3. Custom Vibium extension commands:**

```json
// Find element with auto-wait
{"id": 3, "method": "vibium:find", "params": {
  "context": "<context-id>",
  "selector": "h1",
  "timeout": 30000
}}
// Response: {"id": 3, "type": "success", "result": {"tag": "h1", "text": "Hello", "box": {...}}}

// Click with actionability checks
{"id": 4, "method": "vibium:click", "params": {
  "context": "<context-id>",
  "selector": "button",
  "timeout": 30000
}}

// Type with actionability checks
{"id": 5, "method": "vibium:type", "params": {
  "context": "<context-id>",
  "selector": "input",
  "text": "Hello World",
  "timeout": 30000
}}
```

## Python Client Library

The `vibium-python` library provides both sync and async APIs matching the Node.js client design.

### Installation

```bash
cd vibium-python
uv sync
```

### Sync API

```python
from vibium_python import browser

vibe = browser.launch(headless=True, executable_path="/path/to/clicker")
vibe.go("https://example.com")

element = vibe.find("h1")
print(f"Found: {element.tag} - {element.text}")

button = vibe.find("button")
button.click()

input_elem = vibe.find("input")
input_elem.type("Hello World")

png_data = vibe.screenshot()

vibe.quit()
```

### Async API

```python
import asyncio
from vibium_python import async_browser

async def main():
    vibe = await async_browser.launch(headless=True, executable_path="/path/to/clicker")
    await vibe.go("https://example.com")

    h1 = await vibe.find("h1")
    print(f"Found: {h1.text}")

    await vibe.quit()

asyncio.run(main())
```

### Library Structure

```
vibium-python/
├── src/vibium_python/
│   ├── __init__.py       # Exports all APIs
│   ├── browser.py        # Sync launch()
│   ├── vibe.py          # Sync Vibe class
│   ├── element.py       # Sync Element class
│   ├── bidi.py          # Sync BiDi WebSocket client
│   ├── clicker.py       # Clicker process management
│   ├── async_browser.py # Async launch()
│   ├── async_vibe.py    # Async Vibe class
│   ├── async_element.py # Async Element class
│   └── async_bidi.py    # Async BiDi WebSocket client
├── tests/
│   ├── conftest.py      # Fixtures (HTTP server, clicker path)
│   ├── fixtures/        # Test HTML files
│   └── test_*.py        # Test files
└── pyproject.toml
```

## Running Tests

```bash
cd vibium-python
uv run pytest -v
```

All 21 tests pass:
- 15 sync tests (launch, navigation, find, click, type, screenshot)
- 6 async tests (matching sync functionality)

## Key Findings

1. **Protocol Extension**: Vibium extends WebDriver BiDi with custom `vibium:` methods for actionability-aware operations

2. **Auto-wait**: The `vibium:find`, `vibium:click`, and `vibium:type` commands poll for elements and check visibility/stability before acting

3. **Architecture Simplicity**: The Go binary handles all browser complexity, clients just need WebSocket JSON messaging

4. **MCP Integration**: The same binary can run as an MCP server for LLM agent integration

## Files Created

- `vibium-python/` - Complete Python client library
- `notes.md` - Investigation notes and protocol documentation
- `README.md` - This architectural guide
