# Browser-Based Single File Bundler

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

## Research Summary

This project explores whether Vite can run as WebAssembly entirely in the browser, and provides proof-of-concept tools for bundling HTML pages into single self-contained files client-side.

## Key Findings

### Can Vite Run in the Browser?

**No, Vite cannot run directly in the browser.** Vite is built for Node.js and has heavy dependencies on:
- File system APIs (`fs`, `path`)
- Child processes for development server
- Native bindings (esbuild native binary)

**However**, there are three ways to achieve browser-based bundling:

### What CAN Run in the Browser?

1. **esbuild-wasm** - The WebAssembly version of esbuild (which Vite uses internally)
   - Available as npm package: `esbuild-wasm`
   - ~10MB WASM binary
   - ~10x slower than native (single-threaded in WASM)
   - Needs custom plugins to fetch files via HTTP

2. **@rollup/browser** - Rollup's browser build
   - Uses WASM for the bundler
   - Needs in-memory filesystem (memfs)

3. **StackBlitz WebContainers** - Full Node.js runtime in browser
   - **CAN run real Vite with vite-plugin-singlefile!**
   - Requires Cross-Origin Isolation headers
   - Free for open-source use

### How vite-plugin-singlefile Works

The plugin inlines assets during Vite's build phase:
1. Finds `<script src="...">` and `<link rel="stylesheet" href="...">` tags
2. Reads the referenced files
3. Replaces external references with inline `<script>` and `<style>` tags
4. Configures Vite/Rollup to inline all assets

## Proof of Concept Implementations

This repository includes **three** browser-based bundler implementations:

### 1. Simple Bundler (`index-simple.html`)
- **No external dependencies** - Pure JavaScript
- Fetches HTML page via CORS
- Inlines CSS (including @import statements)
- Inlines JavaScript
- Converts images to data URLs
- Works for traditional multi-file pages
- **Limitation**: Does not handle ES module bundling

### 2. esbuild-wasm Bundler (`index.html`)
- Uses **esbuild-wasm** for ES module bundling
- HTTP fetch plugin for module resolution
- Heavier (~10MB WASM download)
- More capable but slower to initialize

### 3. WebContainer Bundler (`index-webcontainer.html`)
- Runs **real Vite with vite-plugin-singlefile** in the browser
- Uses StackBlitz WebContainers (full Node.js in browser)
- Requires Cross-Origin Isolation headers
- Most capable - identical to running Vite locally
- Slower initial boot (downloads npm packages)

## Usage

### Running Locally

```bash
# Install dependencies
npm install

# Start the standard server (simple and esbuild versions)
npm start

# Start the Cross-Origin Isolated server (for WebContainer version)
npm run start:coi

# Open in browser:
# http://localhost:3000/index-simple.html    (lightweight, no deps)
# http://localhost:3000/index.html           (esbuild-wasm)
# http://localhost:3000/index-webcontainer.html (real Vite via WebContainers)
```

### Using the Bundler

1. Enter the URL of a CORS-enabled HTML page
2. Click "Bundle to Single File" (or "Bundle with Vite" for WebContainer version)
3. Wait for processing
4. Copy the output or download the bundled HTML file

### Testing

```bash
# Run manual tests
npm run test:manual

# Run Playwright tests for simple bundler
npm test

# Run all Playwright tests
npm run test:all
```

## Project Structure

```
vite-wasm-browser-compiler/
├── index.html              # esbuild-wasm bundler
├── index-simple.html       # Simple bundler (no external deps)
├── index-webcontainer.html # WebContainer + real Vite bundler
├── server.js               # Development server with CORS
├── server-coi.js           # Server with Cross-Origin Isolation headers
├── package.json
├── playwright.config.js
├── test-manual.js          # Node.js test script
├── test-pages/
│   └── simple-app/         # Test application
│       ├── index.html
│       ├── styles.css
│       └── main.js
├── tests/
│   ├── bundler.spec.js            # Playwright tests for esbuild version
│   ├── bundler-simple.spec.js     # Playwright tests for simple version
│   └── bundler-webcontainer.spec.js # Playwright tests for WebContainer
├── notes.md                # Research notes
└── README.md               # This file
```

## Test Results

