#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "starlette",
#     "uvicorn",
#     "httpx",
# ]
# ///
"""
WebMCP Demo Server - A Starlette app that serves pages with WebMCP tools.

Run with:
    uv run server.py

This serves an HTML page that registers WebMCP tools via navigator.modelContext.
An AI agent (or our CDP script) can then discover and call these tools.
"""

from starlette.applications import Starlette
from starlette.responses import HTMLResponse, JSONResponse
from starlette.routing import Route
import uvicorn
import json
from datetime import datetime

# In-memory data store for our demo
NOTES = []
COUNTER = {"value": 0}


async def homepage(request):
    """Serve the main page with WebMCP tools registered."""
    return HTMLResponse(INDEX_HTML)


async def api_notes(request):
    """REST API for notes - used by the WebMCP tool handlers."""
    global NOTES
    if request.method == "GET":
        return JSONResponse({"notes": NOTES})
    elif request.method == "POST":
        data = await request.json()
        note = {
            "id": len(NOTES) + 1,
            "text": data.get("text", ""),
            "created": datetime.now().isoformat(),
        }
        NOTES.append(note)
        return JSONResponse({"note": note})
    elif request.method == "DELETE":
        data = await request.json()
        note_id = data.get("id")
        NOTES = [n for n in NOTES if n["id"] != note_id]
        return JSONResponse({"ok": True})


async def api_counter(request):
    """REST API for counter."""
    if request.method == "GET":
        return JSONResponse(COUNTER)
    elif request.method == "POST":
        data = await request.json()
        action = data.get("action", "increment")
        if action == "set":
            COUNTER["value"] = data.get("value", 0)
        elif action == "increment":
            COUNTER["value"] += data.get("amount", 1)
        elif action == "decrement":
            COUNTER["value"] -= data.get("amount", 1)
        elif action == "reset":
            COUNTER["value"] = 0
        return JSONResponse(COUNTER)


app = Starlette(
    routes=[
        Route("/", homepage),
        Route("/api/notes", api_notes, methods=["GET", "POST", "DELETE"]),
        Route("/api/counter", api_counter, methods=["GET", "POST"]),
    ]
)


