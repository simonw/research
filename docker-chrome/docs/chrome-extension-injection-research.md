# Chrome Extension Universal JS Injection Research

## Purpose
Investigate methods for universally injecting JavaScript into web pages using Manifest v3 Chrome Extensions, specifically focusing on `fetch`/`XHR` hooking and compatibility with headless environments.

## Key Findings
- **Manifest v3 Worlds:**
  - **ISOLATED:** Default. Secure, but cannot see page variables or hook global functions like `fetch`.
  - **MAIN:** Required for hooking. Accesses the same DOM and JS context as the page.
- **Injection Methods:**
  - **Static:** `content_scripts` in `manifest.json`. Best for "always-on" scripts.
  - **Dynamic:** `chrome.scripting.registerContentScripts`. Best for runtime configuration.
- **Headless Loading:** The CLI flag `--load-extension` is deprecated in newer Chrome versions. The recommended alternative is using CDP `Extensions.loadUnpacked`.

## Implementation Patterns
A robust "Universal Injector" extension requires two content scripts:
1. `content-script-main.js`: Runs in `MAIN` world to hook network requests.
2. `content-script-isolated.js`: Runs in `ISOLATED` world to handle extension logic and messaging.

## Conclusion
Universal injection is feasible and reliable with Manifest v3, provided the distinction between execution worlds is respected.
