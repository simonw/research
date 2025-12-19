# WebSocket Fanout and Multiplexing Research Summary

## Overview
Investigation into scaling the number of concurrent viewers for a single `docker-chrome` session using WebSocket fanout and multiplexing techniques.

## Key Findings
- **Shared Worker vs Proxy**: A server-side proxy (e.g., Redis Pub/Sub or a simple event emitter) is necessary to fan out CDP events from a single browser connection to multiple client WebSockets.
- **Backpressure**: Slow clients can cause the broadcast buffer to grow, potentially lagging the entire session. Dropping old messages for slow clients is required.
- **Multiplexing**: Using a single WebSocket to carry both CDP events and WebRTC signaling requires a clear framing protocol (e.g., JSON with a `type` field).
- **Latency**: Each layer of fanout adds ~5ms-20ms of end-to-end latency.

## Recommendations
- Implement a server-side "Room" pattern for each browser session.
- Use a non-blocking broadcast mechanism.
- Add a "stale" flag or drop threshold for slow WebSocket clients.
