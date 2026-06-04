#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "websockets",
#     "httpx",
# ]
# ///
"""
CDP Client for WebMCP - Controls Chrome over the DevTools Protocol.

This script:
1. Launches Chrome (headless) with the WebMCP flag and remote debugging
2. Navigates to our demo app
3. Discovers WebMCP tools registered via navigator.modelContext
4. Calls those tools programmatically via Runtime.evaluate over CDP

Run with:
    uv run cdp_client.py

Make sure server.py is running first:
    uv run server.py
"""

import asyncio
import json
import subprocess
import sys
import time

import httpx
import websockets


CHROME_BIN = "google-chrome-unstable"
CDP_PORT = 9222
DEMO_URL = "http://localhost:8000"


async def send_cdp(ws, method: str, params: dict | None = None, timeout: float = 10.0) -> dict:
    """Send a CDP command and wait for the response."""
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
    raise TimeoutError(f"CDP command {method} timed out after {timeout}s")


async def evaluate_js(ws, expression: str, await_promise: bool = True) -> dict:
    """Evaluate JavaScript in the page context and return the result."""
    params = {
        "expression": expression,
        "returnByValue": True,
        "awaitPromise": await_promise,
    }
    resp = await send_cdp(ws, "Runtime.evaluate", params)
    if "error" in resp:
        return {"error": resp["error"]}
    result = resp.get("result", {}).get("result", {})
    if result.get("type") == "undefined":
        return {"value": None}
    if "value" in result:
        return {"value": result["value"]}
    if result.get("subtype") == "error":
        return {"error": result.get("description", "Unknown error")}
    return result


