# Self-Hosting Datasette Lite Research Notes

## Goal
Research how to make Datasette Lite (https://lite.datasette.io/) fully self-hosted so it can run offline without any external network requests.

## Initial Setup
- Cloned datasette-lite repo to /tmp/datasette-lite
- Analyzed code structure and external dependencies

## Research Log

### Step 1: Repository Structure Analysis

The datasette-lite repo is simple:
- `index.html` - Main HTML page with UI and JavaScript
- `webworker.js` - Web worker that runs Pyodide and Datasette
- `app.css` - Styles

### Step 2: External Dependencies Identified

From static analysis of the code:

**1. Pyodide Core (cdn.jsdelivr.net)**
- `pyodide.js` - Main loader script
- `pyodide.asm.js` - JavaScript glue
- `pyodide.asm.wasm` - WebAssembly runtime (~10 MB)
- `python_stdlib.zip` - Python standard library
- `pyodide-lock.json` - Package lockfile

**2. Pyodide Built-in Packages (loadPackage)**
- `micropip` - Package installer
- `ssl` - SSL support
- `setuptools` - For pkg_resources

**3. PyPI Packages (micropip.install)**
- `h11==0.12.0`
- `httpx==0.23`
- `python-multipart==0.0.15`
- `typing-extensions>=4.12.2`
- `datasette` (version varies by ?ref= param)
- `sqlite-utils==3.28` (optional, for CSV/JSON)
- `fastparquet` (optional, for Parquet)

**4. Default Databases**
- `https://latest.datasette.io/fixtures.db`
- `https://datasette.io/content.db`

**5. Analytics (Optional)**
- `https://plausible.io/js/script.manual.js`

### Step 3: External Hosts to Eliminate

1. cdn.jsdelivr.net - Pyodide runtime and packages
2. files.pythonhosted.org - PyPI wheel downloads
3. pypi.org - Package metadata API
4. latest.datasette.io - Default database
5. datasette.io - Default database
6. plausible.io - Analytics (optional)

### Step 4: Pyodide Self-Hosting Research

From Pyodide documentation:
- Full distribution: 327 MB (pyodide-0.27.2.tar.bz2)
- Core only: 5.3 MB (pyodide-core-0.27.2.tar.bz2)
- Set `indexURL` parameter in loadPyodide() to local path
- Need proper CORS headers and WASM MIME type
- Can use `lockFileURL` to specify custom lock file

Key insight from GitHub issue #4600:
- Use `micropip.freeze()` to generate updated pyodide-lock.json
- Download all wheel files locally
- Adjust checksums in lock file if needed

### Step 5: micropip Offline Installation

Options for offline packages:
1. Include wheels in pyodide-lock.json with local URLs
2. Use `emfs:` prefix for wheels in Pyodide filesystem
3. Pre-download all wheels and host them locally
4. Use custom index_urls parameter (but needs network)

### Step 6: Size Estimates

- Pyodide core: ~5-10 MB
- Full Pyodide distribution: ~327 MB
- Datasette + dependencies: ~3-5 MB
- sqlite-utils: ~500 KB
- Total minimal: ~50-100 MB
- Total with full Pyodide: ~350 MB

### Step 7: Build Script Testing

Created and tested a build script that:
1. Downloads Pyodide core files from jsdelivr CDN
2. Downloads required Pyodide packages (micropip, ssl, setuptools, packaging)
3. Downloads PyPI wheels for datasette dependencies
4. Creates modified webworker.js with local paths
5. Removes analytics from index.html
6. Creates a serve.py with correct WASM MIME types

Test results:
- Bundle size without datasette: ~18 MB
- Datasette wheel itself: ~400 KB
- Full bundle with all dependencies: ~20-25 MB estimated

### Step 8: Key Challenges Identified

1. **micropip.install() calls need modification**
   - Current code uses package names: `micropip.install("datasette")`
   - Self-hosted needs wheel paths: `micropip.install("./wheels/datasette-0.65.2-py3-none-any.whl")`
   - Alternative: Use `emfs:` prefix for Pyodide filesystem

2. **Package resolution**
   - micropip normally resolves dependencies from PyPI
   - Self-hosted needs pre-downloaded dependency tree
   - Can use `deps=False` to skip dependency resolution

3. **Version pinning**
   - Current webworker.js uses specific versions (h11==0.12.0)
   - These may conflict with Pyodide's built-in packages
   - Need to check compatibility matrix

4. **Dynamic datasette version**
   - Supports `?ref=` parameter for specific versions
   - Self-hosted would need multiple versions bundled
   - Simplest: pin to single version

5. **Default databases**
   - fixtures.db and content.db are loaded from external URLs
   - Either remove defaults or include sample databases

### Step 9: Final Recommendations

The most practical approach for self-hosting:

1. **Use minimal bundle** (~20-25 MB) rather than full Pyodide (350 MB)
2. **Pin specific versions** of all packages to avoid resolution issues
3. **Pre-download complete dependency tree** for all required packages
4. **Modify webworker.js** to use relative paths for all resources
5. **Remove analytics** and default database loading
6. **Include serve.py** script with correct WASM MIME types

### Files Created

- `analyze_dependencies.py` - Static analysis script
- `build_offline_bundle.py` - Build script prototype
- `dependency_analysis.json` - Detailed dependency report
- `README.md` - Comprehensive documentation

### Conclusion

Self-hosting Datasette Lite is feasible with a bundle size of ~20-25 MB. The main work involves:
1. Downloading Pyodide core + required packages
2. Pre-downloading all PyPI wheels with dependencies
3. Modifying webworker.js to use local paths
4. Serving with correct WASM MIME types

The build script prototype demonstrates the approach, though additional refinement is needed for production use, particularly around the micropip.install() calls and dependency ordering.
