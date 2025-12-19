# Docker Chrome Cloud

A serverless-ready, remotely controllable Chromium instance with full network capture and automation capabilities.

## Architecture

This project replicates the `redroid` pattern but for a desktop browser:

1.  **Frontend**: Next.js Control Pane (copied and adapted from redroid)
    *   WebRTC stream viewer (low latency)
    *   Session management
    *   Network traffic inspector (planned)

2.  **Container**: `linuxserver/chromium` + Node.js
    *   **Streaming**: KasmVNC for low-latency browser streaming
    *   **Browser**: Chromium with remote debugging (CDP) enabled
    *   **Automation**: CDP-based control and script injection
    *   **Stealth**: Anti-bot-detection measures via CDP Page.addScriptToEvaluateOnNewDocument

3.  **Bridge Server**: Node.js
    *   Connects to Chrome via CDP (port 9222)
    *   Captures `Network.requestWillBeSent` and `Network.responseReceived`
    *   Exposes HTTP/WebSocket API for the frontend

## Directory Structure

```
docker-chrome/
├── Dockerfile              # Selkies + Chrome + Node.js
├── scripts/                # Supervisord config
├── server/                 # CDP Bridge Server (network capture, stealth injection)
├── docs/                   # Research documentation
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

*   **Stealth Mode**: CDP-based script injection removes bot detection fingerprints.
*   **Network Capture**: CDP captures decrypted HTTPS traffic (headers, bodies).
*   **Remote Control**: VNC streaming allows human interaction (login, captcha).
*   **Session Reset**: Fresh browser profile on demand via API.
*   **Paste API**: Insert text into focused elements via CDP.

## Cloud Run Deployment

This container is designed for Cloud Run:
*   Single port (8080) for both WebRTC signaling and Control API.
*   Stateless (sessions are ephemeral).
*   Handles its own display server (Xvfb).