INDEX_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>WebMCP Demo</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            font-family: system-ui, -apple-system, sans-serif;
            max-width: 800px; margin: 40px auto; padding: 0 20px;
            background: #0f172a; color: #e2e8f0;
        }
        h1 { color: #38bdf8; margin-bottom: 8px; }
        h2 { color: #7dd3fc; margin: 24px 0 12px; }
        .subtitle { color: #94a3b8; margin-bottom: 24px; }
        .card {
            background: #1e293b; border-radius: 12px; padding: 20px;
            margin-bottom: 16px; border: 1px solid #334155;
        }
        .tool-name { color: #34d399; font-family: monospace; font-weight: bold; }
        .tool-desc { color: #94a3b8; margin-top: 4px; }
        #counter-display {
            font-size: 3em; text-align: center; color: #f59e0b;
            padding: 20px; font-weight: bold;
        }
        #notes-list { list-style: none; }
        #notes-list li {
            padding: 8px 12px; margin: 4px 0; background: #334155;
            border-radius: 6px; display: flex; justify-content: space-between;
        }
        .note-text { color: #e2e8f0; }
        .note-time { color: #64748b; font-size: 0.85em; }
        #status {
            position: fixed; bottom: 20px; right: 20px;
            padding: 8px 16px; border-radius: 20px; font-size: 0.85em;
        }
        .status-ok { background: #065f46; color: #6ee7b7; }
        .status-err { background: #7f1d1d; color: #fca5a5; }
        #log {
            background: #0f172a; border: 1px solid #334155; border-radius: 8px;
            padding: 12px; font-family: monospace; font-size: 0.85em;
            max-height: 200px; overflow-y: auto; color: #94a3b8;
        }
        #log .entry { margin: 2px 0; }
        #log .call { color: #38bdf8; }
        #log .result { color: #34d399; }
        #log .error { color: #f87171; }
    </style>
</head>
<body>
    <h1>WebMCP Demo</h1>
    <p class="subtitle">This page registers tools via <code>navigator.modelContext</code> that AI agents can discover and call.</p>

    <h2>Counter</h2>
    <div class="card">
        <div id="counter-display">0</div>
    </div>

    <h2>Notes</h2>
    <div class="card">
        <ul id="notes-list">
            <li><span class="note-text" style="color: #64748b;">No notes yet. An AI agent can add some!</span></li>
        </ul>
    </div>

    <h2>Registered WebMCP Tools</h2>
    <div id="tools-list"></div>

    <h2>Tool Call Log</h2>
    <div id="log"></div>

    <div id="status"></div>

    <script>
        const API_BASE = window.location.origin;

        // ============================================================
        // Window-level tool registry for CDP discovery and invocation.
        // The native navigator.modelContext API registers tools for
        // the browser's built-in AI agent, but has no external
        // list/call methods. This registry mirrors it for CDP access.
        // ============================================================
        window.__webmcp_tools = {};  // name -> {name, description, inputSchema, execute}

        // UI helpers
        function updateCounterDisplay(value) {
            document.getElementById('counter-display').textContent = value;
        }

        function updateNotesList(notes) {
            const list = document.getElementById('notes-list');
            if (notes.length === 0) {
                list.innerHTML = '<li><span class="note-text" style="color: #64748b;">No notes yet.</span></li>';
                return;
            }
            list.innerHTML = notes.map(n =>
                `<li>
                    <span class="note-text">${escapeHtml(n.text)}</span>
                    <span class="note-time">#${n.id} - ${new Date(n.created).toLocaleTimeString()}</span>
                </li>`
            ).join('');
        }

        function escapeHtml(text) {
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }

        function logEntry(type, message) {
            const log = document.getElementById('log');
            const entry = document.createElement('div');
            entry.className = `entry ${type}`;
            entry.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
            log.appendChild(entry);
            log.scrollTop = log.scrollHeight;
        }

        function setStatus(ok, text) {
            const el = document.getElementById('status');
            el.textContent = text;
            el.className = ok ? 'status-ok' : 'status-err';
        }

        // Define our WebMCP tools
        const TOOLS = [
            {
                name: "get_counter",
                description: "Get the current counter value",
                inputSchema: { type: "object", properties: {} },
                annotations: { readOnlyHint: true },
                async execute() {
                    const resp = await fetch(`${API_BASE}/api/counter`);
                    const data = await resp.json();
                    logEntry('call', `get_counter -> ${data.value}`);
                    return { content: [{ type: "text", text: JSON.stringify(data) }] };
                }
            },
            {
                name: "set_counter",
                description: "Set the counter to a specific value",
                inputSchema: {
                    type: "object",
                    properties: {
                        value: { type: "number", description: "The value to set the counter to" }
                    },
                    required: ["value"]
                },
                async execute(args) {
                    const resp = await fetch(`${API_BASE}/api/counter`, {
                        method: 'POST',
                        headers: {'Content-Type': 'application/json'},
                        body: JSON.stringify({ action: 'set', value: args.value })
                    });
                    const data = await resp.json();
                    updateCounterDisplay(data.value);
                    logEntry('call', `set_counter(${args.value}) -> ${data.value}`);
                    return { content: [{ type: "text", text: JSON.stringify(data) }] };
                }
            },
            {
                name: "increment_counter",
                description: "Increment the counter by a given amount (default 1)",
                inputSchema: {
                    type: "object",
                    properties: {
                        amount: { type: "number", description: "Amount to increment by", default: 1 }
                    }
                },
                async execute(args) {
                    const resp = await fetch(`${API_BASE}/api/counter`, {
                        method: 'POST',
                        headers: {'Content-Type': 'application/json'},
                        body: JSON.stringify({ action: 'increment', amount: args.amount || 1 })
                    });
                    const data = await resp.json();
                    updateCounterDisplay(data.value);
                    logEntry('call', `increment_counter(${args.amount || 1}) -> ${data.value}`);
                    return { content: [{ type: "text", text: JSON.stringify(data) }] };
                }
            },
            {
                name: "add_note",
                description: "Add a new note with the given text",
                inputSchema: {
                    type: "object",
                    properties: {
                        text: { type: "string", description: "The note text to add" }
                    },
                    required: ["text"]
                },
                async execute(args) {
                    const resp = await fetch(`${API_BASE}/api/notes`, {
                        method: 'POST',
                        headers: {'Content-Type': 'application/json'},
                        body: JSON.stringify({ text: args.text })
                    });
                    const data = await resp.json();
                    const allResp = await fetch(`${API_BASE}/api/notes`);
                    const allData = await allResp.json();
                    updateNotesList(allData.notes);
                    logEntry('call', `add_note("${args.text}") -> id=${data.note.id}`);
                    return { content: [{ type: "text", text: JSON.stringify(data) }] };
                }
            },
            {
                name: "list_notes",
                description: "List all notes",
                inputSchema: { type: "object", properties: {} },
                annotations: { readOnlyHint: true },
                async execute() {
                    const resp = await fetch(`${API_BASE}/api/notes`);
                    const data = await resp.json();
                    updateNotesList(data.notes);
                    logEntry('call', `list_notes -> ${data.notes.length} notes`);
                    return { content: [{ type: "text", text: JSON.stringify(data) }] };
                }
            },
            {
                name: "delete_note",
                description: "Delete a note by its ID",
                inputSchema: {
                    type: "object",
                    properties: {
                        id: { type: "number", description: "The ID of the note to delete" }
                    },
                    required: ["id"]
                },
                async execute(args) {
                    await fetch(`${API_BASE}/api/notes`, {
                        method: 'DELETE',
                        headers: {'Content-Type': 'application/json'},
                        body: JSON.stringify({ id: args.id })
                    });
                    const allResp = await fetch(`${API_BASE}/api/notes`);
                    const allData = await allResp.json();
                    updateNotesList(allData.notes);
                    logEntry('call', `delete_note(${args.id})`);
                    return { content: [{ type: "text", text: `Deleted note ${args.id}` }] };
                }
            },
            {
                name: "get_page_info",
                description: "Get information about this demo page including current state",
                inputSchema: { type: "object", properties: {} },
                annotations: { readOnlyHint: true },
                async execute() {
                    const counterResp = await fetch(`${API_BASE}/api/counter`);
                    const counter = await counterResp.json();
                    const notesResp = await fetch(`${API_BASE}/api/notes`);
                    const notes = await notesResp.json();
                    const info = {
                        title: document.title,
                        url: window.location.href,
                        counter_value: counter.value,
                        notes_count: notes.notes.length,
                        tools_registered: Object.keys(window.__webmcp_tools).length,
                        timestamp: new Date().toISOString()
                    };
                    logEntry('call', `get_page_info -> counter=${counter.value}, notes=${notes.notes.length}`);
                    return { content: [{ type: "text", text: JSON.stringify(info, null, 2) }] };
                }
            }
        ];

        // Register all tools
        function registerTools() {
            const toolsList = document.getElementById('tools-list');
            toolsList.innerHTML = '';

            const hasNativeAPI = (typeof navigator.modelContext !== 'undefined');

            for (const tool of TOOLS) {
                // Always store in our window-level registry for CDP access
                window.__webmcp_tools[tool.name] = tool;

                // Also register with native API if available
                if (hasNativeAPI) {
                    try {
                        navigator.modelContext.registerTool(tool);
                    } catch (e) {
                        logEntry('error', `Native registerTool failed for ${tool.name}: ${e.message}`);
                        console.error(`Native registerTool failed for ${tool.name}:`, e);
                    }
                }

                logEntry('result', `Registered: ${tool.name}` + (hasNativeAPI ? ' (native + registry)' : ' (registry only)'));

                // Add to UI
                const card = document.createElement('div');
                card.className = 'card';
                card.innerHTML = `
                    <div class="tool-name">${tool.name}</div>
                    <div class="tool-desc">${tool.description}</div>
                `;
                toolsList.appendChild(card);
            }

            setStatus(true, `${TOOLS.length} tools registered` + (hasNativeAPI ? ' (native WebMCP)' : ' (registry only)'));
            logEntry('result', `All ${TOOLS.length} tools registered. Native API: ${hasNativeAPI}`);
        }

        // Initialize
        registerTools();

        // Load initial state
        fetch(`${API_BASE}/api/counter`).then(r => r.json()).then(d => updateCounterDisplay(d.value));
        fetch(`${API_BASE}/api/notes`).then(r => r.json()).then(d => updateNotesList(d.notes));
    </script>
</body>
</html>
"""


if __name__ == "__main__":
    print("Starting WebMCP demo server on http://localhost:8000")
    print("Open this URL in Chrome 146+ with the WebMCP flag enabled")
    uvicorn.run(app, host="0.0.0.0", port=8000)
