# Vibium Python Client Investigation Notes

## Thu Dec 25 13:17:44 UTC 2025

### Task
- Clone Vibium repo and run QuickStart
- Investigate Go binary architecture
- Document the Node-to-Go protocol
- Build Python client library using TDD

---

## Progress


### Cloned Vibium repo

Successfully cloned https://github.com/VibiumDev/vibium to /tmp/vibium

Built the Go binary:
- `cd /tmp/vibium/clicker && go build -o bin/clicker ./cmd/clicker`
- Binary is ~14MB

QuickStart Status:
- Could not run full QuickStart due to air-gapped environment (no network to download Chrome for Testing)
- However, thoroughly analyzed the source code

---

## Architecture Investigation

### Overall Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         LLM / Agent                         │
└─────────────────────────────────────────────────────────────┘
                      ▲
                      │ MCP Protocol (stdio)
                      ▼
           ┌─────────────────────┐         
           │   Vibium Clicker    │
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
│                        JS/TS Client                         │
└─────────────────────────────────────────────────────────────┘
```

### Go Binary Components (clicker/)

1. **cmd/clicker/main.go** - CLI entry point using Cobra
   - Commands: version, paths, install, launch-test, ws-test, bidi-test, navigate, screenshot, eval, find, click, type, check-actionable, serve, mcp

2. **internal/bidi/** - WebDriver BiDi protocol implementation
   - connection.go: WebSocket connection to browser
   - protocol.go: BiDi message types
   - browsingcontext.go: Page navigation
   - session.go: Session management
   - element.go: Element finding
   - input.go: Click/type actions
   - script.go: JS execution

3. **internal/browser/** - Browser management
   - launcher.go: Launch Chrome with BiDi enabled
   - installer.go: Download Chrome for Testing

4. **internal/proxy/** - WebSocket proxy server
   - server.go: WebSocket server on :9515
   - router.go: Routes messages between client and browser

5. **internal/mcp/** - MCP server
   - server.go: JSON-RPC 2.0 over stdio
   - handlers.go: Tool implementations
   - schema.go: Tool schemas

6. **internal/features/** - Auto-wait and actionability
   - autowait.go: Element polling
   - actionability.go: Visibility/stability checks


---

## Protocol Documentation

### Communication Layers

1. **Node Client → Clicker Proxy** (WebSocket on :9515)
   - Node client spawns `clicker serve` subprocess
   - Connects via WebSocket to `ws://localhost:{port}`
   
2. **Clicker Proxy → Chrome Browser** (WebSocket BiDi)
   - Clicker connects to Chrome's BiDi WebSocket endpoint
   - Routes messages between client and browser

### Message Format: WebDriver BiDi Protocol

The protocol is JSON-based with request/response pattern:

**Command (Request):**
```json
{
  "id": 1,
  "method": "browsingContext.navigate",
  "params": {
    "context": "<context-id>",
    "url": "https://example.com",
    "wait": "complete"
  }
}
```

**Response (Success):**
```json
{
  "id": 1,
  "type": "success",
  "result": {
    "navigation": "<nav-id>",
    "url": "https://example.com"
  }
}
```

**Response (Error):**
```json
{
  "id": 1,
  "type": "error",
  "error": {
    "error": "timeout",
    "message": "Element not found"
  }
}
```

### Standard BiDi Methods (pass-through to browser)

- `session.status` - Check if browser is ready
- `browsingContext.getTree` - Get browsing contexts (tabs)
- `browsingContext.navigate` - Navigate to URL
- `browsingContext.captureScreenshot` - Take screenshot
- `script.callFunction` - Execute JavaScript
- `input.performActions` - Mouse/keyboard actions

### Custom Vibium Extension Commands

The Clicker proxy intercepts these and handles them with auto-wait:

1. **vibium:find** - Find element with wait
   ```json
   {
     "id": 1,
     "method": "vibium:find",
     "params": {
       "context": "<context-id>",
       "selector": "a.link",
       "timeout": 30000
     }
   }
   ```
   Response:
   ```json
   {
     "id": 1,
     "type": "success",
     "result": {
       "tag": "a",
       "text": "Click me",
       "box": {"x": 100, "y": 200, "width": 80, "height": 20}
     }
   }
   ```

2. **vibium:click** - Click element with actionability checks
   ```json
   {
     "id": 1,
     "method": "vibium:click",
     "params": {
       "context": "<context-id>",
       "selector": "button",
       "timeout": 30000
     }
   }
   ```

3. **vibium:type** - Type text with actionability checks
   ```json
   {
     "id": 1,
     "method": "vibium:type",
     "params": {
       "context": "<context-id>",
       "selector": "input",
       "text": "Hello World",
       "timeout": 30000
     }
   }
   ```

### Client Workflow

1. Start `clicker serve` process
2. Wait for "Server listening on ws://localhost:{port}"
3. Connect WebSocket to that port
4. On connect, clicker auto-launches browser and BiDi connection
5. Get context via `browsingContext.getTree`
6. Use standard BiDi or vibium: commands
7. On disconnect, browser closes

---


## Node.js QuickStart Success

Successfully ran the Node.js library with the Go clicker binary:

```javascript
import { browser } from './dist/index.mjs';

const vibe = await browser.launch({
    headless: true,
    executablePath: '/tmp/vibium/clicker/bin/clicker'
});

await vibe.go('file:///tmp/test.html');
const h1 = await vibe.find('h1');
console.log('Found element:', h1.info);
// { tag: 'h1', text: 'Hello World', box: { height: 37, width: 764, x: 8, y: 47.4375 } }

await vibe.quit();
```

Key modifications for headless/container environments:
- Added `--no-sandbox` to Chrome args in launcher.go
- Installed Chrome and chromedriver manually

---

## Python Client Library Development

Starting TDD-based Python client library at:
/home/user/research/vibium-python-client/vibium-python/


## Final Summary

### What was accomplished:

1. **Cloned and analyzed Vibium repo** - Understood Go binary architecture and protocol

2. **Ran Node.js QuickStart** - Successfully built JS client and tested with clicker binary
   - Required adding `--no-sandbox` flag for containerized environment

3. **Built Python client library** using TDD:
   - Sync API matching Node.js design
   - Async API with pytest-asyncio
   - 21 tests all passing

4. **Documented architecture** - Full protocol specification and library usage

### Files created:
- `vibium-python/` - Complete Python library
- `notes.md` - Investigation notes
- `README.md` - Architectural guide
- `vibium-launcher-nosandbox.patch` - Required clicker modification

### Test Results:
```
21 passed in 26.87s
- 2 browser launch tests
- 5 find element tests  
- 2 click tests
- 2 type tests
- 2 screenshot tests
- 2 navigation tests
- 6 async tests
```
