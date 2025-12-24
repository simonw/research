# Datasette-lite JavaScript Initialization: Problem Analysis & Solution Proposal

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

## Executive Summary

Datasette-lite currently cannot execute JavaScript from Datasette pages because it loads content using `innerHTML`, which does not execute `<script>` tags. This document analyzes the problem and proposes a solution using a JavaScript initialization event pattern.

## The Problem

### How Datasette-lite Works

Datasette-lite runs Datasette inside a Web Worker using Pyodide (Python compiled to WebAssembly):

1. A shell `index.html` page loads with a loading spinner
2. A web worker (`webworker.js`) loads Pyodide and Datasette
3. When navigating pages, the worker returns HTML as text
4. The main page injects content using `innerHTML`:

```javascript
// From datasette-lite/index.html line 153
document.getElementById("output").innerHTML = html;
```

### Why Scripts Don't Execute

**Browser Security Feature**: When you set `innerHTML`, any `<script>` tags in the HTML are NOT executed. This is a fundamental browser security feature documented by MDN.

**Timing Issue**: Even if scripts were somehow executed, they rely on `DOMContentLoaded`, which:
1. Fires only once when the initial page loads
2. Has already fired by the time Datasette content is injected
3. Would never fire for subsequent page navigations

### Impact

This breaks:
- **table.js** - Column dropdown menus, sort/facet/hide options
- **datasette-manager.js** - Plugin registration system
- **datasette-cluster-map** - Map visualization
- **datasette-vega** - Chart visualization
- All other plugins that use JavaScript

## Current Architecture

### Datasette's JavaScript

**datasette-manager.js** sets up a manager object and fires a `datasette_init` event:

```javascript
document.addEventListener("DOMContentLoaded", function () {
  initializeDatasette();  // fires 'datasette_init' event
});
```

**table.js** listens for this event:

```javascript
document.addEventListener("datasette_init", function (evt) {
  const { detail: manager } = evt;
  initDatasetteTable(manager);
  addButtonsToFilterRows(manager);
  initAutocompleteForFilterValues(manager);
});
```

### Plugin Pattern

Plugins like datasette-cluster-map use `DOMContentLoaded`:

```javascript
document.addEventListener("DOMContentLoaded", () => {
  // Check for lat/long columns, add map
});
```

## Proposed Solution

### The `datasette_init` Event Pattern

The solution leverages Datasette's existing `datasette_init` event with enhancements:

1. **Add external initialization API**: Allow Datasette-lite to trigger initialization
2. **Support root element scoping**: Pass the content container for scoped queries
3. **Enable cleanup on reinit**: Prevent memory leaks on page navigation

### Key API Change

```javascript
// New public API for SPA frameworks / Datasette-lite
window.__DATASETTE_INIT__ = function(root = document) {
  document.dispatchEvent(new CustomEvent("datasette_init", {
    detail: {
      manager: datasetteManager,
      root: root,           // Scope queries to this element
      isReinit: true,       // Flag for reinitialization
    }
  }));
};
```

### Integration in Datasette-lite

```javascript
// In datasette-lite's onmessage handler:
datasetteWorker.onmessage = (event) => {
  if (/^text\/html/.exec(event.data.contentType)) {
    const output = document.getElementById("output");
    output.innerHTML = event.data.text;

    // Trigger Datasette initialization
    if (window.__DATASETTE_INIT__) {
      window.__DATASETTE_INIT__(output);
    }
  }
};
```

## Implementation Details

### Modified Files

| File | Changes |
|------|---------|
| `datasette-manager.js` | Add `__DATASETTE_INIT__` API, pass root element in events, support `data-datasette-spa` attribute |
| `table.js` | Add cleanup function, scope all queries to root, handle reinit |
| `datasette-cluster-map.js` | Switch from `DOMContentLoaded` to `datasette_init`, add cleanup |

### Backward Compatibility

The solution is **fully backward compatible**:

- Normal Datasette continues to use `DOMContentLoaded` → works exactly as before
- SPA mode is opt-in via `data-datasette-spa` attribute on `<html>`
- Plugins not updated still work (they just won't reinitialize on navigation)

### Plugin Migration Guide

Plugins should migrate from:

```javascript
document.addEventListener("DOMContentLoaded", () => {
  // initialize
});
```

To:

```javascript
document.addEventListener("datasette_init", (evt) => {
  const { root, isReinit } = evt.detail;

  if (isReinit) {
    // Clean up previous instance
  }

  // Initialize using root for DOM queries
  root.querySelector("table.rows-and-columns");
});
```

## Files in This Investigation

```
datasette-lite-js-init/
├── README.md                    # This report
├── notes.md                     # Detailed investigation notes
├── prototype/
│   ├── datasette-manager-modified.js  # Modified datasette-manager.js
│   ├── table-modified.js              # Modified table.js
│   ├── datasette-cluster-map-modified.js  # Modified plugin example
│   └── datasette-lite-integration.js  # Integration example code
├── datasette-manager.diff       # Diff for datasette-manager.js
├── table.diff                   # Diff for table.js
└── datasette-cluster-map.diff   # Diff for datasette-cluster-map
```

## Alternative Approaches Considered

### 1. Manual Script Execution
Parse innerHTML for `<script>` tags and recreate them.

**Rejected**: Causes memory leaks, ordering issues, doesn't solve DOMContentLoaded timing.

### 2. iframe Loading
Load each page in an iframe.

**Rejected**: Isolation issues, URL handling complexity, styling challenges.

### 3. Shadow DOM
Use Shadow DOM for content isolation.

**Rejected**: Doesn't solve script execution, adds complexity.

## Additional Considerations

### Inline Scripts in Templates

Some templates (e.g., `table.html`) contain inline scripts. Solutions:
1. Extract to separate `.js` files using `datasette_init`
2. Use data attributes instead of inline JavaScript
3. Move logic to core JavaScript files

### Plugin Discovery

For dynamically installed plugins (`?install=`), Datasette-lite needs to:
1. Know which plugins have JavaScript
2. Pre-load those scripts before triggering init

Possible solutions:
- Plugin manifest/registry
- Convention-based loading
- Server-side bundling endpoint

### Testing

A test page should be created that:
1. Loads Datasette scripts once
2. Simulates innerHTML content changes
3. Verifies features work after reinit
4. Checks for memory leaks

## Conclusion

The proposed solution:
- Solves the innerHTML script execution problem
- Maintains backward compatibility
- Leverages existing Datasette architecture
- Provides clear plugin migration path
- Is minimal in scope and complexity

The prototype implementation demonstrates feasibility. Next steps would be:
1. Review with Datasette maintainer
2. Add automated tests
3. Document plugin migration guide
4. Update core plugins (cluster-map, vega)
5. Update Datasette-lite to use the new API
