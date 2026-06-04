#!/bin/bash
# Build script to create the datasette-lite NPM package from scratch
# This script clones the upstream datasette-lite repository, modifies the files
# for self-hosting, and creates a publishable NPM package.

set -e

# Configuration
OUTPUT_DIR="${1:-./datasette-lite-npm}"
UPSTREAM_REPO="https://github.com/simonw/datasette-lite"
TEMP_DIR=$(mktemp -d)

echo "Building datasette-lite NPM package..."
echo "Output directory: $OUTPUT_DIR"
echo "Temp directory: $TEMP_DIR"

# Clone the upstream repository
echo "Cloning upstream repository..."
git clone --depth 1 "$UPSTREAM_REPO" "$TEMP_DIR/datasette-lite"

# Create package structure
echo "Creating package structure..."
mkdir -p "$OUTPUT_DIR/dist"
mkdir -p "$OUTPUT_DIR/bin"

# Copy static files
echo "Copying static files..."
cp "$TEMP_DIR/datasette-lite/app.css" "$OUTPUT_DIR/dist/"
cp "$TEMP_DIR/datasette-lite/webworker.js" "$OUTPUT_DIR/dist/"

# Create modified index.html (remove Plausible analytics)
echo "Creating modified index.html..."
cat > "$OUTPUT_DIR/dist/index.html" << 'HTMLEOF'
<!DOCTYPE html>
<html>
<head>
  <title>Datasette</title>
  <link rel="stylesheet" href="app.css">
<style>
#loading-indicator {
  text-align: center;
}
#loading-indicator svg {
  height: 20vh;
}
#loading-indicator textarea {
  height: 40vh;
  width: 90%;
  padding: 1em;
  background-color: #276890;
  color: white;
  border: 1px solid black;
  overflow: auto;
  outline: none;
  resize: none;
  opacity: 0.6;
  font-size: min(3vw, 1.2em);
  line-height: 1.8;
}
textarea#sql-editor {
  padding: 0.5em;
  font-size: 1em;
  line-height: 1.4;
}
</style>
<!-- Analytics placeholder - customize or remove as needed -->
<script>
// Optional: Add your own analytics here
// window.plausible is stubbed out to prevent errors from the original code
window.plausible = window.plausible || function() {};
</script>
</head>
<body>

<div id="loading-indicator">
  <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
  width="200px" height="200px" viewBox="0 0 100 100">
<g transform="rotate(0 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.9166666666666666s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(30 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.8333333333333334s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(60 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.75s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(90 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.6666666666666666s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(120 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.5833333333333334s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(150 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.5s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(180 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.4166666666666667s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(210 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.3333333333333333s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(240 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.25s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(270 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.16666666666666666s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(300 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="-0.08333333333333333s" repeatCount="indefinite"></animate>
  </rect>
</g><g transform="rotate(330 50 50)">
  <rect x="47" y="24" rx="3" ry="6" width="6" height="12" fill="#292664">
    <animate attributeName="opacity" values="1;0" keyTimes="0;1" dur="1s" begin="0s" repeatCount="indefinite"></animate>
  </rect>
</g>
</svg>
<textarea id="loading-logs">Loading...</textarea>
</div>

<script>
const datasetteWorker = new Worker("webworker.js");
const githubUrl = /^https:\/\/github.com\/(.*)\/(.*)\/blob\/(.*)(\?raw=true)?$/;
const gistUrl = /^https:\/\/gist.github.com\/(.*)\/(.*)$/;

function fixUrl(url) {
  const githubMatch = githubUrl.exec(url);
  if (githubMatch) {
    return `https://raw.githubusercontent.com/${githubMatch[1]}/${githubMatch[2]}/${githubMatch[3]}`;
  }
  const gistMatch = gistUrl.exec(url);
  if (gistMatch) {
    return `https://gist.githubusercontent.com/${gistMatch[1]}/${gistMatch[2]}/raw`;
  }
  return url;
}

const urlParams = new URLSearchParams(location.search);
const sqliteUrl = fixUrl(urlParams.get('url'));
const memory = !!urlParams.get('memory');
const metadataUrl = fixUrl(urlParams.get('metadata'));
const csvUrls = urlParams.getAll('csv').map(fixUrl);
const sqlUrls = urlParams.getAll('sql').map(fixUrl);
const jsonUrls = urlParams.getAll('json').map(fixUrl);
const parquetUrls = urlParams.getAll('parquet').map(fixUrl);
const installUrls = urlParams.getAll('install');
const ref = urlParams.get('ref');

