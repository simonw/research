# JS API Tagger - Research Notes

## Goal
Automatically tag HTML tools based on JavaScript APIs they use, with accurate parsing that avoids false positives from comments or string literals.

## Initial Exploration

### Repository Stats
- **155 HTML files** in the root of simonw/tools
- Mix of single-file tools using inline `<script>` tags
- Some use ES modules with CDN imports
- A few use JSX/Babel transpilation

### Sample File Patterns Observed
1. **audio-spectrum.html** - Uses Web Audio API (AudioContext), Canvas API, MediaDevices
2. **base64-gzip-decoder.html** - ES module with CDN import from cdnjs (pako library)
3. **box-shadow.html** - Uses React + Babel (text/babel scripts)
4. **dot.html** - Has importmap (JSON in script tag)

## Parsing Approach Research

### Options Considered

#### 1. Cheerio + Acorn (Chosen)
- **Pros**: Pure JavaScript, fast, mature, handles ES modules
- **Cons**: Doesn't handle JSX natively
- **Status**: Implemented and working

#### 2. Tree-sitter
- **Pros**: Very fast, multiple language support, incremental parsing
- **Cons**: Native compilation required, more complex setup
- **Status**: Dependency resolution issues in this environment

#### 3. Playwright/Puppeteer
- **Pros**: Full browser environment, handles all JS features
- **Cons**: Heavy, slow, overkill for static analysis
- **Status**: Not implemented

#### 4. Babel Parser
- **Pros**: Would handle JSX
- **Cons**: Additional dependency
- **Status**: Could be added for JSX files

### Edge Cases Discovered

1. **`type="importmap"`** - JSON, not JavaScript - must skip
2. **`type="text/babel"`** - JSX, acorn can't parse - graceful failure
3. **`type="text/perl"`** - Embedded Perl code - must skip
4. **Prototype pollution bug** - Object.create(null) needed for API lookup map

## Implementation

### tagger.js - JS API Detection
Uses AST walking to find:
- `NewExpression` - `new AudioContext()`, `new WebSocket()`
- `MemberExpression` - `navigator.mediaDevices`, `localStorage.setItem`
- `CallExpression` - `fetch()`, `requestAnimationFrame()`
- `ImportDeclaration` / `ImportExpression` - CDN URLs from ES imports

### categorizer.js - Multi-dimensional Analysis
Beyond APIs, detects:
- **Libraries** - React, D3, Chart.js, Pyodide, etc. (by CDN URL patterns)
- **HTML structure** - Semantic elements, form inputs, media elements
- **Accessibility** - ARIA attributes, alt text, lang attribute
- **Code complexity** - Lines of code, modern JS features
- **Interaction patterns** - Drag/drop, clipboard, file upload, download
- **Data formats** - JSON, CSV, Base64, Markdown, SVG

## Key Findings

### Most Common APIs
1. Fetch API (49 files)
2. Clipboard API (34 files)
3. localStorage (24 files)
4. Canvas (22 files)
5. FileReader (18 files)

### Most Common CDN Domains
1. cdn.jsdelivr.net (19 files)
2. cdnjs.cloudflare.com (18 files)
3. unpkg.com (4 files)
4. esm.run (4 files)

### Most Common Libraries
1. Pyodide (5 files) - Python in browser
2. Marked (3 files) - Markdown rendering
3. React (3 files) - UI framework
4. JSZip (3 files) - ZIP file handling

### Inferred Tool Categories
- Utility (69 files)
- Clipboard utility (52 files)
- File processor (27 files)
- Generator/downloader (18 files)
- Markdown tool (15 files)

## Alternative Categorization Methods (No LLM)

### Implemented
1. **JS API fingerprinting** - Map APIs to categories (Storage, Media, Network, etc.)
2. **Library detection** - Match CDN URLs against known patterns
3. **HTML structure analysis** - Count semantic elements, form types
4. **Interaction pattern detection** - Regex for event handlers
5. **Data format detection** - Regex for JSON/CSV/etc. handling
6. **Code complexity metrics** - Line counts, modern feature detection
7. **Accessibility scoring** - ARIA attributes, alt text presence

### Other Possible Methods (Not Implemented)
1. **Title/filename clustering** - Extract keywords from tool names
2. **CSS class analysis** - UI framework detection via class patterns
3. **Comment extraction** - Find @description or similar annotations
4. **DOM structure similarity** - Cluster tools by HTML structure
5. **Dependency graph** - Build relationships between files
6. **Output type detection** - Classify by what the tool produces
7. **Input method detection** - Text, file, URL, camera, etc.
8. **API call patterns** - Sequence of API calls as a signature

## Files with Parse Errors
Only 2 files couldn't be fully parsed:
- **box-shadow.html** - JSX (text/babel)
- **numpy-pyodide-lab.html** - JSX (text/babel)

Both use React with Babel transpilation, which acorn doesn't support natively.
