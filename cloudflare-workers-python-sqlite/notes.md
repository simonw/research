# Cloudflare Workers Investigation Notes

## Goal
Explore Cloudflare Workers development:
1. JavaScript hello world
2. Form with server-side processing
3. SQLite page counter with persistence
4. Python workers via WASM using Starlette
5. Python form handling
6. Python SQLite page counter

## Progress

### 2026-01-26 - Starting investigation

### JavaScript Workers

#### JavaScript Hello World - SUCCESS
- Created `js-worker/` with wrangler.toml and src/index.js
- Started with `wrangler dev --local --port 8787`
- curl test result: "Hello World from Cloudflare Workers!"

#### JavaScript Form Page - SUCCESS
- Added `/form` route with GET (displays form) and POST (processes form)
- curl POST test with name=Claude&message=Testing+forms! returned processed HTML

#### JavaScript SQLite Page Counter - SUCCESS
- Added `/counter` route using D1 (Cloudflare's SQLite)
- Used `--persist-to .wrangler/state` for local persistence
- Counter increments on each visit and persists across restarts
- Fixed: Use `prepare().run()` instead of `exec()` for D1 statements

### JavaScript Worker Complete
All three features working:
- `/` or `/hello` - Hello World
- `/form` - Form with server-side processing
- `/counter` - SQLite page counter with persistence

---

### D1 Persistence Details

#### Persistence Folder Structure
The `--persist-to .wrangler/state` flag stores D1 data at:
```
.wrangler/state/v3/d1/miniflare-D1DatabaseObject/<hash>.sqlite
```

The SQLite database file contains:
- `page_views` table - our counter data (page='counter', count=5)
- `_cf_METADATA` table - internal Cloudflare metadata

File details: SQLite 3.x database (16384 bytes, 4 pages)

#### _cf_METADATA Table Details
Schema: key INTEGER (primary key), value BLOB
Contents: (2, 14) - internal Cloudflare bookkeeping

---

### Python Workers Investigation

#### How Python Workers Work
From Cloudflare docs: Python Workers use **Pyodide** (CPython compiled to WebAssembly).
- Local dev (`pywrangler dev`) uses the **actual Workers runtime** with Pyodide
- NOT a simulation - runs in V8 isolate with Pyodide injected
- At deployment, Cloudflare takes a memory snapshot after importing packages
- Snapshots are stored with your Worker for faster cold starts

#### Network/Proxy Issues

Python Worker local development was failing with DNS/connection errors.

**Problem 1: DNS resolution failure**
```
*** Fatal uncaught kj::Exception: DNS lookup failed.; params.host = pyodide-capnp-bin.edgeworker.net
```

**Solution**: Used DNS-over-HTTPS to resolve hostname:
```bash
curl -s "https://cloudflare-dns.com/dns-query?name=pyodide-capnp-bin.edgeworker.net&type=A" \
     -H "accept: application/dns-json"
```
Response: `{"Answer":[{"data":"104.17.72.104"},{"data":"104.17.73.104"}]}`

Added to /etc/hosts:
```
104.17.72.104 pyodide-capnp-bin.edgeworker.net
```

**Problem 2: TCP connection timeout**
After DNS was resolved, workerd still couldn't connect because:
- workerd makes direct TCP connections (not via HTTP proxy)
- Environment has HTTP_PROXY set but outbound TCP is blocked
- Connection times out: `connect(): Connection timed out`

#### Attempted Hack: Modify miniflare to add cache directories
Modified `/opt/node22/lib/node_modules/wrangler/node_modules/miniflare/dist/src/index.js`
Line ~58923 in `getRuntimeArgs()` function to add:
```javascript
"--pyodide-bundle-disk-cache-dir=/tmp/pyodide-cache",
"--pyodide-package-disk-cache-dir=/tmp/pyodide-cache",
```

This tells workerd to use a local cache directory, which would help on subsequent runs
but still needs initial download from pyodide-capnp-bin.edgeworker.net.

#### Attempted Hack: TCP tunnel through HTTP proxy
Created Python script `/tmp/http_connect_tunnel.py` that:
1. Listens on local port 443
2. Uses HTTP CONNECT method to establish tunnel through proxy
3. Forwards TCP traffic through the tunnel

Updated /etc/hosts to point `pyodide-capnp-bin.edgeworker.net` to `127.0.0.1`

**Status: In Progress** - Testing if workerd can use the tunnel.

---

### Datasette-lite Analysis
The datasette-lite repo shows how to run Datasette in Pyodide (browser-based):
- Uses Web Workers (browser) not Cloudflare Workers
- Loads Pyodide from jsdelivr CDN
- Uses micropip to install datasette and deps at runtime
- Can load SQLite databases from URLs or use in-memory
- Uses Datasette's internal HTTPX client for request handling

Key code pattern from webworker.js:
```javascript
self.pyodide = await loadPyodide({
    indexURL: "https://cdn.jsdelivr.net/pyodide/v0.27.2/full/",
    fullStdLib: true
});
await pyodide.loadPackage('micropip');
await micropip.install("datasette")
```
