#!/usr/bin/env python3
"""Minimal MCP Streamable-HTTP client to exercise Litestream's built-in MCP server."""
import json, sys, urllib.request

URL = "http://localhost:9999/"
SESSION = None

def call(method, params=None, id=1):
    global SESSION
    body = json.dumps({"jsonrpc":"2.0","id":id,"method":method,"params":params or {}}).encode()
    headers = {"Content-Type":"application/json","Accept":"application/json, text/event-stream"}
    if SESSION:
        headers["Mcp-Session-Id"] = SESSION
    req = urllib.request.Request(URL, data=body, headers=headers, method="POST")
    with urllib.request.urlopen(req) as r:
        sid = r.headers.get("Mcp-Session-Id")
        if sid: SESSION = sid
        raw = r.read().decode()
    # SSE frames: lines starting with "data: "
    for line in raw.splitlines():
        if line.startswith("data: "):
            return json.loads(line[6:])
    return json.loads(raw) if raw.strip() else None

# Handshake
init = call("initialize", {"protocolVersion":"2024-11-05","capabilities":{},
            "clientInfo":{"name":"explore","version":"1"}}, id=1)
print("server:", init["result"]["serverInfo"])

tools = call("tools/list", id=2)["result"]["tools"]
print("\nTOOLS:")
for t in tools:
    print(f"  - {t['name']}: {t['description'][:70]}")

def tool(name, args=None, id=10):
    # Tools are exposed with a "litestream_" prefix on the server.
    res = call("tools/call", {"name":"litestream_"+name,"arguments":args or {}}, id=id)
    c = res.get("result",{}).get("content",[{}])
    return c[0].get("text","") if c else json.dumps(res)

S3URL = ("s3://litestream/app?endpoint=http://localhost:9000"
         "&region=us-east-1&force-path-style=true")

print("\n== version ==");   print(tool("version"))
print("\n== databases =="); print(tool("databases"))
print("\n== info ==");      print(tool("info"))
print("\n== status ==");    print(tool("status"))
print("\n== ltx (level all, S3 replica) ==")
print(tool("ltx", {"path": S3URL, "level": "all"}))
print("\n== restore via MCP (from MinIO S3 to /tmp/mcp-restore.db) ==")
print(tool("restore", {"path": S3URL, "o": "/tmp/mcp-restore.db"}))
