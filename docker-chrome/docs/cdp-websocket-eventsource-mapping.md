# CDP WebSocket and EventSource Mapping

## Purpose
Map the distinct lifecycles of WebSocket and EventSource (Server-Sent Events) connections to specific CDP Network domain events to ensure accurate UI representation and data capture.

## Key Findings

### WebSocket Lifecycle
1. **Creation:** `Network.webSocketCreated` (Signals start).
2. **Handshake:** 
   - `Network.webSocketWillSendHandshakeRequest`
   - `Network.webSocketHandshakeResponseReceived`
3. **Traffic:** `Network.webSocketFrameSent` / `Network.webSocketFrameReceived` (includes OpCode for Text/Binary/Control).
4. **Closure:** `Network.webSocketClosed`.
*Note:* An initial HTTP upgrade request (`Network.requestWillBeSent` with `type: 'WebSocket'`) may also occur.

### EventSource (SSE) Lifecycle
1. **Start:** `Network.requestWillBeSent` (type: `'EventSource'`).
2. **Connection:** `Network.responseReceived` (type: `'EventSource'`, MIME: `text/event-stream`).
3. **Messages:** `Network.eventSourceMessageReceived` (contains `eventName`, `eventId`, `data`).

## UI Recommendations
- **Grouping:** Use `requestId` to group all related events (handshake, frames, messages) under a single persistent connection entry.
- **Decoding:** WebSocket binary frames need Base64 decoding. SSE messages usually need JSON parsing.
- **Status:** specific event sequences determine 'Connecting', 'Open', and 'Closed' states.

## Conclusion
WebSockets and SSE require specialized handling distinct from standard HTTP requests. The `requestId` is the reliable link across the disparate events for both protocols.
