# Research Notes: Cloudflare Remote Browser with User Takeover

## Research Date: December 22, 2025

## Goal
Build a ChatGPT Operator-like system using Cloudflare infrastructure:
- Run Playwright scripts on a remote browser
- Stream browser view to a web interface
- Allow user takeover for logins, captchas, 2FA, etc.

---

## Key Technologies Researched

### 1. Cloudflare Browser Rendering
- **What it is:** Managed headless Chrome on Cloudflare's network
- **Access methods:**
  - REST API: Simple one-off actions (screenshots, PDFs)
  - Workers Binding: Full Playwright access
- **Key features:**
  - Uses Chrome DevTools Protocol (CDP) via WebSocket internally
  - `@cloudflare/playwright` packages
  - Sessions can be kept alive 1-10 minutes via Durable Objects
- **Documentation:** https://developers.cloudflare.com/browser-rendering/

### 2. Cloudflare Containers
- **What it is:** Full Docker containers running on Cloudflare
- **Use case:** When you need custom runtimes, long-running processes
- **Comparison to Browser Rendering:**
  - More control but more setup (need Dockerfile)
  - Better for very long sessions
  - Browser Rendering is simpler for browser automation
- **Documentation:** https://developers.cloudflare.com/containers/

### 3. Cloudflare Calls/Realtime (WebRTC)
- **What it is:** WebRTC SFU infrastructure for real-time streaming
- **Sub-second latency** for video/audio
- **Overkill for this use case** - CDP screencast is sufficient
- **Documentation:** https://developers.cloudflare.com/realtime/

### 4. AgentCast (Open Source Reference)
- **GitHub:** https://github.com/acoyfellow/agentcast
- **What it does:** Exactly what we want to build
  - Live browser sessions for AI agents
  - CDP screencasting for real-time view
  - Uses Cloudflare Containers + Agents SDK
  - Stagehand for natural language control
- **Key learnings:**
  - CDP screencast is the right approach for streaming
  - Direct input forwarding via CDP works well
  - Viewer can be embedded iframe or custom component

### 5. Chrome DevTools Protocol (CDP) Screencast
- **Method:** `Page.startScreencast` / `Page.screencastFrame`
- **Performance considerations:**
  - Must acknowledge each frame with `Page.screencastFrameAck`
  - Frame rate depends on acknowledgment speed
  - Recommended settings: JPEG 60% quality, everyNthFrame: 2
  - Network-intensive with base64-encoded frames
- **Input forwarding:**
  - `Input.dispatchMouseEvent` for mouse
  - `Input.dispatchKeyEvent` for keyboard

### 6. ChatGPT Operator (Reference Implementation)
- **How it works:**
  - AI agent controls browser via screenshot analysis
  - User can take over at any time
  - Proactively asks for takeover on logins/captchas
  - Shows real-time browser view
- **Key UX patterns:**
  - Clear "takeover mode" indicator
  - User clicks "Done" to return control
  - Sensitive fields handled by user, not agent

---

## Streaming Approach Comparison

| Approach | Latency | Bandwidth | User Takeover | Complexity |
|----------|---------|-----------|---------------|------------|
| CDP Screencast | ~100-500ms | Medium-High | Easy (CDP input) | Low |
| WebRTC (Calls) | <100ms | Lower | Complex | High |
| Periodic Screenshots | 500ms-2s | Medium | Easiest | Lowest |

**Decision:** CDP Screencast is the best balance for this use case.

---

## Architecture Decision: Browser Rendering vs Containers

### Browser Rendering (Chosen)
- ✅ Built-in managed Chrome - no Docker needed
- ✅ Simpler deployment (`wrangler deploy`)
- ✅ CDP access via `@cloudflare/playwright`
- ✅ Can use Durable Objects for session persistence
- ⚠️ Max 10 min keep-alive (sufficient for 5 min sessions)

### Containers (Alternative)
- ✅ Full control over browser environment
- ✅ Longer session support
- ❌ Need to build/maintain Docker image
- ❌ More complex setup

**Decision:** Use Browser Rendering with Durable Objects.

---

