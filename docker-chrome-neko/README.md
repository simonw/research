# Docker Chrome Neko

Remote browser control using **neko** (WebRTC) + **Chromium** + **CDP/Puppeteer** with a Next.js control panel.

## Features

- 🎥 **WebRTC streaming** via neko (low-latency, native browser support)
- 🔍 **Network traffic capture** via Chrome DevTools Protocol
- 💉 **Remote JS injection** (one-time and persistent)
- 📐 **Responsive viewport** - resizes with browser/container
- 🎛️ **Control panel** - navigate, inject scripts, view network traffic

## Quick Start

### Local Development (Docker Compose)

```bash
# Build and start
docker compose up -d --build

# View logs
docker compose logs -f
```

**Exposed Ports:**
- `http://localhost:8080` - neko WebRTC UI
- `http://localhost:3001` - CDP Agent API
- `ws://localhost:3001/ws` - Network events WebSocket
- `http://localhost:9222` - Chrome DevTools Protocol

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/healthz` | GET | Health check |
| `/api/status` | GET | CDP connection status |
| `/api/navigate` | POST | Navigate to URL |
| `/api/inject` | POST | One-time JS execution |
| `/api/inject/persist` | GET/POST/DELETE | Persistent scripts |
| `/api/viewport` | GET/POST | Get/set viewport |
| `/api/paste` | POST | Paste text |
| `/api/network/:id/body` | GET | Get response body |

### Examples

```bash
# Navigate to URL
curl -X POST http://localhost:3001/api/navigate \
  -H "Content-Type: application/json" \
  -d '{"url":"https://example.com"}'

# Inject JavaScript
curl -X POST http://localhost:3001/api/inject \
  -H "Content-Type: application/json" \
  -d '{"code":"document.body.style.background=\"red\""}'

# Set viewport
curl -X POST http://localhost:3001/api/viewport \
  -H "Content-Type: application/json" \
  -d '{"width":1920,"height":1080}'
```

## Architecture

```
┌─────────────────────────────────────────────┐
│           Docker Container                   │
│  ┌────────────┐    ┌──────────────────────┐ │
│  │   neko     │    │    CDP Agent         │ │
│  │  (WebRTC)  │    │   (Express + WS)     │ │
│  │   :8080    │    │      :3001           │ │
│  └─────┬──────┘    └──────────┬───────────┘ │
│        │                      │             │
│        └──────┐    ┌──────────┘             │
│               ▼    ▼                        │
│         ┌──────────────┐                    │
│         │   Chromium   │                    │
│         │  (CDP :9222) │                    │
│         └──────────────┘                    │
└─────────────────────────────────────────────┘
```

## Control Panel (Next.js)

```bash
cd control-pane
npm install
npm run dev
# Open http://localhost:3000
```

## Related Projects

- [docker-chrome](../docker-chrome) - VNC-based approach with Selkies
- [redroid](../redroid) - Android emulator with GCP deployment
