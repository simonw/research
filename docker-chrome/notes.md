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
- **JS Injection**: Chrome extension (MV3) + Playwright init scripts
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

### 2. Chrome Extension (MV3)
- Content script injected on every page
- Can hook fetch/XHR
- Communicates with CDP bridge via WebSocket

### 3. CDP Bridge Server
- Connects to Chrome via CDP
- Exposes WebSocket API for frontend
- Captures network events
- Allows script injection

### 4. Control Pane (Next.js)
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
- [ ] Create Dockerfile
- [ ] Create Chrome extension
- [ ] Create CDP bridge server
- [ ] Create Next.js control pane