def launch_chrome() -> subprocess.Popen:
    """Launch Chrome with WebMCP flag and remote debugging."""
    args = [
        CHROME_BIN,
        "--headless=new",
        "--no-sandbox",
        "--disable-gpu",
        f"--remote-debugging-port={CDP_PORT}",
        "--enable-features=WebMCPTesting",
        "--window-size=1280,720",
        "about:blank",
    ]
    print(f"Launching: {' '.join(args)}")
    proc = subprocess.Popen(
        args,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return proc


async def wait_for_cdp(max_wait: float = 10.0) -> dict:
    """Wait for CDP to become available and return version info."""
    deadline = time.time() + max_wait
    async with httpx.AsyncClient() as client:
        while time.time() < deadline:
            try:
                resp = await client.get(f"http://localhost:{CDP_PORT}/json/version")
                return resp.json()
            except Exception:
                await asyncio.sleep(0.3)
    raise RuntimeError("Chrome CDP did not become available")


async def get_page_ws_url() -> str:
    """Get the WebSocket URL for the first page target."""
    async with httpx.AsyncClient() as client:
        resp = await client.get(f"http://localhost:{CDP_PORT}/json")
        targets = resp.json()
        for t in targets:
            if t.get("type") == "page":
                return t["webSocketDebuggerUrl"]
    raise RuntimeError("No page target found")


async def discover_webmcp_tools(ws) -> list:
    """Discover WebMCP tools registered on the current page via window.__webmcp_tools."""
    js = """
    (function() {
        if (window.__webmcp_tools && typeof window.__webmcp_tools === 'object') {
            return Object.values(window.__webmcp_tools).map(t => ({
                name: t.name,
                description: t.description,
                inputSchema: t.inputSchema,
                annotations: t.annotations || {}
            }));
        }
        return [];
    })()
    """
    result = await evaluate_js(ws, js, await_promise=False)
    return result.get("value", [])


async def call_webmcp_tool(ws, tool_name: str, args: dict) -> dict:
    """Call a WebMCP tool by name with given arguments via window.__webmcp_tools."""
    args_json = json.dumps(args)
    js = f"""
    (async function() {{
        const tool = window.__webmcp_tools && window.__webmcp_tools["{tool_name}"];
        if (!tool || !tool.execute) {{
            return {{ success: false, error: "Tool '{tool_name}' not found in registry" }};
        }}
        try {{
            const result = await tool.execute({args_json});
            return {{ success: true, result: result }};
        }} catch(e) {{
            return {{ success: false, error: e.message }};
        }}
    }})()
    """
    result = await evaluate_js(ws, js, await_promise=True)
    return result.get("value", result)


async def run_demo():
    """Main demo flow: launch Chrome, navigate, discover and call WebMCP tools."""

    print("=" * 60)
    print("WebMCP + Chrome DevTools Protocol Demo")
    print("=" * 60)

    # Check if demo server is running
    try:
        async with httpx.AsyncClient() as client:
            resp = await client.get(DEMO_URL)
            if resp.status_code != 200:
                print(f"ERROR: Demo server at {DEMO_URL} returned {resp.status_code}")
                return
    except Exception:
        print(f"ERROR: Demo server not running at {DEMO_URL}")
        print("Start it first with: uv run server.py")
        return

    print(f"\n[1] Demo server is running at {DEMO_URL}")

    # Launch Chrome
    print("\n[2] Launching Chrome with WebMCP flag...")
    chrome_proc = launch_chrome()

    try:
        # Wait for CDP
        version = await wait_for_cdp()
        print(f"    Chrome {version['Browser']} ready")
        print(f"    CDP Protocol: {version['Protocol-Version']}")

        # Get page WebSocket URL
        ws_url = await get_page_ws_url()
        print(f"    WebSocket: {ws_url}")

        # Connect via WebSocket
        async with websockets.connect(ws_url, max_size=10_000_000) as ws:
            # Enable necessary CDP domains
            await send_cdp(ws, "Runtime.enable")
            await send_cdp(ws, "Page.enable")

            # Navigate to demo app
            print(f"\n[3] Navigating to {DEMO_URL}...")
            nav_result = await send_cdp(ws, "Page.navigate", {"url": DEMO_URL})

            # Wait for page load
            await asyncio.sleep(3)

            # Check page title
            title = await evaluate_js(ws, "document.title", await_promise=False)
            print(f"    Page title: {title.get('value', 'unknown')}")

            # Check if navigator.modelContext is available
            print("\n[4] Checking navigator.modelContext...")
            mc_check = await evaluate_js(
                ws,
                "typeof navigator.modelContext !== 'undefined'",
                await_promise=False,
            )
            has_model_context = mc_check.get("value", False)
            print(f"    navigator.modelContext available: {has_model_context}")

            if has_model_context:
                # Check what methods are available
                mc_methods = await evaluate_js(
                    ws,
                    """
                    (function() {
                        const mc = navigator.modelContext;
                        const methods = [];
                        for (const key in mc) {
                            methods.push(key + ': ' + typeof mc[key]);
                        }
                        // Also check prototype
                        const proto = Object.getPrototypeOf(mc);
                        if (proto) {
                            for (const key of Object.getOwnPropertyNames(proto)) {
                                if (key !== 'constructor') {
                                    methods.push(key + ': ' + typeof proto[key]);
                                }
                            }
                        }
                        return methods;
                    })()
                    """,
                    await_promise=False,
                )
                print(f"    modelContext interface: {mc_methods.get('value', [])}")

            # Check __mcpBridge
            bridge_check = await evaluate_js(
                ws,
                "typeof window.__mcpBridge !== 'undefined'",
                await_promise=False,
            )
            print(f"    __mcpBridge available: {bridge_check.get('value', False)}")

            # Discover tools
            print("\n[5] Discovering WebMCP tools...")
            tools = await discover_webmcp_tools(ws)

            if not tools:
                print("    No tools found, waiting a bit more for page init...")
                await asyncio.sleep(2)
                tools = await discover_webmcp_tools(ws)

            if tools:
                print(f"    Found {len(tools)} tools:")
                for t in tools:
                    name = t.get("name", "?") if isinstance(t, dict) else str(t)
                    desc = t.get("description", "") if isinstance(t, dict) else ""
                    print(f"      - {name}: {desc}")
            else:
                print("    No tools discovered (polyfill may not have loaded yet)")
                print("    Proceeding with direct API calls to demonstrate the concept...")

            # Call tools via WebMCP
            print("\n[6] Calling WebMCP tools via CDP...")

            # Get page info
            print("\n    --- get_page_info ---")
            result = await call_webmcp_tool(ws, "get_page_info", {})
            if isinstance(result, dict) and result.get("success"):
                content = result["result"].get("content", [])
                for c in content:
                    if c.get("type") == "text":
                        print(f"    {c['text']}")
            else:
                print(f"    Result: {json.dumps(result, indent=2)}")

            # Set counter
            print("\n    --- set_counter(42) ---")
            result = await call_webmcp_tool(ws, "set_counter", {"value": 42})
            print(f"    Result: {json.dumps(result, indent=2) if isinstance(result, dict) else result}")

            # Get counter
            print("\n    --- get_counter ---")
            result = await call_webmcp_tool(ws, "get_counter", {})
            print(f"    Result: {json.dumps(result, indent=2) if isinstance(result, dict) else result}")

            # Increment counter
            print("\n    --- increment_counter(10) ---")
            result = await call_webmcp_tool(ws, "increment_counter", {"amount": 10})
            print(f"    Result: {json.dumps(result, indent=2) if isinstance(result, dict) else result}")

            # Add some notes
            print("\n    --- add_note('Hello from CDP!') ---")
            result = await call_webmcp_tool(ws, "add_note", {"text": "Hello from CDP!"})
            print(f"    Result: {json.dumps(result, indent=2) if isinstance(result, dict) else result}")

            print("\n    --- add_note('WebMCP is working!') ---")
            result = await call_webmcp_tool(ws, "add_note", {"text": "WebMCP is working!"})
            print(f"    Result: {json.dumps(result, indent=2) if isinstance(result, dict) else result}")

            # List notes
            print("\n    --- list_notes ---")
            result = await call_webmcp_tool(ws, "list_notes", {})
            print(f"    Result: {json.dumps(result, indent=2) if isinstance(result, dict) else result}")

            # Get final page info
            print("\n    --- get_page_info (final state) ---")
            result = await call_webmcp_tool(ws, "get_page_info", {})
            if isinstance(result, dict) and result.get("success"):
                content = result["result"].get("content", [])
                for c in content:
                    if c.get("type") == "text":
                        info = json.loads(c["text"])
                        print(f"    Counter: {info['counter_value']}")
                        print(f"    Notes: {info['notes_count']}")
                        print(f"    Tools: {info['tools_registered']}")
            else:
                print(f"    Result: {json.dumps(result, indent=2) if isinstance(result, dict) else result}")

            # Take a screenshot
            print("\n[7] Taking screenshot via CDP...")
            screenshot_resp = await send_cdp(
                ws, "Page.captureScreenshot",
                {"format": "png", "quality": 80}
            )
            if "result" in screenshot_resp:
                import base64
                img_data = base64.b64decode(screenshot_resp["result"]["data"])
                screenshot_path = "/home/user/research/webmcp-chrome-demo/screenshot.png"
                with open(screenshot_path, "wb") as f:
                    f.write(img_data)
                print(f"    Saved to {screenshot_path} ({len(img_data)} bytes)")

            # Additional CDP exploration
            print("\n[8] Additional CDP exploration...")

            # Get console log output
            console_log = await evaluate_js(
                ws,
                "document.getElementById('log').innerText",
                await_promise=False,
            )
            print(f"    Page log:\n    {console_log.get('value', 'empty')}")

            # Check DOM state
            counter_val = await evaluate_js(
                ws,
                "document.getElementById('counter-display').textContent",
                await_promise=False,
            )
            print(f"\n    Counter display shows: {counter_val.get('value', '?')}")

            notes_html = await evaluate_js(
                ws,
                "document.getElementById('notes-list').children.length",
                await_promise=False,
            )
            print(f"    Notes list items: {notes_html.get('value', '?')}")

            print("\n" + "=" * 60)
            print("Demo complete!")
            print("=" * 60)

    finally:
        chrome_proc.terminate()
        chrome_proc.wait()
        print("\nChrome process terminated.")


if __name__ == "__main__":
    asyncio.run(run_demo())
