#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "websockets",
#     "httpx",
# ]
# ///
"""
Explore Chrome's native WebMCP API via CDP.

This script digs deeper into what the native navigator.modelContext API
exposes in Chrome 147 with the WebMCPTesting flag, explores CDP capabilities
for browser automation, and demonstrates the declarative WebMCP form API.

Run with:
    uv run explore_cdp.py

Make sure server.py is running first.
"""

import asyncio
import base64
import json
import subprocess
import time

import httpx
import websockets


CHROME_BIN = "google-chrome-unstable"
CDP_PORT = 9223  # Different port to avoid conflicts
DEMO_URL = "http://localhost:8000"


async def send_cdp(ws, method: str, params: dict | None = None, timeout: float = 10.0) -> dict:
    msg_id = int(time.time() * 1000) % 1_000_000
    msg = {"id": msg_id, "method": method, "params": params or {}}
    await ws.send(json.dumps(msg))
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp_text = await asyncio.wait_for(ws.recv(), timeout=deadline - time.time())
            resp = json.loads(resp_text)
            if resp.get("id") == msg_id:
                return resp
        except asyncio.TimeoutError:
            break
    raise TimeoutError(f"CDP command {method} timed out")


async def js(ws, expression: str, await_promise: bool = True) -> any:
    """Shorthand: evaluate JS, return just the value."""
    params = {"expression": expression, "returnByValue": True, "awaitPromise": await_promise}
    resp = await send_cdp(ws, "Runtime.evaluate", params)
    result = resp.get("result", {}).get("result", {})
    if "value" in result:
        return result["value"]
    if result.get("subtype") == "error":
        return f"ERROR: {result.get('description', 'unknown')}"
    return result


async def screenshot(ws, path: str):
    resp = await send_cdp(ws, "Page.captureScreenshot", {"format": "png"})
    img_data = base64.b64decode(resp["result"]["data"])
    with open(path, "wb") as f:
        f.write(img_data)
    return len(img_data)


