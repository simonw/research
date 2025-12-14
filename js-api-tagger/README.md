# Automatic JavaScript API Tagging for simonw/tools

This project provides tools to automatically categorize the 155 HTML tools in [simonw/tools](https://github.com/simonw/tools) based on the JavaScript APIs they use, without false positives from comments or string literals.

## The Challenge

Naively grepping for API names like `localStorage` or `WebAudio` would produce false positives from:
- HTML comments
- JavaScript string literals and template strings
- Non-executed code paths
- Documentation text

We need **proper parsing** to accurately detect which APIs are actually used.

## Solution: Cheerio + Acorn

The approach uses:
1. **[Cheerio](https://cheerio.js.org/)** - Parse HTML and extract `<script>` tag contents
2. **[Acorn](https://github.com/acornjs/acorn)** - Parse JavaScript into an AST
3. **AST Walking** - Find actual API usage in the code structure

This combination is:
- Fast (parses all 155 files in under a second)
- Accurate (no false positives from strings/comments)
- Handles ES modules and modern JavaScript

### Edge Cases Handled

| Script Type | Handling |
|------------|----------|
| `type="module"` | Parsed as ES module |
| `type="importmap"` | Skipped (JSON, not JS) |
| `type="text/babel"` | Graceful failure (JSX unsupported by Acorn) |
| `type="text/perl"` | Skipped (not JavaScript) |

## Tools Created

### `tagger.js` - JavaScript API Detection

Detects 60+ Web APIs organized into categories:

```bash
node tagger.js /tmp/tools detailed
```

**Output excerpt:**
```
=== API Categories Summary ===

Network: 50 files
Clipboard: 52 files
Storage: 26 files
Graphics: 23 files
File: 49 files
Media: 12 files
...

=== Files with Most API Usage ===

openai-audio.html: 12 APIs
  Media:WebAudio, Media:MediaDevices, Network:Fetch, Storage:localStorage...
```

### `categorizer.js` - Multi-dimensional Analysis

Goes beyond APIs to analyze:
- External libraries used
- HTML structure and semantic elements
- Accessibility features
- Code complexity metrics
- Interaction patterns (drag/drop, clipboard, file upload)
- Data format handling (JSON, CSV, Markdown, etc.)

```bash
node categorizer.js /tmp/tools
```

## Key Findings

### Most Common APIs (by file count)

| API | Files |
|-----|-------|
| Fetch API | 49 |
| Clipboard API | 34 |
| localStorage | 24 |
| Canvas | 22 |
| FileReader | 18 |
| URL/URLSearchParams | 30 |

### CDN Sources Used

| Domain | Files |
|--------|-------|
| cdn.jsdelivr.net | 19 |
| cdnjs.cloudflare.com | 18 |
| unpkg.com | 4 |
| esm.run | 4 |

### Libraries Detected

| Library | Files | Purpose |
|---------|-------|---------|
| Pyodide | 5 | Python in browser |
| Marked | 3 | Markdown rendering |
| React | 3 | UI framework |
| JSZip | 3 | ZIP file handling |
| Chart.js | 2 | Data visualization |
| Leaflet | 2 | Interactive maps |

### Inferred Tool Categories

| Category | Files |
|----------|-------|
| Utility | 69 |
| Clipboard utility | 52 |
| File processor | 27 |
| Generator/downloader | 18 |
| Markdown tool | 15 |
| Canvas-based | 8 |
| Media player | 6 |
| Python runtime | 5 |

## Other Categorization Methods (Non-LLM)

### Implemented in this project

1. **JS API fingerprinting** - Map APIs to semantic categories
2. **Library detection** - Match CDN URLs against known patterns
3. **HTML structure analysis** - Count semantic elements, form types
4. **Interaction pattern detection** - Event handlers for drag/drop, keyboard, touch
5. **Data format detection** - JSON, CSV, Base64, Markdown handling
6. **Code complexity metrics** - Line counts, modern JS feature usage
7. **Accessibility scoring** - ARIA attributes, alt text, lang attribute

### Additional possibilities

8. **Title/filename clustering** - Extract keywords from tool names and cluster
9. **CSS class analysis** - Detect UI frameworks by class naming patterns (e.g., `btn-primary` for Bootstrap)
10. **Comment/docstring extraction** - Find `@description` or similar annotations
11. **DOM structure fingerprinting** - Cluster tools by HTML structure similarity
12. **Dependency graph analysis** - Map relationships between tools
13. **Output type classification** - Categorize by what the tool produces (image, text, file, etc.)
14. **Input method detection** - Classify by input source (text, file, URL, camera, microphone)
15. **API call sequence analysis** - Use sequences of API calls as signatures
16. **Color/style analysis** - Extract color palettes and design patterns
17. **Bundle size estimation** - Approximate total code/dependency weight

## Usage

```bash
# Install dependencies
npm install cheerio acorn acorn-walk

# Run API tagger (outputs summary + results.json)
node tagger.js /tmp/tools

# Run multi-dimensional categorizer (outputs summary + categories.json)
node categorizer.js /tmp/tools

# Get JSON output only
node tagger.js /tmp/tools json
```

## Technical Notes

### Prototype Pollution Prevention

The API lookup object uses `Object.create(null)` to avoid false matches from inherited properties like `constructor`, `toString`, etc.

```javascript
// Bad - matches Object.prototype methods
const APIS = { localStorage: 'Storage' };
APIS['toString'];  // Returns [Function: toString]

// Good - null prototype
const APIS = Object.assign(Object.create(null), { localStorage: 'Storage' });
APIS['toString'];  // Returns undefined
```

### Parse Error Handling

Only 2 of 155 files have parse errors:
- `box-shadow.html` - Uses JSX (text/babel)
- `numpy-pyodide-lab.html` - Uses JSX (text/babel)

To support these, you could add `@babel/parser` as a fallback for JSX scripts.

## Files

- `tagger.js` - Main API detection script
- `categorizer.js` - Multi-dimensional analysis script
- `find-parse-errors.js` - Utility to identify unparseable scripts
- `results.json` - Full API detection results
- `categories.json` - Full categorization results
- `notes.md` - Research notes and methodology
