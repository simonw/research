# Research Summary: Chromium Window Modes (X11 Perspective)

## Overview
This research analyzed Chromium's behavior in various window modes from a systems/X11 perspective, focusing on window properties like `WM_CLASS`, `override-redirect`, and `_NET_WM_WINDOW_TYPE`. The goal was to understand how these modes interact with window managers (like Openbox) and Xvfb/VNC capture systems.

## Key Chromium Modes
- **Normal Mode**: Standard browser window.
    - `WM_CLASS`: "chromium"
    - `override_redirect`: `false`
    - `_NET_WM_WINDOW_TYPE`: `_NET_WM_WINDOW_TYPE_NORMAL`
- **--start-fullscreen**: Starts the browser in fullscreen (like F11). Managed by the Window Manager.
- **--kiosk**: Enables kiosk mode. Borderless, fullscreen, and designed to prevent user exit. Often bypasses standard WM controls.
- **--app**: Launches a specific URL in a minimal "application" window. Allows overriding `WM_CLASS` via the `--class` flag.

## X11 Technical Details
- **Override Redirect**: Chromium uses `override_redirect = true` for transient windows like menus, tooltips, and popups. This bypasses the Window Manager, allowing the browser to draw directly to the screen.
- **Window Types**: Chromium sets EWMH atoms (e.g., `_NET_WM_WINDOW_TYPE_NORMAL`, `_NET_WM_WINDOW_TYPE_MENU`) to hint to the WM how the window should be treated.
- **WM_CLASS**: Essential for WM identification and applying specific rules (e.g., in Openbox `rc.xml`).

## Black Screen & Capture Issues
The research identified several mechanisms that could lead to "black screens" in headless/containerized environments:
1. **WM Interactions**: The Window Manager not being configured to handle or display `override_redirect` windows correctly.
2. **Capture Pipeline**: Xvfb or VNC capture failing to sync with specific window types or rapid property changes.
3. **Hardware Acceleration**: GPU acceleration conflicts in virtualized X environments.

## Conclusion
Understanding the X11 properties set by Chromium in different modes is critical for ensuring stable display and interaction in containerized environments. Using `xprop` to verify these properties is a recommended debugging step.
