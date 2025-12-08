# Datasette Lite NPM Package Investigation Notes

## Date: 2025-12-08

## Overview

Investigating the datasette-lite repository to turn it into an NPM package that can be self-hosted.

## Repository Structure Analysis

The datasette-lite repo contains:

- `index.html` - Main HTML page with:
  - Loading indicator (SVG spinner + textarea for logs)
  - JavaScript that creates a Web Worker and communicates with it
  - Event handlers for navigation (popstate, click on links, form submission)
  - Analytics integration (Plausible) tied to lite.datasette.io

- `webworker.js` - The core logic:
  - Loads Pyodide (Python in WebAssembly) from CDN
  - Installs datasette and dependencies via micropip
  - Downloads SQLite databases, CSV, JSON, Parquet files from URLs
  - Runs Datasette as a server within the web worker
  - Handles requests and returns HTML/JSON responses

- `app.css` - Datasette styling (CSS reset + UI components)

## Key Dependencies (loaded from CDN)

1. **Pyodide v0.27.2** - Python runtime compiled to WebAssembly
   - Loaded from: `https://cdn.jsdelivr.net/pyodide/v0.27.2/full/pyodide.js`

2. **Datasette** - Installed at runtime via micropip

## Self-Hosting Considerations

### What needs to be modified for self-hosting:

1. **Analytics removal/configuration** - The Plausible analytics is hardcoded to lite.datasette.io
2. **Base URL handling** - Currently assumes root deployment
3. **CORS headers** - Required for loading external data files

### Package Design Decisions

1. Create a simple package that bundles the static files
2. Remove analytics by default (or make it configurable)
3. Provide a simple HTTP server for testing
4. Document CORS requirements for production deployment

## Implementation Steps

1. Create NPM package structure with package.json
2. Copy and modify the source files:
   - Remove hardcoded analytics
   - Inline CSS into HTML (optional, for single-file deployment)
3. Add a build script
4. Test local installation
5. Document usage

## Testing Notes

### Test Results

1. **Server Test** - Started the bundled HTTP server on port 8765
   - HTTP 200 for `/` (index.html)
   - HTTP 200 for `/app.css` with correct `text/css` content type
   - HTTP 200 for `/webworker.js` with correct `application/javascript` content type

2. **npm pack Test** - Successfully created `datasette-lite-1.0.0.tgz`
   - Package size: 13.3 KB
   - Unpacked size: 45.0 KB
   - Contains 7 files

3. **npm install Test** - Successfully installed from tarball
   - `npm install ../datasette-lite-npm/datasette-lite-1.0.0.tgz` completed
   - Package installed correctly to `node_modules/datasette-lite/`

4. **npx Test** - Successfully ran `npx datasette-lite`
   - Server started correctly on specified port
   - Files served with correct CORS and content headers

5. **Build Script Test** - Successfully recreated package from scratch
   - Cloned upstream repo
   - Built package with all modifications
   - npm pack worked on rebuilt package

## Key Learnings

1. **Pyodide loads from CDN** - The Pyodide WebAssembly runtime is loaded directly from jsDelivr CDN. This means the self-hosted version still requires internet access for first load (Pyodide is ~10MB).

2. **Datasette installed at runtime** - The actual Datasette Python package is installed via micropip when the page loads. This adds to initial load time but ensures the latest version.

3. **No server-side processing needed** - Everything runs client-side. The only server requirement is serving static files with proper CORS headers.

4. **Analytics easily removed** - The Plausible analytics was domain-specific and easily stripped for self-hosting.