## Key Technical Discoveries

### 1. CDP Session from Playwright
```typescript
const cdp = await page.context().newCDPSession(page);
```

### 2. Screencast Configuration
```typescript
await cdp.send('Page.startScreencast', {
  format: 'jpeg',
  quality: 60,        // Balance quality/bandwidth
  maxWidth: 1280,
  maxHeight: 720,
  everyNthFrame: 2    // ~15fps effective
});
```

### 3. Frame Acknowledgment (Critical!)
```typescript
cdp.on('Page.screencastFrame', async (frame) => {
  // Process frame first
  broadcastFrame(frame.data);
  // Then acknowledge to get next frame
  await cdp.send('Page.screencastFrameAck', { sessionId: frame.sessionId });
});
```

### 4. Input Forwarding
```typescript
// Mouse
await cdp.send('Input.dispatchMouseEvent', {
  type: 'mousePressed', // or 'mouseReleased', 'mouseMoved'
  x: 100,
  y: 200,
  button: 'left',
  clickCount: 1
});

// Keyboard
await cdp.send('Input.dispatchKeyEvent', {
  type: 'keyDown', // or 'keyUp', 'char'
  key: 'a',
  text: 'a'
});
```

### 5. Script Execution with Takeover
- Use `AsyncFunction` constructor for dynamic code execution
- Create a Promise that `requestTakeover()` returns
- Resolve the promise when user clicks "Done"
- Script execution naturally pauses at `await requestTakeover()`

---

## External References

