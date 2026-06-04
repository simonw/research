# Datasette-lite JavaScript Initialization Investigation

## Goal
Understand why JavaScript doesn't execute in Datasette-lite and propose/prototype a solution using a JavaScript init() event pattern.

## Investigation Started

### Cloned Repositories
- /tmp/datasette - main Datasette application
- /tmp/datasette-lite - WebAssembly-based Datasette in the browser
- /tmp/datasette-cluster-map - plugin that displays maps
- /tmp/datasette-vega - plugin for charts

---

## Phase 1: Understanding the innerHTML Problem

### How Datasette-lite Works

Datasette-lite runs Datasette inside a **Web Worker** using Pyodide (Python compiled to WebAssembly).

The architecture:
1. `index.html` - Main page with a loading indicator and an empty `<div id="output"></div>`
2. `webworker.js` - Runs Pyodide, loads Datasette, handles requests
3. When user navigates, the main page sends a message to the worker
4. Worker runs the Datasette request and returns HTML
5. Main page sets the HTML using **innerHTML**:
   ```javascript
   // From index.html line 153
   document.getElementById("output").innerHTML = html;
   ```

### The Problem: innerHTML Doesn't Execute Scripts

This is a fundamental browser security feature. When you set `innerHTML`, any `<script>` tags in the HTML are **NOT executed**.

From MDN: "Although `<script>` tags are inserted into the document, the scripts they reference are not executed."

This means:
1. Datasette's `table.js` never runs
2. Datasette's `datasette-manager.js` never runs
3. No plugins that inject JavaScript work
4. Inline scripts in templates don't run

### Current Datasette JavaScript Architecture

**datasette-manager.js** (loaded via base.html):
- Sets up `window.__DATASETTE__` manager object
- Fires `datasette_init` CustomEvent on **DOMContentLoaded**
- Provides plugin registration API

**table.js** (loaded via table.html):
- Listens for `datasette_init` event
- Adds column dropdown menus
- Adds filter row buttons
- Sets up autocomplete for filter values

**Base template script loading** (base.html):
```html
<script>window.datasetteVersion = '{{ datasette_version }}';</script>
<script src="{{ urls.static('datasette-manager.js') }}" defer></script>
{% for url in extra_js_urls %}
    <script {% if url.module %}type="module" {% endif %}src="{{ url.url }}"></script>
{% endfor %}
```

### Plugin JavaScript Patterns

**datasette-cluster-map**:
```javascript
document.addEventListener("DOMContentLoaded", () => {
  // Check for lat/long columns, add map
});
```

**datasette-vega**:
```javascript
document.addEventListener('DOMContentLoaded', () => {
  // Create chart widget
});
```

Both use DOMContentLoaded, which:
1. Only fires once when the original page loads
2. Has already fired by the time Datasette-lite injects content
3. Would never fire for subsequently injected content anyway

### Why This is a Problem in Datasette-lite

In normal Datasette:
1. Server returns full HTML page with scripts
2. Browser loads page, executes scripts
3. DOMContentLoaded fires → scripts initialize

In Datasette-lite:
1. Initial page loads (DOMContentLoaded fires - but content is just loading spinner)
2. Pyodide/Datasette starts in web worker
3. HTML pages are fetched via worker, returned as text
4. Content injected via innerHTML → **scripts don't execute**
5. Even if they did, DOMContentLoaded already fired

---

## Phase 2: Analyzing Possible Solutions

### Option 1: Manually Execute Scripts After innerHTML

Could parse the inserted HTML, find `<script>` tags, and re-create them:

```javascript
document.getElementById("output").innerHTML = html;
const scripts = document.getElementById("output").querySelectorAll("script");
scripts.forEach(script => {
  const newScript = document.createElement("script");
  if (script.src) {
    newScript.src = script.src;
  } else {
    newScript.textContent = script.textContent;
  }
  document.body.appendChild(newScript);
});
```

**Problems:**
- Scripts would re-run on every page navigation
- No cleanup between pages (memory leaks, duplicate handlers)
- Script ordering/dependency issues
- Still doesn't solve plugin scripts that use DOMContentLoaded

### Option 2: Use a Page Init Event (Recommended)

Create a new pattern where:
1. All scripts are loaded ONCE (in the shell page or pre-loaded)
2. Scripts register handlers for a "page content loaded" event
3. The event fires EVERY TIME content is loaded (initial + navigation)

This is similar to how SPAs (React Router, Vue Router) handle page transitions.

**Key insight**: Datasette already has `datasette_init` event! We just need:
1. Fire it on content load, not DOMContentLoaded
2. Have scripts that need re-initialization listen for it
3. Update plugins to use this pattern

### Option 3: Pre-load All Scripts in Datasette-lite

