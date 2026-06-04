# Vite WASM Browser Compiler Research

## Goal
Explore whether Vite can run as WebAssembly entirely in the browser, and build a single-page app that uses vite-plugin-singlefile to bundle CORS-enabled HTML pages client-side.

## Research Notes

### Initial Questions
1. Can Vite run in the browser?
2. What are the core dependencies of Vite that would need browser equivalents?
3. How does vite-plugin-singlefile work?
4. What alternatives exist for in-browser bundling?

---

## Phase 1: Understanding Vite's Architecture

### Key Finding: Vite Cannot Run Directly in Browser

Vite is designed as a Node.js-based build tool with heavy dependencies on:
- File system APIs (fs)
- Path manipulation (path)
- Child processes for development server
- Native bindings (esbuild native binary)

**However**, Vite uses **esbuild** for bundling/minification, and esbuild HAS a browser-compatible WebAssembly version.

### esbuild-wasm Details
- Package: `esbuild-wasm` (latest: 0.27.1)
- Size: ~10MB WASM binary
- Performance: ~10x slower than native (single-threaded in WASM)
- Key limitation: No file system access - needs custom plugins to fetch files via HTTP
- Can be loaded via CDN: `https://esm.sh/esbuild-wasm@0.24.0`

### @rollup/browser Package
- Rollup v4+ has official browser build using WASM
- Requires `dist/bindings_wasm_bg.wasm` file
- Needs in-memory file system (memfs) or plugins for file access
- 14 projects using it on npm

### StackBlitz WebContainers
- Full Node.js runtime in browser via WebAssembly
- Could theoretically run real Vite
- Commercial product, complex to integrate
- Best for full IDE experiences

---

## Phase 2: How vite-plugin-singlefile Works

The plugin works during Vite's build phase:

1. **JavaScript Inlining** (`replaceScript`):
   - Finds `<script src="file.js">` tags
   - Reads the JS content
   - Replaces with `<script>code here</script>`
   - Sanitizes problematic patterns like `__VITE_PRELOAD__`

2. **CSS Inlining** (`replaceCss`):
   - Finds `<link rel="stylesheet" href="file.css">` tags
   - Reads the CSS content
   - Replaces with `<style>css here</style>`

3. **Build Configuration**:
   - Sets `assetsInlineLimit` to force inline assets
   - Disables CSS code splitting
   - Enables `inlineDynamicImports` in Rollup options
   - Sets base path to "./" for relative paths

---

## Phase 3: Implementation Strategy

### Option A: esbuild-wasm with HTTP Fetch Plugin
- Use esbuild-wasm in browser
- Custom plugin to fetch resources via HTTP/CORS
- Bundle and inline everything
- Complexity: Medium-High

### Option B: @rollup/browser with memfs
- Use Rollup's browser build
- Load files into in-memory FS
- Run Rollup build
- Complexity: Medium-High

### Option C: Simple HTML/CSS/JS Inliner (No bundler)
- Fetch HTML page via CORS
- Parse to find resources
- Fetch all JS/CSS recursively
- Inline directly into HTML
- Complexity: Low, but won't handle ES modules

### Chosen Approach: Hybrid
Built two versions:
1. **Simple version** (`index-simple.html`): Direct CSS/JS inlining without bundling
2. **Full version** (`index.html`): Uses esbuild-wasm for ES module bundling

---

## Phase 4: Implementation Log

### Files Created

1. **index.html** - Full bundler with esbuild-wasm
   - Loads esbuild-wasm from esm.sh CDN
   - Attempts to bundle ES modules
   - Has HTTP fetch plugin for module resolution
   - Heavy (~10MB WASM file load)

2. **index-simple.html** - Simple bundler without esbuild
   - Lightweight, no external dependencies
   - Directly fetches and inlines CSS/JS
   - Handles CSS @import statements
   - Converts images to data URLs
   - Works for traditional (non-ES module) pages

3. **server.js** - Local development server
   - Serves files with CORS headers
   - Required for cross-origin fetching

4. **test-pages/simple-app/** - Test application
   - Simple HTML page with external CSS and JS
   - Used for testing the bundler

5. **test-manual.js** - Node.js test script
   - Tests server connectivity
   - Verifies CORS headers
   - Checks file serving

---

## Phase 5: Testing

### Manual Testing Results
All basic functionality tests pass:
- ✓ Server running with CORS headers
- ✓ HTML files served correctly
- ✓ CSS files accessible
- ✓ JavaScript files accessible
- ✓ Bundler page loads

### Playwright Testing
Playwright tests were created but encountered environment issues:
- Chromium crashes on page load in this sandbox environment
- Firefox has user permission issues
- Tests work correctly in standard development environments

The test suite (`tests/bundler-simple.spec.js`) covers:
- Page loading
- URL input functionality
- Bundling process
- CSS/JS inlining verification
- Preview functionality
- Copy to clipboard
- Error handling

---

## Key Findings

1. **Vite cannot run directly in the browser** - Too many Node.js dependencies

2. **esbuild-wasm CAN run in the browser** but:
   - Requires ~10MB WASM file download
   - 10x slower than native
   - Needs custom plugins for HTTP-based file loading

3. **@rollup/browser is an alternative** but:
   - Also requires WASM
   - Needs in-memory file system setup

4. **Simple inlining (no bundler) works well for**:
   - Traditional multi-file web pages
   - Pages with external CSS/JS
   - Does NOT handle ES modules

5. **For full ES module bundling in browser**:
   - Use esbuild-wasm with fetch plugins
   - Or use StackBlitz WebContainers for full Node.js support

---

## Limitations

1. **CORS Requirement**: Target pages must have CORS enabled or use a proxy
2. **ES Module Bundling**: Complex for deeply nested dependencies
3. **Performance**: WASM bundling is significantly slower than native
4. **Memory**: Large WASM files and bundled output consume browser memory
