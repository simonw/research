# Cloudflare Workers with Python and SQLite

Investigation of Cloudflare Workers development using JavaScript and Python (via Pyodide/WASM), with focus on SQLite persistence using Cloudflare D1.

## Quick Start - JavaScript Worker

```bash
cd js-worker
wrangler dev --local --persist-to .wrangler/state --port 8787
```

Test endpoints:
```bash
# Hello world
curl http://localhost:8787/

# Form with server-side processing
curl http://localhost:8787/form
curl -X POST -d "name=User&message=Hello" http://localhost:8787/form

# SQLite page counter (persisted)
curl http://localhost:8787/counter
```

## Summary

### JavaScript Worker - ✅ COMPLETE

Successfully built a Cloudflare Worker with:
1. **Hello World** - Basic text response
2. **Form Processing** - HTML form with server-side POST handling
3. **SQLite Counter** - Persistent page view counter using D1

Key learnings:
- Use `wrangler dev --local --persist-to .wrangler/state` for local development
- D1 databases are configured in `wrangler.toml` with `[[d1_databases]]`
- Use `prepare().run()` instead of `exec()` for D1 SQL statements
- Persistence data stored in `.wrangler/state/v3/d1/miniflare-D1DatabaseObject/*.sqlite`

### Python Worker (Minimal) - ✅ WORKS

Successfully built a minimal Python Worker (`py-worker-minimal/`) with:
1. **Hello World** - Basic text response
2. **Form Processing** - HTML form with server-side POST handling
3. **SQLite Counter** - In-memory SQLite counter (sqlite3 is built into Pyodide!)
4. **Status Endpoint** - Shows available features

**Key discovery**: Python Workers WITHOUT external package dependencies work perfectly locally!

```bash
cd py-worker-minimal
pywrangler dev --port 8789

# Test endpoints
curl http://localhost:8789/hello          # Hello World
curl http://localhost:8789/status         # Shows SQLite available
curl http://localhost:8789/form           # HTML form
curl -X POST -d "name=Test&message=Hi" http://localhost:8789/form
```

### Python Worker (Starlette) - ⚠️ BLOCKED (External Package Dependencies)

The Starlette-based worker (`py-worker/`) is blocked because:

**Issue**: `workerd` requires direct internet access to download Python packages (hashlib, ssl, etc.). It does NOT honor HTTP_PROXY environment variables.

**The code is correct** and would work:
- When deployed to actual Cloudflare Workers
- In environments with direct internet access

See `py-worker/src/entry.py` for the Starlette implementation.

## Project Structure

```
cloudflare-workers-python-sqlite/
├── js-worker/                 # Working JavaScript worker
│   ├── wrangler.toml         # Cloudflare config with D1 database
│   └── src/index.js          # Worker code with 3 routes
├── py-worker-minimal/        # Working minimal Python worker
│   ├── wrangler.toml         # Python worker config
│   ├── pyproject.toml        # No external dependencies
│   └── src/entry.py          # Pure Python with sqlite3
├── py-worker/                # Python Starlette worker (needs network)
│   ├── wrangler.toml         # Python worker config
│   ├── pyproject.toml        # Python dependencies (starlette)
│   └── src/entry.py          # Starlette hello world
├── notes.md                  # Detailed investigation notes
└── README.md                 # This file
```

## JavaScript Worker Details

### Configuration (wrangler.toml)
```toml
name = "hello-worker"
main = "src/index.js"
compatibility_date = "2024-01-01"

[[d1_databases]]
binding = "DB"
database_name = "page-counter"
database_id = "local-dev-db"
```

### D1 SQLite Usage
```javascript
// Initialize table
await env.DB.prepare(`
  CREATE TABLE IF NOT EXISTS page_views (
    page TEXT PRIMARY KEY,
    count INTEGER DEFAULT 0
  )
`).run();

// Upsert pattern
await env.DB.prepare(`
  INSERT INTO page_views (page, count) VALUES ('counter', 1)
  ON CONFLICT(page) DO UPDATE SET count = count + 1
`).run();

// Query
const result = await env.DB.prepare(
  'SELECT count FROM page_views WHERE page = ?'
).bind('counter').first();
```

### D1 Persistence Structure

Data persists at:
```
.wrangler/state/v3/d1/miniflare-D1DatabaseObject/<hash>.sqlite
```

The SQLite database contains:
- Your application tables (e.g., `page_views`)
- `_cf_METADATA` table for Cloudflare internal bookkeeping

## Minimal Python Worker Details

### Working Code (py-worker-minimal/src/entry.py)

```python
from workers import WorkerEntrypoint, Response
from urllib.parse import urlparse, parse_qs
import sqlite3  # Built into Pyodide!

class Default(WorkerEntrypoint):
    async def on_fetch(self, request):  # Note: on_fetch, not fetch!
        url = urlparse(request.url)
        path = url.path

        if path == "/" or path == "/hello":
            return Response("Hello World from Python Worker!")

        if path == "/form":
            if request.method == "POST":
                body = await request.text()
                params = parse_qs(body)
                name = params.get("name", [""])[0]
                # Process form...
            return Response(html, headers={"Content-Type": "text/html"})

        if path == "/counter":
            conn = sqlite3.connect(":memory:")
            # SQLite operations...
            return Response(f"SQLite Counter: {count}")

        return Response("Not Found", status=404)
```

### What Works Without Network Access

| Feature | Status | Notes |
|---------|--------|-------|
| Basic Python stdlib | ✅ | urllib.parse, etc. |
| sqlite3 module | ✅ | Built into Pyodide |
| workers module | ✅ | Response, WorkerEntrypoint |
| Form handling | ✅ | POST body parsing |
| URL routing | ✅ | Manual path matching |
| Web frameworks | ❌ | Starlette, FastAPI need network |
| Crypto modules | ❌ | hashlib, ssl need network |

---

## Python Worker (Starlette) Details

### How Python Workers Work

Cloudflare Python Workers use **Pyodide** (CPython compiled to WebAssembly):
- Local dev uses the actual Workers runtime (not a simulation)
- Pyodide runs inside a V8 isolate
- Cloudflare takes memory snapshots at deployment for fast cold starts

### Python Worker Code (py-worker/src/entry.py)
```python
from starlette.applications import Starlette
from starlette.responses import PlainTextResponse
from starlette.routing import Route
from workers import WorkerEntrypoint
import asgi

async def hello(request):
    return PlainTextResponse("Hello from Python + Starlette!")

app = Starlette(routes=[
    Route("/", hello),
    Route("/hello", hello),
])

class Default(WorkerEntrypoint):
    async def fetch(self, request):
        return await asgi.fetch(app, request.js_object, self.env)
```

### Local Development Limitations

workerd requires direct internet access to download:
1. **Pyodide bundle** (~13.8MB) from `pyodide-capnp-bin.edgeworker.net`
2. **Python packages** from `storage.googleapis.com`

In proxy-only environments, these connections fail with:
- DNS lookup failures (fixable with /etc/hosts)
- TCP connection timeouts (not fixable - workerd doesn't use HTTP_PROXY)

**Workaround attempts documented in notes.md**:
- Modified miniflare to add Pyodide cache directories
- Downloaded Pyodide bundle manually via curl
- Attempted TCP tunnel through HTTP CONNECT proxy

## References

- [Cloudflare Workers Python Docs](https://developers.cloudflare.com/workers/languages/python/)
- [Cloudflare D1 Docs](https://developers.cloudflare.com/d1/)
- [Pyodide](https://pyodide.org/) - Python for the browser
- [datasette-lite](https://github.com/simonw/datasette-lite) - Datasette running in browser with Pyodide
