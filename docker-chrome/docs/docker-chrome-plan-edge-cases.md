# Docker-Chrome Plan Edge Cases Research Summary

## Overview
Risk analysis and edge-case identification for the planned `docker-chrome` improvements, including network tracking and session lifecycle.

## Key Findings
- **Network Type propagation**: `requestWillBeSent.type` can be `undefined` for redirects, workers, or special schemes. Canonical backfill from `responseReceived.type` is required.
- **WebSocket/SSE**: These are long-lived and require dedicated CDP events (`Network.webSocketCreated`, `Network.eventSourceMessageReceived`); they will not show up in the standard `loadingFinished` flow.
- **Session Reset**: `docker-chrome/server/index.js` currently uses global state. A "Reset Session" must carefully clear contexts, close pages, and potentially re-inject persistent scripts to guarantee a "fresh" state without restarting the container.
- **CORS Preflights**: Preflight `OPTIONS` requests can create "duplicate" rows in the UI if not filtered or grouped with the main request.

## Recommendations
- Implement requestId-based state consolidation on the server.
- Treat WebSockets as first-class, long-lived records in the Network panel.
- Implement a clear `Session` object to encapsulate `context` and `page` instead of using globals.
