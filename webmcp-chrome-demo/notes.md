# WebMCP + Chrome DevTools Protocol Investigation Notes

## 2026-02-22

### Goal
Research WebMCP, download a Chrome build that supports it, build a Python demo app, and experiment with controlling Chrome over CDP (Chrome DevTools Protocol).

### What is WebMCP?

WebMCP (Web Model Context Protocol) is a W3C Community Group draft standard that adds a `navigator.modelContext` API to browsers. It allows websites to register "tools" — JavaScript functions with structured schemas — that AI agents can discover and invoke, replacing fragile screen-scraping approaches.

- **Co-authored by**: Google (Chrome team) and Microsoft (Edge team)
- **Standardization**: W3C Web Machine Learning Community Group
- **Spec**: https://webmachinelearning.github.io/webmcp/
- **Status**: Draft, not on W3C Standards Track yet
- **First implementation**: Chrome 146 Canary (Feb 2026), behind feature flag

### Key distinction: WebMCP vs Anthropic's MCP
- Anthropic's MCP: Backend protocol, JSON-RPC, connects AI to hosted services
- WebMCP: Client-side browser API, no JSON-RPC, connects AI to browser-based interfaces
- They are complementary, not competing

### Native API Surface (Chrome 147)

Discovered by probing via CDP `Runtime.evaluate`:

```
interface ModelContext {
    clearContext(): void;
    provideContext(options?: {tools: ModelContextTool[]}): void;
    registerTool(tool: ModelContextTool): void;
    unregisterTool(name: string): void;
};

dictionary ModelContextTool {
    required DOMString name;
    required DOMString description;
    object inputSchema;        // JSON Schema
    required ToolExecuteCallback execute;
    ToolAnnotations annotations;
};
```

### Key findings from experimentation

1. **`navigator.modelContext` is available on ALL pages** when the flag is enabled — even `about:blank`. The `ModelContext` constructor is on the prototype chain.

2. **No external tool discovery API**: The native API has `registerTool` and `unregisterTool` but NO `listTools()` or `callTool()`. Tools are meant to be discovered by the browser's built-in AI agent (like Gemini), not by external code via CDP.

3. **Duplicate tool names are rejected**: Calling `registerTool` with a name that's already registered throws: `"Failed to execute 'registerTool' on 'ModelContext': Duplicate tool name"`

4. **`unregisterTool` with invalid name throws**: `"Failed to execute 'unregisterTool' on 'ModelContext': Invalid tool name"` — meaning Chrome validates that the tool exists.

5. **`provideContext()` replaces all tools**: Unlike `registerTool` which adds incrementally, `provideContext({tools: [...]})` replaces the entire tool set.

6. **`clearContext()` removes everything**: After calling `clearContext()`, you can `registerTool` fresh — no state leaks.

7. **Annotations supported**: `{ readOnlyHint: true }` accepted without error on registration.

8. **Declarative API**: `<form toolname="..." tooldescription="...">` works at the HTML level — Chrome recognizes these attributes. However, there's no observable way to confirm the browser actually exposes these as tools to its AI agent from the outside.

### How to bridge CDP → WebMCP tools

Since the native API has no `listTools`/`callTool`, I used a window-level registry approach:

1. Page stores tools in `window.__webmcp_tools = {}` (name → tool object)
2. Page ALSO registers them with `navigator.modelContext.registerTool()` for the native browser AI
3. CDP client uses `Runtime.evaluate` to:
   - Read `window.__webmcp_tools` for tool discovery
   - Call `tool.execute(args)` for invocation

This is essentially what the `@mcp-b/global` polyfill does — it creates a `__mcpBridge` that external MCP servers can query. The polyfill didn't load in headless mode (CDN fetch from unpkg.com failed), but the native API worked fine.

### Chrome version used
- `Google Chrome 147.0.7695.0 dev` (downloaded as `google-chrome-unstable` deb package)
- Flag: `--enable-features=WebMCPTesting`
- Also available via `chrome://flags/#enable-webmcp-testing` in interactive mode

### CDP interaction pattern

```python
# Launch Chrome with remote debugging
google-chrome-unstable --headless=new --no-sandbox --remote-debugging-port=9222 --enable-features=WebMCPTesting

# Connect via websocket
ws_url = requests.get("http://localhost:9222/json")[0]["webSocketDebuggerUrl"]
ws = websockets.connect(ws_url)

# Enable domains
send(ws, "Runtime.enable")
send(ws, "Page.enable")

# Navigate
send(ws, "Page.navigate", {"url": "http://localhost:8000"})

# Evaluate JS to interact with WebMCP tools
send(ws, "Runtime.evaluate", {
    "expression": "window.__webmcp_tools['set_counter'].execute({value: 42})",
    "awaitPromise": True,
    "returnByValue": True
})
```

### The @mcp-b/global polyfill ecosystem

The `@mcp-b/global` npm package (from WebMCP-org) provides:
- A polyfill for `navigator.modelContext` on browsers without native support
- A `__mcpBridge` that external MCP servers can query for tool discovery
- Dual transport: TabServerTransport (extensions) + IframeChildTransport (iframes)
- CDN: `https://unpkg.com/@mcp-b/global@latest/dist/index.iife.js`

When native API is detected, it syncs tools between native and polyfill layers.

### Chrome DevTools MCP server

`@mcp-b/chrome-devtools-mcp` is a fork of Google's official `chrome-devtools-mcp` that adds:
- `list_webmcp_tools` — discovers tools via `@mcp-b/global`'s bridge
- `call_webmcp_tool` — invokes tools
- 26+ standard CDP tools (navigate, click, fill, screenshot, evaluate, etc.)

This is how Claude Code, Cursor, etc. can control Chrome + interact with WebMCP tools.

### Things I tried that didn't work or had issues

1. **CDN polyfill in headless mode**: `@mcp-b/global` from unpkg.com didn't load in headless Chrome — likely DNS/network restrictions in the sandbox. Solution: don't depend on it, use native API + custom registry.

2. **Accessing tools externally via native API only**: Not possible without a window-level bridge. The `ModelContext` interface intentionally lacks introspection methods — it's a registration-only API from the page's perspective.

3. **Arrow characters in HTML**: Used `→` in the first version of the HTML which worked fine, then switched to `->` for simplicity.

### Performance note from research
- WebMCP tool calls: 20-100 tokens (JSON responses)
- Screenshot-based approaches: 2,000+ tokens per screenshot
- Claimed 89% token reduction for simple tasks, 77% for complex multi-step tasks
