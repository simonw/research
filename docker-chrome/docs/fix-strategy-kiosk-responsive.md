# Research Summary: Fix Strategy for Kiosk Mode & Responsiveness

## Overview
This research aimed to decouple and solve two persistent issues in the `docker-chrome` project: the "black screen" when using Chromium's `--kiosk` mode and the lack of fluid responsiveness in the remote stream.

## 1. The Kiosk Mode Problem
- **Root Cause**: Chromium's `--kiosk` and `--app` modes set specific X11 window types that minimal Window Managers (like Openbox) often struggle to manage. These windows may be placed off-screen or treated as `override-redirect` (bypassing the WM entirely), leading to capture failures.
- **The "Good State" Workaround**: Rather than forcing `--kiosk`, use **`--start-fullscreen`**. This creates a standard manageable window that fills the display reliably. Interaction restrictions (hiding the URL bar, right-click, etc.) should be handled via the Chrome DevTools Protocol (CDP) and CSS rather than browser-level flags.

## 2. The Responsiveness Challenge (The Four-Layer Problem)
Responsiveness fails when there is a mismatch between any of these four independent layers:
1. **Client Viewport**: The space available in the user's local browser.
2. **WebRTC Stream**: The resolution negotiated by the Selkies/GStreamer pipeline.
3. **X11 Display**: The virtual framebuffer size set for Xvfb (e.g., 1920x1080).
4. **CDP Viewport**: The dimensions the page *thinks* it has, as set via `page.setViewportSize`.

**Strategy**: To eliminate black bars and blurriness, the project must synchronize these layers or use CSS `object-fit: cover/contain` with aspect-ratio constraints on the container.

## 3. Critical Infrastructure Fixes
- **WebSocket Path Collision**: The server's upgrade handler was found to potentially intercept Selkies WebRTC signaling if it used the `/ws` path.
- **Proposed Solution**: Rename the control WebSocket endpoint to `/control-ws` to ensure clean separation from the media signaling path.

## Recommended Diagnostic Workflow
- **X11 Inspection**: Use `xprop` and `wmctrl` within the container to verify that Chromium is actually mapped and visible on the `:0` display.
- **Visual Verification**: Use `ffmpeg -f x11grab` to take a screenshot of the root window to see exactly what Xvfb is rendering on the server side.

## Conclusion
Stability is achieved by using standard window modes and handling constraints programmatically via CDP, while responsiveness requires a unified scaling strategy across the frontend and the X11 framebuffer.
