# Docker Chrome - Development Notes

## Project Goal
Create a serverless Google Cloud Run service that:
1. Runs a modified Chrome that can capture network traffic
2. Execute arbitrary JS on any website
3. Can be remotely streamed and controlled through another browser
4. Network requests show up in a control pane

## Architecture Decision

Based on research, the recommended architecture is:
- **Streaming**: Selkies GStreamer (WebRTC) - much better latency than VNC
- **Browser**: Chromium (not Firefox) - best CDP/Playwright support
- **Automation**: Playwright connected via CDP
- **JS Injection**: CDP `Page.addScriptToEvaluateOnNewDocument` (extension removed)
- **Network Capture**: CDP Network domain events (decrypted HTTPS)

### Why Selkies over noVNC/xpra
| Feature | Selkies | noVNC |
|---------|---------|-------|
| Latency | Excellent (WebRTC) | Noticeable lag |
| Input responsiveness | Native-like | Acceptable |
| Cloud Run fit | Yes | Marginal |

### Why CDP over mitmproxy
- CDP observes traffic AFTER TLS termination in browser
- No certificate installation needed
- No proxy configuration needed
- Full request/response headers and bodies
- Timing information included

## Key Components

### 1. Docker Container
- Base: Ubuntu 22.04 with X11
- Chromium with remote debugging enabled
- Selkies GStreamer for WebRTC streaming
- Node.js for CDP bridge server

### 2. CDP Bridge Server
- Uses CDP `Page.addScriptToEvaluateOnNewDocument` for stealth/injection
- Can hook fetch/XHR via injected scripts
- Connects to Chrome via CDP
- Exposes WebSocket API for frontend
- Captures network events
- Allows script injection

### 3. Control Pane (Next.js)
- WebRTC player (Selkies client)
- Network request panel
- URL bar for navigation
- Script injection interface

## Research Sources

### Selkies GStreamer
- GitHub: https://github.com/selkies-project/selkies-gstreamer
- Example Docker images available
- HTML5 web client included

### Browserless Reference
- Popular Docker Chrome solution
- Uses Playwright/Puppeteer
- Good reference for Dockerfile patterns

## Implementation Progress

### 2024-12-17
- [x] Read redroid codebase for control pane pattern
- [x] Researched Selkies, CDP, Chrome extensions
- [x] Create Dockerfile
- [x] Create CDP bridge server
- [x] Create Next.js control pane

### 2025-12-18
- [x] Removed Chrome extension (replaced with CDP injection)
- [x] Added stealth scripts via CDP
- [x] Tested bot detection (bot.sannysoft passes, Google blocks on IP)
- [x] Improved paste UX with visual feedback
- [x] Added click-to-view response body functionality to NetworkPanel
- [ ] Add residential proxy support

### 2025-12-18 (Session 2) - Kiosk Mode & Display Fixes

#### Issue 1: Kiosk Mode Not Working
**Problem**: Chrome was showing full UI (URL bar, tabs, close button) instead of kiosk mode.

**Root Cause**: The linuxserver/chromium container uses Openbox window manager. The `--kiosk` Chrome flag makes Chrome fullscreen within its window, but Openbox still adds window decorations (title bar, close button).

**Solution**: Added `NO_DECOR=true` environment variable to deploy.sh. This tells the linuxserver container to configure Openbox to not show window borders.

#### Issue 2: Viewport/Display Scaling
**Problem**: Chrome window appeared small within the Selkies stream (black space around it).

**Root Cause**: Selkies dynamically negotiates resolution with clients, but without explicit configuration, the X11 display resolution may not match the Chrome window size.

**Solution**: Added `SELKIES_MANUAL_WIDTH=1920` and `SELKIES_MANUAL_HEIGHT=1080` to lock the display resolution. Combined with `--kiosk`, `--start-maximized`, and `NO_DECOR=true`, Chrome should fill the entire display.

#### Changes Made:
1. **deploy.sh**: Added environment variables:
   - `NO_DECOR=true` - Remove Openbox window decorations
   - `SELKIES_MANUAL_WIDTH=1920` - Lock display width
   - `SELKIES_MANUAL_HEIGHT=1080` - Lock display height
   - `--start-maximized` added to CHROME_CLI

2. **server/index.js**: Added `--start-maximized` flag to restartChromeWithNewUserDataDir()