async def run_exploration():
    print("=" * 70)
    print("Deep Exploration: Chrome WebMCP Native API + CDP Automation")
    print("=" * 70)

    # Launch Chrome
    proc = subprocess.Popen(
        [CHROME_BIN, "--headless=new", "--no-sandbox", "--disable-gpu",
         f"--remote-debugging-port={CDP_PORT}", "--enable-features=WebMCPTesting",
         "--window-size=1280,900", "about:blank"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )

    try:
        # Wait for CDP
        async with httpx.AsyncClient() as client:
            for _ in range(30):
                try:
                    resp = await client.get(f"http://localhost:{CDP_PORT}/json/version")
                    version = resp.json()
                    break
                except Exception:
                    await asyncio.sleep(0.3)

        print(f"\nChrome {version['Browser']}")

        # Get page WS
        async with httpx.AsyncClient() as client:
            resp = await client.get(f"http://localhost:{CDP_PORT}/json")
            targets = resp.json()
            ws_url = next(t["webSocketDebuggerUrl"] for t in targets if t["type"] == "page")

        async with websockets.connect(ws_url, max_size=10_000_000) as ws:
            await send_cdp(ws, "Runtime.enable")
            await send_cdp(ws, "Page.enable")
            await send_cdp(ws, "DOM.enable")
            await send_cdp(ws, "Network.enable")

            # ========================================
            # Part 1: Explore native modelContext API
            # ========================================
            print("\n" + "=" * 70)
            print("PART 1: Native navigator.modelContext API Exploration")
            print("=" * 70)

            # Navigate to a blank page first to check native API without our app
            await send_cdp(ws, "Page.navigate", {"url": "about:blank"})
            await asyncio.sleep(1)

            mc_exists = await js(ws, "typeof navigator.modelContext", await_promise=False)
            print(f"\n  navigator.modelContext type on about:blank: {mc_exists}")

            # Enumerate the full API surface
            api_surface = await js(ws, """
            (function() {
                const mc = navigator.modelContext;
                if (!mc) return 'not available';
                const info = { type: typeof mc, constructor: mc.constructor?.name };
                // Own properties
                info.ownProps = Object.getOwnPropertyNames(mc);
                // Prototype methods
                const proto = Object.getPrototypeOf(mc);
                info.protoName = proto?.constructor?.name;
                info.protoMethods = Object.getOwnPropertyNames(proto).filter(k => k !== 'constructor');
                // Symbol properties
                info.symbols = Object.getOwnPropertySymbols(mc).map(s => s.toString());
                return info;
            })()
            """, await_promise=False)
            print(f"  API surface: {json.dumps(api_surface, indent=4)}")

            # Try registering a tool on about:blank (won't work - not secure context? or will it?)
            reg_result = await js(ws, """
            (function() {
                try {
                    navigator.modelContext.registerTool({
                        name: 'test_tool',
                        description: 'A test tool',
                        inputSchema: { type: 'object', properties: {} },
                        execute: async () => ({ content: [{ type: 'text', text: 'hello' }] })
                    });
                    return 'SUCCESS: tool registered on about:blank';
                } catch(e) {
                    return 'FAILED: ' + e.message;
                }
            })()
            """, await_promise=False)
            print(f"  Register tool on about:blank: {reg_result}")

            # ========================================
            # Part 2: Navigate to our demo app
            # ========================================
            print("\n" + "=" * 70)
            print("PART 2: WebMCP Tools on Demo App")
            print("=" * 70)

            await send_cdp(ws, "Page.navigate", {"url": DEMO_URL})
            await asyncio.sleep(3)

            title = await js(ws, "document.title", await_promise=False)
            print(f"\n  Page: {title}")

            # Check tools registered in native API
            native_check = await js(ws, """
            (function() {
                const mc = navigator.modelContext;
                // The native API doesn't expose a listTools method.
                // Let's see what state it holds internally.
                const info = {
                    type: typeof mc,
                    hasRegisterTool: typeof mc.registerTool === 'function',
                    hasUnregisterTool: typeof mc.unregisterTool === 'function',
                    hasProvideContext: typeof mc.provideContext === 'function',
                    hasClearContext: typeof mc.clearContext === 'function',
                };
                // Try to call registerTool with a duplicate name to confirm tools are stored
                try {
                    mc.registerTool({
                        name: 'get_counter',
                        description: 'duplicate test',
                        inputSchema: { type: 'object', properties: {} },
                        execute: async () => ({})
                    });
                    info.duplicateResult = 'allowed (no error)';
                } catch(e) {
                    info.duplicateResult = 'rejected: ' + e.message;
                }
                return info;
            })()
            """, await_promise=False)
            print(f"  Native API state: {json.dumps(native_check, indent=4)}")

            # Confirm tools in our registry
            tool_count = await js(ws, "Object.keys(window.__webmcp_tools).length", await_promise=False)
            tool_names = await js(ws, "Object.keys(window.__webmcp_tools)", await_promise=False)
            print(f"  Window registry: {tool_count} tools: {tool_names}")

            # ========================================
            # Part 3: Declarative API test
            # ========================================
            print("\n" + "=" * 70)
            print("PART 3: Declarative WebMCP API (HTML form with toolname)")
            print("=" * 70)

            # Inject a declarative WebMCP form into the page
            inject_result = await js(ws, """
            (function() {
                const form = document.createElement('form');
                form.setAttribute('toolname', 'search_demo');
                form.setAttribute('tooldescription', 'Search the demo for items');
                form.innerHTML = `
                    <input name="query" type="text" required placeholder="Search query">
                    <input name="limit" type="number" value="10">
                    <button type="submit">Search</button>
                `;
                document.body.appendChild(form);

                // Check if the browser recognizes the toolname attribute
                const allForms = document.querySelectorAll('form[toolname]');
                return {
                    formInjected: true,
                    formsWithToolname: allForms.length,
                    toolname: form.getAttribute('toolname'),
                    tooldescription: form.getAttribute('tooldescription')
                };
            })()
            """, await_promise=False)
            print(f"  Declarative form: {json.dumps(inject_result, indent=4)}")

            # Check if Chrome exposes declarative tools to modelContext
            declarative_check = await js(ws, """
            (function() {
                // Try unregistering and re-registering to see if declarative tools auto-appear
                const forms = document.querySelectorAll('form[toolname]');
                const results = [];
                for (const f of forms) {
                    results.push({
                        toolname: f.getAttribute('toolname'),
                        tooldescription: f.getAttribute('tooldescription'),
                        inputs: Array.from(f.querySelectorAll('input')).map(i => ({
                            name: i.name, type: i.type, required: i.required
                        }))
                    });
                }
                return results;
            })()
            """, await_promise=False)
            print(f"  Declarative forms found: {json.dumps(declarative_check, indent=4)}")

            # ========================================
            # Part 4: CDP automation capabilities
            # ========================================
            print("\n" + "=" * 70)
            print("PART 4: CDP Automation Capabilities")
            print("=" * 70)

            # Get document structure
            doc = await send_cdp(ws, "DOM.getDocument", {"depth": 2})
            root_node = doc["result"]["root"]
            print(f"\n  DOM root: {root_node['nodeName']} (children: {root_node.get('childNodeCount', 0)})")

            # Get page metrics
            metrics = await send_cdp(ws, "Performance.getMetrics")
            if "result" in metrics:
                interesting = ["JSHeapUsedSize", "JSHeapTotalSize", "Nodes", "Documents",
                               "Frames", "JSEventListeners"]
                for m in metrics["result"]["metrics"]:
                    if m["name"] in interesting:
                        val = m["value"]
                        if "Heap" in m["name"]:
                            val = f"{val / 1024 / 1024:.1f} MB"
                        print(f"    {m['name']}: {val}")

            # Network info
            cookies = await send_cdp(ws, "Network.getCookies")
            print(f"  Cookies: {len(cookies['result']['cookies'])}")

            # Console messages (collect any that occurred)
            console_msgs = await js(ws, """
            (function() {
                // Can't access console history from JS, but we can check our log
                return document.getElementById('log').innerText;
            })()
            """, await_promise=False)
            print(f"\n  Page activity log:\n    " + (console_msgs or "(empty)").replace("\n", "\n    "))

            # ========================================
            # Part 5: Interactive tool calling session
            # ========================================
            print("\n" + "=" * 70)
            print("PART 5: Interactive Tool Calling Session")
            print("=" * 70)

            async def call_tool(name, args=None):
                args_json = json.dumps(args or {})
                result = await js(ws, f"""
                (async function() {{
                    const tool = window.__webmcp_tools["{name}"];
                    if (!tool) return {{ error: "not found" }};
                    return await tool.execute({args_json});
                }})()
                """)
                return result

            # Simulate an AI agent workflow
            print("\n  Simulating AI agent workflow...")

            # Step 1: Get page info to understand context
            info = await call_tool("get_page_info")
            info_text = json.loads(info["content"][0]["text"])
            print(f"  1. Page state: counter={info_text['counter_value']}, notes={info_text['notes_count']}")

            # Step 2: Reset counter
            await call_tool("set_counter", {"value": 0})
            print("  2. Reset counter to 0")

            # Step 3: Add several notes in sequence
            topics = [
                "WebMCP is a W3C draft standard for browser-AI interaction",
                "Chrome 146+ supports navigator.modelContext natively",
                "Tools are registered with name, description, inputSchema, execute",
                "CDP (Chrome DevTools Protocol) enables remote browser control",
                "The declarative API uses HTML form attributes (toolname, tooldescription)",
            ]
            for i, topic in enumerate(topics):
                await call_tool("add_note", {"text": topic})
                await call_tool("increment_counter", {"amount": 1})
                print(f"  3.{i+1}. Added note #{i+1} and incremented counter")

            # Step 4: Read back all notes
            notes_result = await call_tool("list_notes")
            notes_data = json.loads(notes_result["content"][0]["text"])
            print(f"  4. Retrieved {len(notes_data['notes'])} notes:")
            for n in notes_data["notes"]:
                print(f"     [{n['id']}] {n['text'][:60]}...")

            # Step 5: Delete a note
            await call_tool("delete_note", {"id": 3})
            print("  5. Deleted note #3")

            # Step 6: Final state
            final_info = await call_tool("get_page_info")
            final = json.loads(final_info["content"][0]["text"])
            print(f"  6. Final state: counter={final['counter_value']}, notes={final['notes_count']}")

            # Take final screenshot
            sz = await screenshot(ws, "/home/user/research/webmcp-chrome-demo/screenshot_explore.png")
            print(f"\n  Screenshot saved ({sz} bytes)")

            # ========================================
            # Part 6: Probe native API edge cases
            # ========================================
            print("\n" + "=" * 70)
            print("PART 6: Native API Edge Cases")
            print("=" * 70)

            # Test unregisterTool
            unreg = await js(ws, """
            (function() {
                try {
                    navigator.modelContext.unregisterTool('nonexistent_tool');
                    return 'no error for unregistering nonexistent tool';
                } catch(e) {
                    return 'error: ' + e.message;
                }
            })()
            """, await_promise=False)
            print(f"  unregisterTool('nonexistent'): {unreg}")

            # Test provideContext
            provide = await js(ws, """
            (function() {
                try {
                    navigator.modelContext.provideContext({
                        tools: [{
                            name: 'provided_tool',
                            description: 'Tool added via provideContext',
                            inputSchema: { type: 'object', properties: { x: { type: 'number' } } },
                            execute: async (args) => ({
                                content: [{ type: 'text', text: 'x=' + (args.x || 0) }]
                            })
                        }]
                    });
                    return 'provideContext succeeded';
                } catch(e) {
                    return 'error: ' + e.message;
                }
            })()
            """, await_promise=False)
            print(f"  provideContext(): {provide}")

            # Test clearContext
            clear = await js(ws, """
            (function() {
                try {
                    navigator.modelContext.clearContext();
                    return 'clearContext succeeded';
                } catch(e) {
                    return 'error: ' + e.message;
                }
            })()
            """, await_promise=False)
            print(f"  clearContext(): {clear}")

            # Can we re-register after clear?
            rereg = await js(ws, """
            (function() {
                try {
                    navigator.modelContext.registerTool({
                        name: 'after_clear',
                        description: 'Registered after clearContext',
                        inputSchema: { type: 'object', properties: {} },
                        execute: async () => ({ content: [{ type: 'text', text: 'alive!' }] })
                    });
                    return 'SUCCESS: can register after clear';
                } catch(e) {
                    return 'error: ' + e.message;
                }
            })()
            """, await_promise=False)
            print(f"  registerTool after clear: {rereg}")

            # Test what annotations are supported
            annot_test = await js(ws, """
            (function() {
                try {
                    navigator.modelContext.registerTool({
                        name: 'annotated_tool',
                        description: 'Tool with annotations',
                        inputSchema: { type: 'object', properties: {} },
                        annotations: { readOnlyHint: true },
                        execute: async () => ({ content: [{ type: 'text', text: 'ok' }] })
                    });
                    return 'tool with annotations registered OK';
                } catch(e) {
                    return 'error: ' + e.message;
                }
            })()
            """, await_promise=False)
            print(f"  Tool with annotations: {annot_test}")

            print("\n" + "=" * 70)
            print("Exploration complete!")
            print("=" * 70)

    finally:
        proc.terminate()
        proc.wait()
        print("\nChrome terminated.")


if __name__ == "__main__":
    asyncio.run(run_exploration())