datasetteWorker.postMessage({type: 'startup', ref, sqliteUrl, memory, csvUrls, sqlUrls, jsonUrls, parquetUrls, installUrls, metadataUrl});

let loadingLogs = ["Loading..."];

function escapeHtml(unsafe) {
  return unsafe
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

datasetteWorker.onmessage = (event) => {
  var ta = document.getElementById('loading-logs');
  if (event.data.type == 'log') {
    loadingLogs.push(event.data.line);
    ta.value = loadingLogs.join("\n");
    ta.scrollTop = ta.scrollHeight;
    return;
  }
  let html = '';
  if (event.data.error) {
    html = `<div style="padding: 0.5em"><h3>Error</h3><pre style="white-space: pre-wrap">${escapeHtml(event.data.error)}</pre></div>`;
  } else if (/^text\/html/.exec(event.data.contentType)) {
    html = event.data.text;
  } else if (/^application\/json/.exec(event.data.contentType)) {
    html = `<pre style="padding: 0.5em">${escapeHtml(JSON.stringify(JSON.parse(event.data.text), null, 4))}</pre>`;
  } else {
    html = `<pre style="padding: 0.5em">${escapeHtml(event.data.text)}</pre>`;
  }
  document.getElementById("output").innerHTML = html;
  let title = document.getElementById("output").querySelector("title");
  if (title) {
    document.title = title.innerText;
  }
  window.scrollTo({top: 0, left: 0});
  document.getElementById('loading-indicator').style.display = 'none';
};
</script>

<div id="output"></div>

<form id="load-custom">
  <p style="padding: 1em">
    <button type="button" class="db-url">Load SQLite DB by URL</button>
    <button type="button" class="csv-url">Load CSV</button>
    <button type="button" class="json-url">Load JSON</button>
    <button type="button" class="parquet-url">Load Parquet</button>
    <button type="button" class="sql-url">Load SQL</button>
    <a style="text-decoration: none; padding-left: 0.3em; color: #067EFF" href="https://github.com/simonw/datasette-lite">Documentation</a>
  </p>
</form>

<script>
// Analytics disabled for self-hosted version
const useAnalytics = false;

window.onpopstate = function(event) {
  console.log(event);
  datasetteWorker.postMessage({path: event.target.location.hash.split("#")[1] || ""});
}

// Start with path from location.hash, if available
if (location.hash) {
  datasetteWorker.postMessage({path: location.hash.replace(/^\#/, '')});
} else {
  datasetteWorker.postMessage({path: "/"});
}

function isExternal(url) {
  let startsWithProtocol = !!/^http:\/\/|https:\/\//.exec(url);
  if (!startsWithProtocol) {
    return false;
  }
  // Is it localhost?
  return new URL(url).host != 'localhost';
}

function isFragmentLink(url) {
  // Is this a #fragment on the current page?
  let u = new URL(url);
  return (
    location.pathname == u.pathname &&
    location.hostname == u.hostname &&
    u.hash &&
    u.hash != location.hash
  );
}

function loadPath(path) {
  path = path.split("#")[0].replace("http://localhost", "");
  console.log({path});
  history.pushState({path: path}, path, "#" + path);
  datasetteWorker.postMessage({path});
}

let output = document.getElementById('output');
output.addEventListener('click', (ev => {
  var link = ev.srcElement.closest("a");
  if (link && link.href) {
    ev.stopPropagation();
    ev.preventDefault();
    if (isFragmentLink(link.href)) {
      // Jump them to that element, but don't update the URL bar
      // since we use # in the URL to mean something else
      let fragment = new URL(link.href).hash.replace("#", "");
      if (fragment) {
        let el = document.getElementById(fragment);
        el.scrollIntoView();
      }
      return;
    }
    let href = link.getAttribute("href");
    if (isExternal(href)) {
      window.open(href);
      return;
    }
    loadPath(href);
  }
}), true);

output.addEventListener('submit', (ev => {
  console.log(ev);
  ev.stopPropagation();
  ev.preventDefault();
  if (ev.target && ev.target.nodeName == "FORM" && ev.target.method.toLowerCase() == "get") {
    let qs = new URLSearchParams(new FormData(ev.target)).toString();
    let action = ev.target.getAttribute("action");
    loadPath(`${action}?${qs}`);
  }
}), true);

async function checkUrl(url, contentTypes) {
  try {
    response = (await fetch(url, {method: 'HEAD'}));
    if (response.status != 200) {
      return false;
    }
    if (contentTypes && (!contentTypes.includes(response.headers.get('content-type')))) {
      return false;
    }
    return true;
  } catch (error) {
    console.error(error);
    return false;
  }
}

const fileTypes = [
  {
    id: "db-url",
    prompt: "Enter a full URL to a SQLite .db file",
    param: "url",
    mimes: [
      "application/octet-stream",
      "application/x-sqlite3",
      "application/vnd.sqlite3",
    ],
  },
  { id: "csv-url", prompt: "Enter a full URL to a CSV file", param: "csv" },
  { id: "json-url", prompt: "Enter a full URL to a JSON file", param: "json" },
  {
    id: "parquet-url",
    prompt: "Enter a full URL to a Parquet file",
    param: "parquet",
  },
  { id: "sql-url", prompt: "Enter a full URL to a SQL file", param: "sql" },
];

fileTypes.forEach((fileType) => {
  document
    .querySelector(`#load-custom button.${fileType.id}`)
    .addEventListener("click", async function (ev) {
      ev.preventDefault();
      let url = fixUrl(prompt(fileType.prompt));
      if (!url) {
        return;
      }
      let valid = await checkUrl(url, fileType.mimes);
      if (valid) {
        location.href =
          location.pathname +
          "?" +
          fileType.param +
          "=" +
          encodeURIComponent(url);
      } else {
        alert(`That ${fileType.param.toUpperCase()} could not be loaded`);
      }
    });
});

</script>
</body>
</html>
HTMLEOF

# Create package.json
echo "Creating package.json..."
cat > "$OUTPUT_DIR/package.json" << 'JSONEOF'
{
  "name": "datasette-lite",
  "version": "1.0.0",
  "description": "Self-hostable Datasette running entirely in the browser using WebAssembly and Pyodide",
  "main": "index.js",
  "bin": {
    "datasette-lite": "./bin/serve.js"
  },
  "scripts": {
    "serve": "node bin/serve.js",
    "start": "node bin/serve.js"
  },
  "keywords": [
    "datasette",
    "sqlite",
    "pyodide",
    "webassembly",
    "wasm",
    "browser",
    "data",
    "database"
  ],
  "author": "Simon Willison",
  "license": "Apache-2.0",
  "repository": {
    "type": "git",
    "url": "https://github.com/simonw/datasette-lite"
  },
  "homepage": "https://lite.datasette.io/",
  "files": [
    "dist/",
    "bin/",
    "README.md"
  ]
}
JSONEOF

# Create index.js
echo "Creating index.js..."
cat > "$OUTPUT_DIR/index.js" << 'JSEOF'
/**
 * datasette-lite
 *
 * Self-hostable Datasette running entirely in the browser using WebAssembly and Pyodide.
 */

const path = require('path');

/**
 * Path to the dist directory containing static files
 */
const distPath = path.join(__dirname, 'dist');

/**
 * List of files in the dist directory
 */
const files = ['index.html', 'app.css', 'webworker.js'];

module.exports = {
  distPath,
  files,

  /**
   * Get the full path to a specific file in the dist directory
   * @param {string} filename - Name of the file
   * @returns {string} Full path to the file
   */
  getFilePath(filename) {
    return path.join(distPath, filename);
  }
};
JSEOF

# Create serve.js
echo "Creating bin/serve.js..."
cat > "$OUTPUT_DIR/bin/serve.js" << 'SERVEEOF'
#!/usr/bin/env node

const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.PORT || process.argv[2] || 8000;
const DIST_DIR = path.join(__dirname, '..', 'dist');

const MIME_TYPES = {
  '.html': 'text/html',
  '.css': 'text/css',
  '.js': 'application/javascript',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.wasm': 'application/wasm',
  '.db': 'application/octet-stream',
};

function getMimeType(filePath) {
  const ext = path.extname(filePath).toLowerCase();
  return MIME_TYPES[ext] || 'application/octet-stream';
}

const server = http.createServer((req, res) => {
  // Parse URL and remove query string
  let urlPath = req.url.split('?')[0];

  // Default to index.html
  if (urlPath === '/') {
    urlPath = '/index.html';
  }

  const filePath = path.join(DIST_DIR, urlPath);

  // Security: prevent directory traversal
  if (!filePath.startsWith(DIST_DIR)) {
    res.writeHead(403);
    res.end('Forbidden');
    return;
  }

  fs.readFile(filePath, (err, data) => {
    if (err) {
      if (err.code === 'ENOENT') {
        // If file not found, serve index.html (for SPA routing)
        fs.readFile(path.join(DIST_DIR, 'index.html'), (err2, indexData) => {
          if (err2) {
            res.writeHead(404);
            res.end('Not Found');
            return;
          }
          res.writeHead(200, {
            'Content-Type': 'text/html',
            'Access-Control-Allow-Origin': '*',
          });
          res.end(indexData);
        });
        return;
      }
      res.writeHead(500);
      res.end('Internal Server Error');
      return;
    }

    const mimeType = getMimeType(filePath);
    res.writeHead(200, {
      'Content-Type': mimeType,
      'Access-Control-Allow-Origin': '*',
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Opener-Policy': 'same-origin',
    });
    res.end(data);
  });
});

server.listen(PORT, () => {
  console.log(`Datasette Lite server running at http://localhost:${PORT}/`);
  console.log(`Press Ctrl+C to stop`);
});
SERVEEOF
chmod +x "$OUTPUT_DIR/bin/serve.js"

# Create README.md
echo "Creating README.md..."
cat > "$OUTPUT_DIR/README.md" << 'READMEEOF'
# datasette-lite

Self-hostable [Datasette](https://datasette.io/) running entirely in the browser using WebAssembly and [Pyodide](https://pyodide.org/).

This is a packaged version of [Datasette Lite](https://github.com/simonw/datasette-lite) that you can install and host on your own servers.

## Installation

```bash
npm install datasette-lite
```

## Usage

### Quick Start

After installation, you can start a local server:

```bash
npx datasette-lite
# or
npx datasette-lite 3000  # specify a port
```

Then open http://localhost:8000/ in your browser.

### Deploying Static Files

The `dist/` folder contains all the static files you need to host Datasette Lite:

- `index.html` - Main HTML page
- `app.css` - Stylesheet
- `webworker.js` - Web Worker that runs Pyodide and Datasette

You can copy these files to any static hosting service (GitHub Pages, Netlify, Vercel, S3, etc.).

```bash
# Copy files to your deployment directory
cp -r node_modules/datasette-lite/dist/* ./public/
```

### CORS Requirements

When loading external data files (SQLite databases, CSV, JSON, Parquet), the files must be served with CORS headers:

```
Access-Control-Allow-Origin: *
```

GitHub Pages and Gists automatically include this header.

## URL Parameters

Datasette Lite supports various URL parameters:

- `?url=URL` - Load a SQLite database from URL
- `?csv=URL` - Load a CSV file
- `?json=URL` - Load a JSON file
- `?parquet=URL` - Load a Parquet file
- `?sql=URL` - Execute SQL from a URL
- `?memory=1` - Start with just an in-memory database
- `?install=PACKAGE` - Install additional Python packages
- `?metadata=URL` - Load custom metadata
- `?ref=VERSION` - Use a specific Datasette version (e.g., `?ref=1.0a11`)

### Examples

```
http://localhost:8000/?csv=https://raw.githubusercontent.com/fivethirtyeight/data/master/fight-songs/fight-songs.csv
```

```
http://localhost:8000/?url=https://latest.datasette.io/fixtures.db
```

## How it Works

Datasette Lite runs the full Datasette Python web application directly in your browser:

1. When you load the page, it downloads the Pyodide WebAssembly runtime
2. It installs Datasette and its dependencies via micropip
3. It downloads your data files (SQLite databases, CSV, JSON, etc.)
4. Datasette runs as a Web Worker, handling requests and rendering HTML

All processing happens client-side - no server required for the actual data processing.

## License

Apache 2.0 - Same as the original Datasette and Datasette Lite projects.

## Credits

- [Datasette](https://datasette.io/) by Simon Willison
- [Datasette Lite](https://github.com/simonw/datasette-lite) by Simon Willison
- [Pyodide](https://pyodide.org/) - Python in the browser via WebAssembly
READMEEOF

# Cleanup
echo "Cleaning up..."
rm -rf "$TEMP_DIR"

echo ""
echo "Package built successfully!"
echo ""
echo "Contents of $OUTPUT_DIR:"
find "$OUTPUT_DIR" -type f | sort

echo ""
echo "To test the package:"
echo "  cd $OUTPUT_DIR"
echo "  npm install"
echo "  npm start"
echo ""
echo "To pack the package for publishing:"
echo "  cd $OUTPUT_DIR"
echo "  npm pack"