Datasette-lite could pre-load all required scripts in its index.html before any content loads, then trigger initialization when content is inserted.

Combined with Option 2, this would work well.

### Option 4: iframe-based Loading

Load each page in an iframe instead of innerHTML. This would execute scripts.

**Problems:**
- Iframe isolation issues
- Cross-frame communication complexity
- URL bar won't reflect current page
- Styling/sizing challenges

---

## Phase 3: Proposed Solution Design

### The `datasette_init` Event Approach

The solution involves:

1. **Modify Datasette's core JavaScript** to support multiple initialization modes:
   - Normal mode: Fire on DOMContentLoaded (current behavior)
   - SPA mode: Fire when explicitly triggered

2. **Create initialization function** that can be called from outside:
   ```javascript
   window.__DATASETTE_INIT__ = function(root = document) {
     // Re-run initialization for content in root element
     document.dispatchEvent(new CustomEvent("datasette_init", {
       detail: { manager: datasetteManager, root: root }
     }));
   };
   ```

3. **Update table.js and other scripts** to:
   - Be idempotent (safe to call multiple times)
   - Scope DOM queries to the provided root element
   - Clean up previous state before reinitializing

4. **Update plugin pattern** to use the same approach

5. **In Datasette-lite**:
   - Pre-load core scripts once
   - After innerHTML, call `window.__DATASETTE_INIT__(outputElement)`

### Advantages

1. **Backward compatible**: Normal Datasette works exactly as before
2. **SPA-friendly**: Works with Datasette-lite and potential future SPAs
3. **Plugin-friendly**: Plugins can adopt the pattern incrementally
4. **Minimal changes**: Core concept already exists, just needs refinement

### Key Changes Required

**datasette-manager.js**:
- Export initialization function
- Allow specifying root element for scoped queries
- Support re-initialization

**table.js**:
- Accept root element in init function
- Clean up previous handlers
- Scope all queries to root

**Plugins** (datasette-cluster-map, datasette-vega):
- Switch from DOMContentLoaded to datasette_init
- Support root element parameter

**Datasette-lite**:
- Pre-load Datasette scripts
- Call init function after innerHTML

---

## Phase 4: Prototype Implementation

### Files Created

1. **prototype/datasette-manager-modified.js** - Modified datasette-manager.js with:
   - `window.__DATASETTE_INIT__(root)` function for external triggering
   - Support for `root` element parameter in events
   - `isReinit` flag to distinguish initial load vs reinitialization
   - `data-datasette-spa` attribute to disable auto-init

2. **prototype/table-modified.js** - Modified table.js with:
   - Cleanup function to remove previous handlers and DOM elements
   - All DOM queries scoped to `root` element
   - Idempotent initialization (safe to call multiple times)
   - Handles both normal and SPA URL formats

3. **prototype/datasette-cluster-map-modified.js** - Modified plugin showing:
   - Switch from `DOMContentLoaded` to `datasette_init` event
   - Cleanup of previous map instance on reinit
   - Support for `root` element parameter

4. **prototype/datasette-lite-integration.js** - Example integration code for Datasette-lite

### Generated Diffs

- `datasette-manager.diff` - Changes to datasette/static/datasette-manager.js
- `table.diff` - Changes to datasette/static/table.js
- `datasette-cluster-map.diff` - Changes to datasette-cluster-map

### Key Design Decisions

1. **Backward Compatibility**: The modified code works exactly like the original in normal Datasette. The SPA mode is opt-in via `data-datasette-spa` attribute.

2. **Event-based Architecture**: Using the existing `datasette_init` event pattern, just with enhanced payload (root element, isReinit flag).

3. **Cleanup Pattern**: Scripts need to track handlers and DOM elements for cleanup. This is necessary for any SPA to prevent memory leaks.

4. **Root Element Scoping**: All DOM queries should be scoped to the root element passed in the event. This allows multiple Datasette instances or partial page updates.

---

## Additional Considerations

### Inline Scripts in Templates

Some templates include inline scripts (e.g., table.html has a script for the "count all" link). Options:

1. **Extract to separate file**: Move inline scripts to their own .js files that listen for datasette_init
2. **Use data attributes**: Store configuration in data attributes, read by init scripts
3. **Parse and eval**: Datasette-lite could parse inline scripts and extract needed data (hacky)

### Plugin Discovery

For plugins installed via `?install=`, Datasette-lite would need to:
1. Know which plugins have JavaScript
2. Pre-load those scripts before triggering init

This could be solved by:
- A plugin registry/manifest
- Convention-based loading (e.g., `/-/static/{plugin-name}.js`)
- Server-side bundling

### CSS Handling

CSS loaded via innerHTML works fine (no execution needed). The existing pattern of loading CSS via link elements is already handled by plugins.