Playwright tests verify core functionality:
- ✓ Page loads correctly
- ✓ Bundles simple HTML page with CSS and JS
- ✓ Copy button works
- ✓ Bundled output is self-contained and works
- ✓ Shows warning when no URL is entered

## Limitations

1. **CORS Required**: Target pages must allow cross-origin requests
2. **ES Modules**: Simple bundler doesn't handle ES module imports
3. **Performance**: WASM-based bundling is ~10x slower than native
4. **Memory**: Large pages with many resources consume significant browser memory
5. **WebContainers**: Requires Cross-Origin Isolation headers on the server

## Technical Details

### How the Simple Bundler Works

```javascript
// 1. Fetch HTML
const html = await fetch(url);
const doc = new DOMParser().parseFromString(html, 'text/html');

// 2. Find and inline CSS
const links = doc.querySelectorAll('link[rel="stylesheet"]');
for (const link of links) {
    const css = await fetch(resolveUrl(baseUrl, link.href));
    const style = doc.createElement('style');
    style.textContent = css;
    link.replaceWith(style);
}

// 3. Find and inline JavaScript
const scripts = doc.querySelectorAll('script[src]');
for (const script of scripts) {
    const js = await fetch(resolveUrl(baseUrl, script.src));
    script.textContent = js;
    script.removeAttribute('src');
}

// 4. Output bundled HTML
const bundled = doc.documentElement.outerHTML;
```

### esbuild-wasm Integration

```javascript
import * as esbuild from 'esbuild-wasm';

await esbuild.initialize({
    wasmURL: 'https://esm.sh/esbuild-wasm@0.24.0/esbuild.wasm',
    worker: true
});

const result = await esbuild.build({
    stdin: { contents: code, loader: 'js' },
    bundle: true,
    minify: true,
    format: 'iife',
    write: false,
    plugins: [{
        name: 'http-fetch',
        setup(build) {
            build.onLoad({ filter: /.*/ }, async (args) => {
                const contents = await fetch(args.path).then(r => r.text());
                return { contents, loader: 'js' };
            });
        }
    }]
});
```

### WebContainer Integration

```javascript
import { WebContainer } from '@webcontainer/api';

// Boot WebContainer (requires Cross-Origin Isolation)
const webcontainerInstance = await WebContainer.boot();

// Mount project files
await webcontainerInstance.mount({
    'package.json': { file: { contents: '{"dependencies": {"vite": "^5.0.0", "vite-plugin-singlefile": "^2.0.0"}}' }},
    'vite.config.js': { file: { contents: 'import { viteSingleFile } from "vite-plugin-singlefile"; export default { plugins: [viteSingleFile()] }' }},
    'index.html': { file: { contents: htmlContent }}
});

// Install dependencies and build
await webcontainerInstance.spawn('npm', ['install']);
await webcontainerInstance.spawn('npm', ['run', 'build']);

// Read the bundled output
const output = await webcontainerInstance.fs.readFile('/dist/index.html', 'utf-8');
```

## Sources

- [Vite Features - WASM Support](https://vite.dev/guide/features)
- [vite-plugin-singlefile on npm](https://www.npmjs.com/package/vite-plugin-singlefile)
- [vite-plugin-singlefile on GitHub](https://github.com/richardtallent/vite-plugin-singlefile)
- [esbuild-wasm on npm](https://www.npmjs.com/package/esbuild-wasm)
- [esbuild API Documentation](https://esbuild.github.io/api/)
- [Running ESBuild in the Browser](https://schof.co/running-esbuild-in-the-browser/)
- [@rollup/browser on npm](https://www.npmjs.com/package/@rollup/browser)
- [Rollup Browser Documentation](https://rollupjs.org/browser/)
- [StackBlitz WebContainers](https://webcontainers.io/)
- [WebContainer API Documentation](https://developer.stackblitz.com/platform/api/webcontainer-api)

## Conclusion

While Vite itself cannot run directly in the browser due to Node.js dependencies, there are three viable approaches for browser-based bundling:

1. **Simple inlining** - Fast, lightweight, no dependencies, but limited to traditional pages
2. **esbuild-wasm** - Handles ES modules, but large WASM download and slower
3. **WebContainers** - Runs real Vite, full compatibility, but requires special headers

The WebContainer approach is the most capable, allowing real Vite with vite-plugin-singlefile to run entirely in the browser. This proves that full-featured browser-based bundling is possible today.