#### Key linuxserver/chromium Environment Variables:
| Variable | Purpose |
|----------|---------|
| `CHROME_CLI` | Chrome command line flags (passed directly to wrapped-chromium) |
| `NO_DECOR` | Remove Openbox window decorations for PWA-like experience |
| `NO_FULL` | Don't auto-fullscreen apps in Openbox |
| `SELKIES_MANUAL_WIDTH` | Lock X11 display width |
| `SELKIES_MANUAL_HEIGHT` | Lock X11 display height |
| `SELKIES_IS_MANUAL_RESOLUTION_MODE` | Force manual resolution mode |

#### Critical Finding: --kiosk and --app Causes Black Screen
The `--kiosk` Chrome flag does NOT work with the linuxserver/chromium container. When `--kiosk` or `--app` is used, Chrome shows a completely black/blank screen - the browser process runs but nothing renders.

**Working Alternative**: Use `--app=about:blank` instead. This provides a minimal "PWA-like" window with no tabs, URL bar, or bookmarks, while navigation via CDP/Playwright still works.

### 2025-12-18 (Session 3) - Responsiveness & Lockdown Fixes

#### Issue 1: Browser Not Filling Viewport (Black Bars)
**Problem**: Browser had black bars around it - didn't fill the viewport.

**Root Cause**: Using `SELKIES_MANUAL_WIDTH=1920` and `SELKIES_MANUAL_HEIGHT=1080` locked the X11 display to fixed resolution. When client viewport was different, there would be black bars.

**Solution**: Remove manual resolution env vars - Selkies defaults to dynamic resolution that adapts to client viewport.

#### Issue 2: Restore Kiosk-Like Lockdown
**Problem**: `--kiosk` causes black screen, but we need minimal UI with no right-click, tabs, or URL bar.

**Solution**: 
1. Use `--start-fullscreen --start-maximized` - fills screen (NOTE: `--app` mode also causes black screen!)
2. Use `--block-new-web-contents` - prevent popups and new windows
3. Use Selkies hardening env vars: `HARDEN_DESKTOP=true`, `DISABLE_MOUSE_BUTTONS=true`, `HARDEN_KEYBINDS=true`
4. Inject lockdown scripts via CDP to disable right-click and block keyboard shortcuts
5. Use `NO_DECOR=true` to remove window decorations and make Chrome fill the entire display

> **Critical Finding: --app Mode Also Causes Black Screen**
> Just like `--kiosk`, the `--app=about:blank` flag also causes a completely black screen on linuxserver/chromium. Use `--start-fullscreen` instead.

**Final Working Configuration (deploy.sh)**:
```bash
CHROME_CLI=--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1 --remote-allow-origins=* --no-first-run --start-fullscreen --start-maximized --block-new-web-contents --disable-infobars --disable-extensions --disable-dev-tools --disable-blink-features=AutomationControlled about:blank
CDP_PORT=9222
NO_DECOR=true
HARDEN_DESKTOP=true
DISABLE_MOUSE_BUTTONS=true
HARDEN_KEYBINDS=true
SELKIES_UI_SHOW_SIDEBAR=false
```

**Note**: Removed `SELKIES_MANUAL_WIDTH` and `SELKIES_MANUAL_HEIGHT` to enable dynamic resolution.

#### Key linuxserver/chromium Hardening Variables:
| Variable | Purpose |
|----------|---------|
| `HARDEN_DESKTOP` | Meta-variable enabling multiple hardening options |
| `HARDEN_OPENBOX` | Enables openbox-specific hardening |
| `DISABLE_MOUSE_BUTTONS` | Disables Openbox right-click (not Chrome's context menu) |
| `HARDEN_KEYBINDS` | Disables Alt+F4, Alt+Escape, etc. |
| `SELKIES_UI_SHOW_SIDEBAR` | Hide the Selkies control sidebar |

### Current Status (2025-12-19)

**Working:**
- ✅ Browser fills the viewport (using 1920x1080 fixed resolution)
- ✅ No URL bar or tabs visible (fullscreen mode)
- ✅ Selkies sidebar hidden
- ✅ No infobar warnings
- ✅ Opens to about:blank

**Needs further work:**
- ⚠️ Right-click context menu still appears - CDP script injection timing issue
  - The `DISABLE_MOUSE_BUTTONS` only affects Openbox, not Chrome's internal context menu
  - CDP scripts are injected after Chrome starts, but the about:blank page is already loaded
  - Need to either reload the page after script injection or use a different approach

**Next steps to fix right-click:**
1. Add `page.reload()` after applying stealth scripts in `connectToBrowser()`
2. Or use Chrome's `--disable-context-menu` flag (if available)
3. Or inject script via a Chrome extension (more reliable)
