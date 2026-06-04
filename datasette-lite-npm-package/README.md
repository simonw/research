# Datasette Lite NPM Package Investigation

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This report documents the process of converting [datasette-lite](https://github.com/simonw/datasette-lite) into a self-hostable NPM package.

## Summary

Successfully created an NPM package that allows anyone to:
1. Install Datasette Lite via `npm install`
2. Run a local server with `npx datasette-lite`
3. Deploy the static files to any hosting service

## What is Datasette Lite?

Datasette Lite is a fully client-side implementation of [Datasette](https://datasette.io/), running entirely in the browser using:

- **[Pyodide](https://pyodide.org/)** - A port of Python to WebAssembly
- **Web Workers** - To run Python without blocking the main thread
- **Datasette** - A Python tool for exploring and publishing data

When you load Datasette Lite, your browser:
1. Downloads the Pyodide WebAssembly runtime (~10MB from CDN)
2. Installs Datasette and its dependencies via `micropip`
3. Downloads your data files (SQLite, CSV, JSON, Parquet)
4. Runs Datasette entirely client-side, with no server needed

## Package Structure

```
datasette-lite/
├── package.json       # NPM package metadata
├── index.js           # Node.js exports (distPath, files)
├── README.md          # Documentation
├── bin/
│   └── serve.js       # CLI server for local testing
└── dist/
    ├── index.html     # Main HTML page (analytics removed)
    ├── app.css        # Datasette styling
    └── webworker.js   # Web Worker with Pyodide/Datasette
```

## Changes from Original

1. **Removed Plausible analytics** - The original loads Plausible analytics tied to `lite.datasette.io`. The NPM package removes this for self-hosting.

2. **Added CLI server** - A simple Node.js HTTP server (`bin/serve.js`) for local testing with proper CORS headers.

3. **Added Node.js exports** - `index.js` exports paths for programmatic access to the static files.

## How to Use

### Quick Start

```bash
# Install the package
npm install datasette-lite

# Start local server
npx datasette-lite
# or specify a port
npx datasette-lite 3000
```

### Deploy Static Files

Copy the `dist/` folder to any static hosting:

```bash
cp -r node_modules/datasette-lite/dist/* ./public/
```

Works with: GitHub Pages, Netlify, Vercel, S3, nginx, Apache, etc.

### CORS Requirements

When loading external data files, they must be served with:
```
Access-Control-Allow-Origin: *
```

GitHub Pages and Gists include this header automatically.

## URL Parameters

- `?url=URL` - Load a SQLite database
- `?csv=URL` - Load CSV file(s)
- `?json=URL` - Load JSON file(s)
- `?parquet=URL` - Load Parquet file(s)
- `?sql=URL` - Execute SQL from URL
- `?memory=1` - Start with in-memory database only
- `?install=PACKAGE` - Install additional Python packages
- `?metadata=URL` - Load custom metadata
- `?ref=VERSION` - Use specific Datasette version

## Build Script

The included `build-package.sh` script can recreate the package from scratch:

```bash
./build-package.sh /path/to/output
```

This will:
1. Clone the upstream repository
2. Copy and modify the source files
3. Create all NPM package files
4. Output a ready-to-publish package

## Testing Results

| Test | Result |
|------|--------|
| HTTP server | HTTP 200 for all static files |
| Content types | Correct MIME types served |
| npm pack | 13.3 KB package, 45 KB unpacked |
| npm install | Successful installation |
| npx command | Server starts correctly |
| Build script | Recreates package successfully |

## Limitations

1. **Internet required for first load** - Pyodide (~10MB) loads from jsDelivr CDN. For fully offline use, you'd need to self-host Pyodide too.

2. **Load time** - Initial load takes 10-30 seconds to download Pyodide and install Datasette.

3. **Memory usage** - Running Python in WebAssembly uses significant browser memory.

## Files in This Investigation

- `notes.md` - Detailed investigation notes
- `README.md` - This report
- `build-package.sh` - Script to recreate the NPM package from scratch

## Credits

- [Datasette](https://datasette.io/) by Simon Willison
- [Datasette Lite](https://github.com/simonw/datasette-lite) by Simon Willison
- [Pyodide](https://pyodide.org/) - Python in the browser
