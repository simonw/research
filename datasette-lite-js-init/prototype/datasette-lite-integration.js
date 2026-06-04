/**
 * Example Datasette-lite integration with the new init pattern
 *
 * This shows how Datasette-lite's index.html would be modified to:
 * 1. Pre-load Datasette's JavaScript files
 * 2. Trigger reinitialization after each page load via innerHTML
 */

// ============================================================================
// APPROACH 1: Simple integration with __DATASETTE_INIT__
// ============================================================================

// In Datasette-lite's onmessage handler, after setting innerHTML:
datasetteWorker.onmessage = (event) => {
  // ... existing handling for logs and errors ...

  if (/^text\/html/.exec(event.data.contentType)) {
    const output = document.getElementById("output");
    output.innerHTML = event.data.text;

    // NEW: Trigger Datasette reinitialization for the loaded content
    if (window.__DATASETTE_INIT__) {
      // Pass the output element as root so queries are scoped to it
      window.__DATASETTE_INIT__(output);
    }
  }

  // ... rest of existing handling ...
};


// ============================================================================
// APPROACH 2: More robust integration with script extraction
// ============================================================================

/**
 * This approach extracts data from inline scripts in the HTML
 * and makes it available before triggering init
 */
function handleDatasetteHTML(html, outputElement) {
  // Set the HTML
  outputElement.innerHTML = html;

  // Extract and evaluate any important data from inline scripts
  // For example, DATASETTE_ALLOW_FACET is defined in table.html
  const inlineScripts = outputElement.querySelectorAll('script:not([src])');
  inlineScripts.forEach(script => {
    const content = script.textContent;

    // Look for specific patterns we need to evaluate
    // DATASETTE_ALLOW_FACET
    const facetMatch = content.match(/DATASETTE_ALLOW_FACET\s*=\s*(true|false)/i);
    if (facetMatch) {
      window.DATASETTE_ALLOW_FACET = facetMatch[1].toLowerCase() === 'true';
    }

    // Add other patterns as needed...
  });

  // Trigger reinitialization
  if (window.__DATASETTE_INIT__) {
    window.__DATASETTE_INIT__(outputElement);
  }
}


// ============================================================================
// APPROACH 3: Full index.html modification
// ============================================================================

/*
The Datasette-lite index.html would need these changes:

1. Add data-datasette-spa attribute to prevent auto-init:

   <html data-datasette-spa>

2. Pre-load Datasette scripts in the <head>:

   <head>
     <!-- Existing styles -->
     <link rel="stylesheet" href="app.css">

     <!-- Pre-load Datasette scripts -->
     <script>window.datasetteVersion = '1.0';</script>
     <script src="/-/static/datasette-manager.js" defer></script>
     <script src="/-/static/table.js" defer></script>

     <!-- Pre-load plugin scripts -->
     <script src="/-/static/datasette-cluster-map.js" defer></script>
   </head>

3. In the main script, after innerHTML:

   datasetteWorker.onmessage = (event) => {
     // ... existing code ...

     if (/^text\/html/.exec(event.data.contentType)) {
       const output = document.getElementById("output");
       output.innerHTML = html;

       // Trigger reinitialization
       if (window.__DATASETTE_INIT__) {
         window.__DATASETTE_INIT__(output);
       }
     }
   };
*/


// ============================================================================
// ALTERNATIVE: Using MutationObserver for automatic detection
// ============================================================================

/**
 * This approach uses MutationObserver to automatically detect
 * when content is added and trigger initialization.
 *
 * Advantage: No changes needed to the onmessage handler
 * Disadvantage: Might fire on partial updates
 */
function setupAutoInit() {
  const output = document.getElementById("output");

  const observer = new MutationObserver((mutations) => {
    // Check if significant content was added
    const hasNewContent = mutations.some(mutation => {
      return mutation.addedNodes.length > 0 &&
             Array.from(mutation.addedNodes).some(node =>
               node.nodeType === 1 && node.tagName !== 'SCRIPT'
             );
    });

    if (hasNewContent && window.__DATASETTE_INIT__) {
      // Small delay to ensure all content is in place
      requestAnimationFrame(() => {
        window.__DATASETTE_INIT__(output);
      });
    }
  });

  observer.observe(output, {
    childList: true,
    subtree: false  // Only watch direct children for efficiency
  });
}


// ============================================================================
// SCRIPT LOADING HELPER
// ============================================================================

/**
 * Helper to load scripts that might be referenced in the Datasette HTML
 * This is useful if plugins add their own scripts that need to be loaded
 */
const loadedScripts = new Set();

function loadScriptFromHTML(outputElement) {
  const scripts = outputElement.querySelectorAll('script[src]');
  const promises = [];

  scripts.forEach(script => {
    const src = script.getAttribute('src');

    // Skip if already loaded
    if (loadedScripts.has(src)) {
      return;
    }

    // Create new script element
    const newScript = document.createElement('script');
    const promise = new Promise((resolve, reject) => {
      newScript.onload = resolve;
      newScript.onerror = reject;
    });

    if (script.type === 'module') {
      newScript.type = 'module';
    }
    newScript.src = src;
    document.body.appendChild(newScript);
    loadedScripts.add(src);
    promises.push(promise);
  });

  return Promise.all(promises);
}

/**
 * Usage:
 *
 * datasetteWorker.onmessage = async (event) => {
 *   if (/^text\/html/.exec(event.data.contentType)) {
 *     const output = document.getElementById("output");
 *     output.innerHTML = html;
 *
 *     // Load any new scripts first
 *     await loadScriptFromHTML(output);
 *
 *     // Then trigger initialization
 *     if (window.__DATASETTE_INIT__) {
 *       window.__DATASETTE_INIT__(output);
 *     }
 *   }
 * };
 */