### Documentation
- [Cloudflare Browser Rendering](https://developers.cloudflare.com/browser-rendering/)
- [Cloudflare Playwright](https://developers.cloudflare.com/browser-rendering/playwright/)
- [Cloudflare Durable Objects](https://developers.cloudflare.com/durable-objects/)
- [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/tot/Page/)
- [Playwright Page API](https://playwright.dev/docs/api/class-page)

### Code References
- [AgentCast](https://github.com/acoyfellow/agentcast) - CDP streaming implementation
- [Cypress CDP Automation](https://github.com/cypress-io/cypress/blob/develop/packages/server/lib/browsers/cdp_automation.ts) - Screencast handling
- [Steel Browser](https://github.com/steel-dev/steel-browser) - Human-in-the-loop controls

### Articles
- [Cloudflare Browser Rendering API GA](https://blog.cloudflare.com/browser-rendering-api-ga-rolling-out-cloudflare-snippets-swr-and-bringing-workers-for-platforms-to-our-paygo-plans/)
- [Introducing Operator (OpenAI)](https://openai.com/index/introducing-operator/)
- [Browserless Hybrid Automations](https://www.browserless.io/blog/how-to-run-puppeteer-within-chrome-to-create-hybrid-automations)

---

## Constraints & Limitations

1. **Browser Rendering Limits:**
   - Concurrent browsers: Depends on plan
   - Session keep-alive: Max 10 minutes
   - Requires `nodejs_compat` flag

2. **CDP Screencast:**
   - Network-heavy (base64 JPEG frames)
   - CPU usage can spike with animations
   - Frame rate depends on acknowledgment speed

3. **Script Execution:**
   - Need to sandbox user code (security consideration)
   - 5-minute timeout per session
   - Single browser tab per session

---

## Questions Resolved

1. **Form-based vs Direct takeover?** → Direct (for captchas)
2. **Scripts provided how?** → UI textarea with sample pre-filled
3. **Takeover detection?** → Explicit `requestTakeover()` call in script
4. **Session trigger?** → Manual from UI
5. **Authentication?** → Cloudflare API key in env var
6. **Session limits?** → Rely on Cloudflare's limits
7. **Error handling?** → Show error, stop script
## Issue: fs.mkdtemp Error

Date: Tue Dec 23 00:53:33 EST 2025

### Problem
When calling `POST /init` to create a session, the worker throws:
```
browserType.connectOverCDP: [unenv] fs.mkdtemp is not implemented yet!
```

### Investigation
- Using `@cloudflare/playwright@1.0.0`
- Configuration has `nodejs_compat` flag and compatibility_date = "2025-09-15"
- The `launch()` function internally tries to use `fs.mkdtemp` which isn't fully polyfilled

### Findings from Research
- Found references to `connectOverCDP()` in cloudflare/playwright-mcp repository
- The MCP server uses `playwright.chromium.connectOverCDP()` instead of `launch()`
- This might be the correct approach for Cloudflare Workers

### Next Steps
- Investigate if `launch()` is the wrong API for Cloudflare Workers
- Check if we should use `acquire()` + `connect()` pattern instead
- Review official examples more carefully


### Solution Found!

**Root Cause:**
The error `browserType.connectOverCDP: [unenv] fs.mkdtemp is not implemented yet!` was caused by an outdated compatibility date in wrangler.toml.

**The Fix:**
Changed `compatibility_date` from `"2025-09-15"` to `"2025-09-17"` in worker/wrangler.toml

**Why This Works:**
- `@cloudflare/playwright` requires `nodejs_compat` flag AND a compatibility date of 2025-09-15 or later
- The `node:fs` module (which Playwright needs for temp files) requires compatibility date of 2025-09-01 or later
- Using 2025-09-17 (as shown in official docs) ensures both requirements are met
- This enables native `node:fs` instead of unenv polyfills, which don't implement `fs.mkdtemp`

**Test Results:**
```bash
$ curl -X POST https://remote-browser.vana.workers.dev/sessions \
  -H "Authorization: Bearer dev-api-key-12345"
  
{"sessionId":"9e82c55c-a806-4992-92e6-ddf155d995a0"}
```

✅ Session creation now works!
✅ Deployment successful
✅ No more fs.mkdtemp errors

**Updated Configuration:**
```toml
compatibility_date = "2025-09-17"
compatibility_flags = ["nodejs_compat"]
```

## Error: Code Generation Disallowed

Date: Tue Dec 23 01:01:40 EST 2025

### Problem
```
EvalError: Code generation from strings disallowed for this context
at new AsyncFunction (<anonymous>)
at BrowserSession.runScript
```

### Root Cause
Line 155-156 in session.ts:
```typescript
const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
const scriptFn = new AsyncFunction('page', 'requestTakeover', code);
```

Cloudflare Workers blocks `eval()`, `Function()`, and `AsyncFunction()` constructors by default for security.

### Solutions to Explore
1. Add `unsafe-eval` to compatibility flags (if available)
2. Use Web Workers API (if supported in Durable Objects)
3. Alternative: Use a safer subset of operations (no dynamic code)
4. Alternative: Pre-define allowed operations and use a DSL instead of JavaScript

### Next Step
Check Cloudflare docs for dynamic code execution in Workers


### Recommended Solution: Client-Side Script Execution

**Problem:** Cloudflare Workers blocks `eval()`, `Function()`, and `AsyncFunction()` for security.

**Why This Is a Hard Blocker:**
- No `unsafe-eval` compatibility flag exists for Workers
- quickjs-emscripten has WASM loading issues in Workers
- Python Workers don't support Playwright
- This is by design for security

**Best Solution: Move Script Execution to Client (Frontend)**

Instead of:
```
Frontend → Send Code → Worker Executes → Controls Browser
```

Do:
```
Frontend Executes Code → Send Commands → Worker → Controls Browser
```

**Architecture:**
1. User writes Playwright script in Monaco editor (frontend)
2. Frontend executes script using `eval()` or `AsyncFunction` (allowed in browser)
3. Script calls `page.*` methods which are **proxied** to send commands to Worker
4. Worker receives commands and applies them to real browser
5. Worker streams back results/frames

**Benefits:**
- ✅ No eval restrictions (browsers allow it)
- ✅ User can see code execute in real-time with browser DevTools
- ✅ Simpler Worker code (just command executor)
- ✅ Better debugging experience
- ✅ Could even work offline for testing

**Drawbacks:**
- ⚠️ More complex frontend (need Playwright API proxy)
- ⚠️ Network latency for each command
- ⚠️ Can't use server-side only features

**Alternative: Predefined Script Templates**
- Simpler but less flexible
- Worker has list of allowed scripts
- User selects from dropdown instead of writing code
- Good for MVP, limiting attack surface


## Implementation Complete: Client-Side Script Execution

### Architecture Changes

**Before:**
```
Frontend → Send Code String → Worker executes with AsyncFunction (❌ BLOCKED)
```

**After:**
```
Frontend executes code → Proxied page.* calls → WebSocket commands → Worker executes
```

### Key Components Created

1. **PlaywrightProxy** (`frontend/src/lib/playwright-proxy.ts`)
   - Manages command/response flow over WebSocket
   - Tracks pending commands with promises
   - Handles timeouts (30s per command)

2. **PlaywrightPageProxy** 
   - Mimics Playwright Page API
   - Each method call sends command to Worker
   - Returns promise that resolves when Worker responds
   - Supported methods: goto, click, fill, type, press, waitForSelector, screenshot, title, evaluate, getByPlaceholder, getByText, getByRole

3. **PlaywrightLocatorProxy**
   - Supports locator chaining (e.g., `page.getByPlaceholder().fill()`)

4. **Worker executeCommand**
   - Receives commands via WebSocket
   - Executes on real Playwright page
   - Sends results back with commandId

### WebSocket Protocol Updates

**New Message Types:**
```typescript
// Client → Worker
{
  type: 'command',
  commandId: 'cmd_123',
  method: 'goto',
  args: ['https://example.com']
}

// Worker → Client
{
  type: 'commandResult',
  commandId: 'cmd_123',
  result: {...},
  error?: 'Error message'
}
```

### How It Works

1. User writes script in Monaco editor (frontend)
2. Clicks "Run Script"
3. Frontend calls `api.runScript()` to notify Worker
4. Frontend executes script using `AsyncFunction` (✅ ALLOWED in browser)
5. Script uses proxied `page` object
6. Each `page.*` call:
   - Generates unique commandId
   - Sends command via WebSocket
   - Returns promise
   - Promise resolves when Worker sends commandResult
7. Worker receives command, executes on real browser, sends result
8. Frontend script continues execution with result

### Example Flow

**User Script:**
```javascript
await page.goto('https://example.com');
const title = await page.title();
return { title };
```

**Under the Hood:**
```
1. page.goto() → WS: {type:'command', commandId:'cmd_1', method:'goto', args:['https://...']}
2. Worker executes: this.page.goto('https://...')
3. Worker → WS: {type:'commandResult', commandId:'cmd_1', result: {...}}
4. Frontend promise resolves
5. page.title() → WS: {type:'command', commandId:'cmd_2', method:'title', args:[]}
6. Worker executes: this.page.title()
7. Worker → WS: {type:'commandResult', commandId:'cmd_2', result: 'Example Domain'}
8. Frontend promise resolves with 'Example Domain'
9. Script completes, returns result
```

### Benefits Achieved

✅ **No eval restrictions** - Code executes in browser where AsyncFunction is allowed
✅ **Worker stays secure** - Only executes predefined Playwright methods
✅ **Full Playwright API** - Easy to add new methods to proxy
✅ **Error handling** - Errors propagate correctly to frontend
✅ **Timeout protection** - 30s per command prevents hanging

### Deployment Status

✅ Worker deployed: https://remote-browser.vana.workers.dev
✅ Version: c896a8ce-8c59-4217-b102-53b7e1b26c53
✅ Ready for frontend deployment

### Next Steps

1. Deploy frontend to Vercel
2. Test end-to-end with real scripts
3. Add more Playwright methods to proxy as needed
4. Consider adding script examples/templates


## Fix: waitUntil Error

**Error:** `Cannot read properties of null (reading 'waitUntil')`

**Root Cause:** 
In `worker/src/session.ts` line 42, we had:
```typescript
this.runScript(code).catch(e => this.handleError(e));
```

This created a dangling promise in the Durable Object context. Durable Objects require proper promise handling.

**Fix:**
```typescript
await this.runScript(code);
```

Now the promise is properly awaited before returning the response.

**Deployed:** Version cbf580d7-de03-4178-813d-c788cbbf4832

