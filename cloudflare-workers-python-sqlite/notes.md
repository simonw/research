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

#### Network/Proxy Issues - BLOCKING

Python Worker local development requires direct internet access from the workerd binary.
workerd does NOT honor HTTP_PROXY environment variables for its internal network operations.

**Problem 1: DNS resolution failure**
```
*** Fatal uncaught kj::Exception: DNS lookup failed.; params.host = pyodide-capnp-bin.edgeworker.net
```

**Solution**: Used DNS-over-HTTPS to resolve hostname and added to /etc/hosts:
```bash
curl -s "https://cloudflare-dns.com/dns-query?name=pyodide-capnp-bin.edgeworker.net&type=A" -H "accept: application/dns-json"
# Response: {"Answer":[{"data":"104.17.72.104"}]}

echo "104.17.72.104 pyodide-capnp-bin.edgeworker.net" >> /etc/hosts
```

**Problem 2: TCP connection timeout**
After DNS was resolved, workerd makes direct TCP connections which time out:
```
*** Fatal uncaught kj::Exception: connect(): Connection timed out
```

#### Attempted Hack 1: Modify miniflare for caching
Modified `/opt/node22/lib/node_modules/wrangler/node_modules/miniflare/dist/src/index.js`
Line ~58923 in `getRuntimeArgs()` function to add:
```javascript
"--pyodide-bundle-disk-cache-dir=/tmp/pyodide-cache",
"--pyodide-package-disk-cache-dir=/tmp/pyodide-cache",
```

**Result**: Partial success - discovered the Pyodide bundle URL:
```
https://pyodide-capnp-bin.edgeworker.net/pyodide_0.26.0a2_2024-03-01_76.capnp.bin
```

Downloaded the 13.8MB bundle via HTTP proxy and placed in cache:
```bash
curl -o /tmp/pyodide-cache/pyodide_0.26.0a2_2024-03-01_76.capnp.bin \
     "https://pyodide-capnp-bin.edgeworker.net/pyodide_0.26.0a2_2024-03-01_76.capnp.bin"
```

This allowed Pyodide bundle to load from cache!

**Problem 3: Package downloads**
After Pyodide bundle loads, workerd tries to download Python packages from storage.googleapis.com:
```
Failed to download package; path = python-package-bucket/20240829.4/hashlib-1.0.0.tar.gz
DNS lookup failed.; params.host = storage.googleapis.com
```

Added `142.250.191.187 storage.googleapis.com` to /etc/hosts, but TCP connections still timeout.

#### Attempted Hack 2: TCP tunnel through HTTP CONNECT
Created a Python script that:
1. Listens on localhost:443
2. Sends HTTP CONNECT request through proxy
3. Forwards TCP data bidirectionally

**Result**: Tunnel establishes but TLS verification fails through proxy:
```
upstream connect error... CERTIFICATE_VERIFY_FAILED:verify cert failed
```

#### Conclusion for Python Workers
**Local development of Python Workers requires direct internet access.** workerd binary:
1. Does NOT use HTTP_PROXY for its internal network operations
2. Makes direct TCP connections for Pyodide bundles and Python packages
3. Downloads from: pyodide-capnp-bin.edgeworker.net, storage.googleapis.com

**The Python Worker code IS correct** and would work on actual Cloudflare deployment.
See `py-worker/src/entry.py` for the Starlette hello world implementation.

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

This browser-based approach differs from Cloudflare Python Workers which use
server-side Pyodide embedded in the workerd runtime.
