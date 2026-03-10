/**
 * Anti-detection patches for proxied pages.
 * Injected into <head> before puppet-agent.js to mask automation signals
 * that Cloudflare and similar bot detectors check.
 */
(function () {
  "use strict";

  // Hide webdriver flag (set by automation tools)
  Object.defineProperty(navigator, "webdriver", {
    get: () => undefined,
    configurable: true,
  });

  // Provide chrome.runtime stub (expected in real Chrome)
  if (!window.chrome) {
    window.chrome = {};
  }
  if (!window.chrome.runtime) {
    window.chrome.runtime = {};
  }

  // Mock navigator.plugins (empty PluginArray in headless)
  if (navigator.plugins.length === 0) {
    const fakePlugins = [
      { name: "Chrome PDF Plugin", filename: "internal-pdf-viewer", description: "Portable Document Format" },
      { name: "Chrome PDF Viewer", filename: "mhjfbmdgcfjbbpaeojofohoefgiehjai", description: "" },
      { name: "Native Client", filename: "internal-nacl-plugin", description: "" },
    ];

    Object.defineProperty(navigator, "plugins", {
      get: () => {
        const arr = fakePlugins;
        arr.item = (i) => arr[i] || null;
        arr.namedItem = (name) => arr.find((p) => p.name === name) || null;
        arr.refresh = () => {};
        return arr;
      },
      configurable: true,
    });
  }

  // Mock navigator.languages (sometimes empty in automation)
  if (!navigator.languages || navigator.languages.length === 0) {
    Object.defineProperty(navigator, "languages", {
      get: () => ["en-US", "en"],
      configurable: true,
    });
  }

  // Remove UV config leak from global scope
  try {
    if (typeof __uv$config !== "undefined") {
      delete self.__uv$config;
    }
  } catch {
    // May not be deletable in all contexts
  }

  // Prevent permission query leak (headless returns "denied" for notifications)
  const origQuery = navigator.permissions?.query?.bind(navigator.permissions);
  if (origQuery) {
    navigator.permissions.query = function (desc) {
      if (desc.name === "notifications") {
        return Promise.resolve({ state: "prompt", onchange: null });
      }
      return origQuery(desc);
    };
  }
})();
