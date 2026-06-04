// Modified datasette-manager.js with SPA/reinit support
// Custom events for use with the native CustomEvent API
const DATASETTE_EVENTS = {
  INIT: "datasette_init", // returns datasette manager instance in evt.detail
};

// Datasette "core" -> Methods/APIs that are foundational
const DOM_SELECTORS = {
  jsonExportLink: ".export-links a[href*=json]",
  tableWrapper: ".table-wrapper",
  table: "table.rows-and-columns",
  aboveTablePanel: ".above-table-panel",
  tableHeaders: `table.rows-and-columns th`,
  filterRows: ".filter-row",
  facetResults: ".facet-results [data-column]",
};

/**
 * Monolith class for interacting with Datasette JS API
 */
const datasetteManager = {
  VERSION: window.datasetteVersion,
  plugins: new Map(),

  // NEW: Track current initialization state
  _initialized: false,
  _currentRoot: null,

  registerPlugin: (name, pluginMetadata) => {
    if (datasetteManager.plugins.has(name)) {
      console.warn(`Warning -> plugin ${name} was redefined`);
    }
    datasetteManager.plugins.set(name, pluginMetadata);

    if (pluginMetadata.makeAboveTablePanelConfigs) {
      datasetteManager.renderAboveTablePanel();
    }
  },

  makeColumnActions: (columnMeta) => {
    let columnActions = [];
    datasetteManager.plugins.forEach((plugin) => {
      if (plugin.makeColumnActions) {
        columnActions.push(...plugin.makeColumnActions(columnMeta));
      }
    });
    return columnActions;
  },

  renderAboveTablePanel: () => {
    const root = datasetteManager._currentRoot || document;
    const aboveTablePanel = root.querySelector(DOM_SELECTORS.aboveTablePanel);

    if (!aboveTablePanel) {
      console.warn(
        "This page does not have a table, the renderAboveTablePanel cannot be used.",
      );
      return;
    }

    let aboveTablePanelWrapper = aboveTablePanel.querySelector(".panels");

    if (!aboveTablePanelWrapper) {
      aboveTablePanelWrapper = document.createElement("div");
      aboveTablePanelWrapper.classList.add("tab-contents");
      const panelNav = document.createElement("div");
      panelNav.classList.add("tab-controls");
      panelNav.style.display = "flex";
      panelNav.style.gap = "8px";
      panelNav.style.marginTop = "4px";
      panelNav.style.marginBottom = "20px";
      aboveTablePanel.appendChild(panelNav);
      aboveTablePanel.appendChild(aboveTablePanelWrapper);
    }

    datasetteManager.plugins.forEach((plugin, pluginName) => {
      const { makeAboveTablePanelConfigs } = plugin;
      if (makeAboveTablePanelConfigs) {
        const controls = aboveTablePanel.querySelector(".tab-controls");
        const contents = aboveTablePanel.querySelector(".tab-contents");
        const configs = makeAboveTablePanelConfigs();

        configs.forEach((config, i) => {
          const nodeContentId = `${pluginName}_${config.id}_panel-content`;
          if (document.getElementById(nodeContentId)) {
            return;
          }
          const pluginControl = document.createElement("button");
          pluginControl.textContent = config.label;
          pluginControl.onclick = () => {
            contents.childNodes.forEach((node) => {
              node.style.display = node.id === nodeContentId ? "block" : "none";
            });
          };
          controls.appendChild(pluginControl);

          const pluginNode = document.createElement("div");
          pluginNode.id = nodeContentId;
          config.render(pluginNode);
          pluginNode.style.display = "none";
          contents.appendChild(pluginNode);
        });

        if (contents.childNodes.length) {
          contents.childNodes[0].style.display = "block";
        }
      }
    });
  },

  selectors: DOM_SELECTORS,
};

/**
 * NEW: Core initialization function that can be called multiple times
 * @param {Element} root - The root element to scope initialization to (default: document)
 * @param {Object} options - Configuration options
 * @param {boolean} options.isReinit - Whether this is a re-initialization (for SPA navigation)
 */
const initializeDatasette = (root = document, options = {}) => {
  const { isReinit = false } = options;

  // Store current root for scoped queries
  datasetteManager._currentRoot = root;

  // Set up global reference (only on first init)
  if (!window.__DATASETTE__) {
    window.__DATASETTE__ = datasetteManager;
  }

  console.debug(`Datasette Manager ${isReinit ? 'Reinitialized' : 'Created'}!`);

  // Dispatch the init event with root element info
  const initDatasetteEvent = new CustomEvent(DATASETTE_EVENTS.INIT, {
    detail: {
      manager: datasetteManager,
      root: root,           // NEW: The root element for scoped queries
      isReinit: isReinit,   // NEW: Flag to indicate reinitialization
    }
  });

  document.dispatchEvent(initDatasetteEvent);
  datasetteManager._initialized = true;
};

/**
 * NEW: Public API for SPA frameworks / Datasette-lite to trigger initialization
 * This allows external code to reinitialize Datasette after content changes
 */
window.__DATASETTE_INIT__ = function(root = document) {
  initializeDatasette(root, { isReinit: true });
};

/**
 * Main function - normal page load behavior
 * Fires AFTER the document has been parsed
 */
document.addEventListener("DOMContentLoaded", function () {
  // Only auto-init if not in SPA mode
  // SPA mode is indicated by presence of data-datasette-spa attribute on html element
  if (!document.documentElement.hasAttribute('data-datasette-spa')) {
    initializeDatasette();
  }
});

// Export for module usage
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { datasetteManager, initializeDatasette, DATASETTE_EVENTS };
}
