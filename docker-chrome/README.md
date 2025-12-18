# Docker Chrome Cloud

A serverless-ready, remotely controllable Chromium instance with full network capture and automation capabilities.

## Architecture

This project replicates the `redroid` pattern but for a desktop browser:

1.  **Frontend**: Next.js Control Pane (copied and adapted from redroid)
    *   WebRTC stream viewer (low latency)
    *   Session management
    *   Network traffic inspector (planned)

2.  **Container**: `selkies-gstreamer` + Chromium
    *   **Streaming**: Selkies (GStreamer WebRTC) for <100ms latency
    *   **Browser**: Google Chrome Stable with remote debugging (CDP) enabled
    *   **Automation**: Playwright connected to existing Chrome instance
    *   **Injection**: Custom Chrome Extension (MV3) for universal script injection

3.  **Bridge Server**: Node.js
    *   Connects to Chrome via CDP (port 9222)
    *   Captures `Network.requestWillBeSent` and `Network.responseReceived`
    *   Exposes HTTP/WebSocket API for the frontend

## Directory Structure

```
docker-chrome/
├── Dockerfile              # Selkies + Chrome + Node.js
├── extension/              # MV3 Chrome Extension for injection
├── scripts/                # Supervisord config
├── server/                 # CDP Bridge Server
└── control-pane/           # Next.js Frontend Application
    ├── src/
    │   ├── app/            # Page logic
    │   └── components/     # UI Components
    └── package.json
```

## Quick Start

### Build Container

```bash
docker build -t docker-chrome .
```

### Run Container

```bash
docker run -d \
  -p 8080:8080 \
  -p 9222:9222 \
  --shm-size=2g \
  docker-chrome
```

## Features

*   **Universal Injection**: Extension injects hooks into *every* page/frame immediately.
*   **Network Capture**: CDP captures decrypted HTTPS traffic (headers, bodies).
*   **Remote Control**: WebRTC streaming allows human interaction (login, captcha).
*   **Automation**: Playwright can drive the session programmatically via the bridge.

## Cloud Run Deployment

This container is designed for Cloud Run:
*   Single port (8080) for both WebRTC signaling and Control API.
*   Stateless (sessions are ephemeral).
*   Handles its own display server (Xvfb).
