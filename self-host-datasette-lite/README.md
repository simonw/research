# Self-Hosting Datasette Lite: Research Report

## Overview

[Datasette Lite](https://lite.datasette.io/) is a Pyodide-based web application that runs Datasette entirely in the browser using WebAssembly. This report documents how to create a fully self-hosted version that works offline without any external network requests.

## Current Architecture

Datasette Lite consists of three main files served from GitHub Pages:
- `index.html` - Main page with UI and JavaScript
- `webworker.js` - Web worker that loads Pyodide and runs Datasette
- `app.css` - Styles

When loaded, the application makes requests to multiple external hosts to fetch its dependencies.

## External Dependencies Identified

### 1. Pyodide Core (cdn.jsdelivr.net)

The application loads Pyodide v0.27.2 from the jsdelivr CDN:

| File | Purpose |
|------|---------|
| `pyodide.js` | Main JavaScript loader (~2.5 MB) |
| `pyodide.asm.js` | JavaScript glue code |
| `pyodide.asm.wasm` | WebAssembly runtime (~10 MB) |
| `python_stdlib.zip` | Python standard library (~5 MB) |
| `pyodide-lock.json` | Package lockfile (~500 KB) |

### 2. Pyodide Built-in Packages

Loaded via `pyodide.loadPackage()`:
- `micropip` - Python package installer for Pyodide
- `ssl` - SSL/TLS support
- `setuptools` - For `pkg_resources` support

### 3. PyPI Packages (files.pythonhosted.org, pypi.org)

Installed via `micropip.install()`:
- `h11==0.12.0` - HTTP/1.1 parser
- `httpx==0.23` - HTTP client
- `python-multipart==0.0.15` - Multipart form data parser
- `typing-extensions>=4.12.2` - Typing backports
- `datasette` - The main Datasette application

Optional (depending on URL parameters):
- `sqlite-utils==3.28` - For CSV/JSON import
- `fastparquet` - For Parquet file support

### 4. Default Databases (when no URL parameters)

- `https://latest.datasette.io/fixtures.db`
- `https://datasette.io/content.db`

### 5. Analytics (Optional)

- `https://plausible.io/js/script.manual.js`

## Self-Hosting Solution

### Approach 1: Minimal Bundle (~20-25 MB)

Download only the essential files:

1. **Pyodide Core Files**
   ```
   pyodide/
   ├── pyodide.js
   ├── pyodide.asm.js
   ├── pyodide.asm.wasm
   ├── python_stdlib.zip
   ├── pyodide-lock.json
   ├── micropip-*.whl
   ├── ssl-*.whl
   ├── setuptools-*.whl
   └── packaging-*.whl
   ```

2. **Pre-downloaded PyPI Wheels**
   ```
   wheels/
   ├── datasette-*.whl
   ├── httpx-*.whl
   ├── h11-*.whl
   ├── python_multipart-*.whl
   ├── typing_extensions-*.whl
   └── [all dependencies...]
   ```

3. **Modified Application Files**
   - `index.html` (analytics removed)
   - `webworker.js` (paths updated to local)
   - `app.css` (unchanged)

### Approach 2: Full Pyodide Bundle (~350 MB)

Include the complete Pyodide distribution for maximum offline capability:

```bash
# Download full distribution
wget https://github.com/pyodide/pyodide/releases/download/0.27.2/pyodide-0.27.2.tar.bz2
tar -xjf pyodide-0.27.2.tar.bz2
```

This includes 338 pre-built packages and allows installing additional packages from the local bundle.

## Required Code Modifications

### webworker.js

1. **Change `importScripts` to local path:**
   ```javascript
   // FROM:
   importScripts("https://cdn.jsdelivr.net/pyodide/v0.27.2/full/pyodide.js");
   // TO:
   importScripts("./pyodide/pyodide.js");
   ```

2. **Update `loadPyodide` indexURL:**
   ```javascript
   // FROM:
   indexURL: "https://cdn.jsdelivr.net/pyodide/v0.27.2/full/",
   // TO:
   indexURL: "./pyodide/",
   ```

3. **Modify `micropip.install` calls to use local wheels:**
   ```python
   # FROM:
   await micropip.install("h11==0.12.0")
   await micropip.install("datasette")

   # TO (Option A - relative URL):
   await micropip.install("./wheels/h11-0.12.0-py3-none-any.whl")
   await micropip.install("./wheels/datasette-0.65.2-py3-none-any.whl")

   # TO (Option B - emfs path, if wheels are in Pyodide filesystem):
   await micropip.install("emfs:/wheels/h11-0.12.0-py3-none-any.whl")
   ```

4. **Remove or replace default databases:**
   ```javascript
   // Either remove the defaults entirely, or include local copies
   // and update the URLs to local paths
   ```

### index.html

Remove analytics scripts:
```html
<!-- Remove these lines -->
<script defer data-domain="lite.datasette.io" src="https://plausible.io/js/script.manual.js"></script>
<script>window.plausible = window.plausible || function() { ... }</script>
```

## Server Requirements

The self-hosted bundle must be served with:

1. **Correct MIME type for WebAssembly:**
   ```
   .wasm → application/wasm
   ```

2. **CORS headers (if loading from different origins):**
   ```
   Access-Control-Allow-Origin: *
   ```

3. **Optional but recommended for SharedArrayBuffer:**
   ```
   Cross-Origin-Embedder-Policy: require-corp
   Cross-Origin-Opener-Policy: same-origin
   ```

Example Python server:
```python
import http.server
import mimetypes

mimetypes.add_type('application/wasm', '.wasm')

class CORSHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Access-Control-Allow-Origin', '*')
        super().end_headers()

http.server.test(HandlerClass=CORSHandler, port=8000)
```

## Implementation Challenges

### 1. Dependency Resolution

micropip normally fetches package metadata from PyPI to resolve dependencies. For offline use:
- Pre-download the complete dependency tree
- Use `deps=False` parameter to skip dependency resolution
- Install packages in correct order (dependencies first)

### 2. Version Conflicts

Pyodide 0.27.2 includes some packages that datasette-lite also installs:
- `httpx` (Pyodide: 0.28.1, datasette-lite installs: 0.23)
- `typing-extensions` (both available)

The self-hosted version should align versions to avoid conflicts.

### 3. Dynamic Version Selection

The `?ref=` URL parameter allows selecting Datasette versions. For self-hosted:
- Pin to a single Datasette version, OR
- Bundle multiple versions and update the JavaScript logic

### 4. Database Loading

Default databases are fetched from external URLs. Options:
- Remove default database loading (start with empty state)
- Include sample databases in the bundle
- Only support `?url=` parameter with local paths

## Estimated Bundle Sizes

| Component | Size |
|-----------|------|
| Pyodide core files | ~18 MB |
| Datasette + dependencies | ~2-3 MB |
| sqlite-utils (optional) | ~500 KB |
| fastparquet (optional) | ~5-10 MB |
| **Minimal total** | **~20-25 MB** |
| **Full Pyodide distribution** | **~350 MB** |

## Tools Provided

This research includes:

1. **`analyze_dependencies.py`** - Static analysis of all external dependencies
2. **`build_offline_bundle.py`** - Prototype script to create self-hosted bundle
3. **`dependency_analysis.json`** - Detailed dependency report

## Recommendations

For creating a distributable offline .zip:

1. **Use the minimal bundle approach** (~20-25 MB compressed)
2. **Pin Datasette to a specific version** to avoid complexity
3. **Remove default databases** - users will provide their own via `?url=` or local files
4. **Include a simple HTTP server script** for easy local serving
5. **Document MIME type requirements** clearly for other hosting solutions

## References

- [Pyodide Documentation: Downloading and Deploying](https://pyodide.org/en/stable/usage/downloading-and-deploying.html)
- [Pyodide GitHub Issue #4600: Hosting all pyodide files and packages](https://github.com/pyodide/pyodide/issues/4600)
- [micropip Documentation](https://micropip.pyodide.org/en/stable/project/usage.html)
- [PyScript Offline Usage Guide](https://docs.pyscript.net/2024.3.2/user-guide/offline/)
