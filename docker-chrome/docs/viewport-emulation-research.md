# Research Summary: Viewport & Device Emulation (Puppeteer/CDP)

## Overview
This research explores the mechanics of viewport and device emulation in Chromium using Puppeteer and the Chrome DevTools Protocol (CDP). It clarifies the distinctions between physical window boundaries and the rendering viewport, particularly in non-headless environments.

## Internal Mechanics
- **Emulation Manager**: Puppeteer manages emulation via an `EmulationManager` that wraps CDP commands.
- **Key CDP Commands**:
    - `Emulation.setDeviceMetricsOverride`: Sets the viewport width/height, device scale factor (DPR), and mobile mode.
    - `Emulation.setTouchEmulationEnabled`: Toggles touch event support.
    - `Browser.setWindowBounds`: Resizes the actual OS-level browser window (headed mode only).
- **Reload Trigger**: Changing the `mobile` or `touch` flags via `page.setViewport()` often necessitates a page reload to ensure the site correctly adapts its layout and event listeners.

## Key Distinctions
### Window Size vs. Viewport Size
| Feature | Window Size (`Browser.setWindowBounds`) | Viewport Size (`Emulation.setDeviceMetricsOverride`) |
|---------|----------------------------------------|-------------------------------------------------------|
| **Scope** | Physical OS window dimensions | Rendering area (DOM `window.innerWidth`) |
| **Headed** | Supported | Supported |
| **Headless** | Not Applicable | Supported |
| **Visuals** | Resizes the outer shell | Resizes the inner content area |

### Implementation Risks
- **Mismatches**: If the window is smaller than the emulated viewport, scrollbars appear in the capture. If the window is larger, extra "gray area" or padding may be visible.
- **Persistent Emulation**: In Puppeteer, `defaultViewport` is 800x600 by default. Set it to `null` during `puppeteer.connect()` to allow the browser to maintain its own dimensions.

## Best Practices for Streaming
1. **Synchronized Resizing**: Always update both the window bounds and the viewport metrics to the same dimensions to maintain a clean "kiosk" look without scrollbars or margins.
2. **Order of Operations**: Set viewport/emulation parameters *before* navigating to a URL to prevent unnecessary reloads and flash of un-emulated content.
3. **Target Support**: Be aware that certain targets (like `chrome-extension://` pages) do not support metrics overrides; Puppeteer ignores these errors silently.

## Conclusion
Effective Chromium automation in containerized environments requires distinguishing between the browser window and the viewport. For clean results, both must be synchronized via CDP.
