# CDP WebSocket and EventSource Mapping - Official Reference

**Source**: Chrome DevTools Protocol Official Specification  
**URL**: https://chromedevtools.github.io/devtools-protocol/tot/Network/  
**Last Updated**: December 2025

## Executive Summary

WebSockets and EventSource (Server-Sent Events) have **dedicated ResourceType values** and **specific CDP events** in the Network domain:

- **WebSocket**: `type: 'WebSocket'` with 7 dedicated events
- **EventSource**: `type: 'EventSource'` with 1 dedicated event

## ResourceType Enum (Official)

```typescript
type ResourceType = 
  | 'Document' | 'Stylesheet' | 'Image' | 'Media' | 'Font'
  | 'Script' | 'TextTrack' | 'XHR' | 'Fetch' | 'Prefetch'
  | 'EventSource'    // ← SSE connections
  | 'WebSocket'      // ← WebSocket connections
  | 'Manifest' | 'SignedExchange' | 'Ping' 
  | 'CSPViolationReport' | 'Preflight' | 'FedCM' | 'Other'
```

## WebSocket Events (7 Total)

### 1. `Network.webSocketCreated`
**Fired**: Upon WebSocket creation  
**Parameters**:
- `requestId` (RequestId): Unique identifier
- `url` (string): WebSocket URL (ws:// or wss://)
- `initiator` (Initiator): What created the WebSocket

### 2. `Network.webSocketWillSendHandshakeRequest`
**Fired**: Before sending handshake  
**Parameters**:
- `requestId` (RequestId)
- `timestamp` (MonotonicTime)
- `wallTime` (TimeSinceEpoch)
- `request` (WebSocketRequest): Headers

### 3. `Network.webSocketHandshakeResponseReceived`
**Fired**: Handshake response received  
**Parameters**:
- `requestId` (RequestId)
- `timestamp` (MonotonicTime)
- `response` (WebSocketResponse): Status 101, headers

### 4. `Network.webSocketFrameSent`
**Fired**: WebSocket message sent  
**Parameters**:
- `requestId` (RequestId)
- `timestamp` (MonotonicTime)
- `response` (WebSocketFrame): opcode, payloadData

### 5. `Network.webSocketFrameReceived`
**Fired**: WebSocket message received  
**Parameters**:
- `requestId` (RequestId)
- `timestamp` (MonotonicTime)
- `response` (WebSocketFrame): opcode, payloadData

### 6. `Network.webSocketFrameError`
**Fired**: WebSocket error  
**Parameters**:
- `requestId` (RequestId)
- `timestamp` (MonotonicTime)
- `errorMessage` (string)

### 7. `Network.webSocketClosed`
**Fired**: WebSocket closed  
**Parameters**:
- `requestId` (RequestId)
- `timestamp` (MonotonicTime)

## EventSource Event (1 Total)

### `Network.eventSourceMessageReceived`
**Fired**: SSE message received  
**Parameters**:
- `requestId` (RequestId)
- `timestamp` (MonotonicTime)
- `eventName` (string): Event type (e.g., 'message', 'update')
- `eventId` (string): Message ID from SSE `id:` field
- `data` (string): Message content from SSE `data:` field

## Event Sequences

### WebSocket Lifecycle

```
Network.webSocketCreated
  ↓
Network.requestWillBeSent (type: 'WebSocket')  [may not fire]
  ↓
Network.webSocketWillSendHandshakeRequest
  ↓
Network.webSocketHandshakeResponseReceived
  ↓
Network.webSocketFrameSent / Network.webSocketFrameReceived  [multiple]
  ↓
Network.webSocketClosed
```

### EventSource Lifecycle

```
Network.requestWillBeSent (type: 'EventSource')
  ↓
Network.responseReceived (type: 'EventSource')
  ↓
Network.eventSourceMessageReceived  [multiple]
  ↓
Network.loadingFinished or Network.loadingFailed
```

## WebSocket Frame Opcodes

| Opcode | Type   | Description                    |
|--------|--------|--------------------------------|
| 1      | Text   | UTF-8 text message             |
| 2      | Binary | Binary data (base64 in CDP)    |
| 8      | Close  | Connection close frame         |
| 9      | Ping   | Heartbeat ping                 |
| 10     | Pong   | Heartbeat pong response        |

## UI Grouping Guidance

### Detection Logic

```typescript
function getConnectionType(event: NetworkEvent): 'WebSocket' | 'EventSource' | 'HTTP' {
  // Check event method
  if (event.method === 'Network.webSocketCreated') {
    return 'WebSocket';
  }
  
  if (event.method === 'Network.eventSourceMessageReceived') {
    return 'EventSource';
  }
  
  // Check ResourceType
  if (event.method === 'Network.requestWillBeSent' || 
      event.method === 'Network.responseReceived') {
    const type = event.params.type;
    if (type === 'WebSocket') return 'WebSocket';
    if (type === 'EventSource') return 'EventSource';
  }
  
  return 'HTTP';
}
```

### Grouping Strategy

**WebSocket**:
- Group all events by `requestId`
- Show as single persistent connection
- Nest handshake, frames, and close under connection
- Display frame count and connection status

**EventSource**:
- Group all events by `requestId`
- Show as single persistent connection
- Nest all SSE messages under connection
- Display message count and event types

### Filter Chips

```typescript
const NETWORK_FILTERS = {
  all: { label: 'All', types: ['*'] },
  doc: { label: 'Doc', types: ['Document'] },
  xhr: { label: 'XHR', types: ['XHR', 'Fetch'] },
  js: { label: 'JS', types: ['Script'] },
  css: { label: 'CSS', types: ['Stylesheet'] },
  img: { label: 'Img', types: ['Image'] },
  media: { label: 'Media', types: ['Media', 'Font'] },
  ws: { label: 'WS', types: ['WebSocket'], icon: '🔌' },
  sse: { label: 'SSE', types: ['EventSource'], icon: '📡' },
  other: { label: 'Other', types: ['Manifest', 'Ping', 'Other'] }
}
```

### Suggested UI Structure

```
🔌 WebSocket: wss://example.com/socket [requestId: 1234.5]
  ├─ 📤 Handshake Request
  ├─ 📥 Handshake Response (101 Switching Protocols)
  ├─ 💬 Messages (23 frames)
  │   ├─ ⬆️ [12:34:56] Text: "Hello"
  │   ├─ ⬇️ [12:34:57] Text: "Welcome"
  │   └─ ⬆️ [12:34:58] Binary: [128 bytes]
  └─ ❌ Closed at 12:35:00

📡 EventSource: https://example.com/events [requestId: 5678.9]
  ├─ 📤 Request
  ├─ 📥 Response (200 OK, text/event-stream)
  └─ 📨 Messages (15 events)
      ├─ [12:40:01] message: "First update"
      ├─ [12:40:05] update [id:42]: {"status":"ok"}
      └─ [12:40:10] heartbeat [id:43]: "ping"
```

## Request ID Tracking

**Critical**: The same `requestId` links all events for a connection.

```typescript
interface ConnectionTracker {
  requestId: string;
  type: 'WebSocket' | 'EventSource';
  url: string;
  startTime: number;
  status: 'pending' | 'open' | 'closed' | 'failed';
  
  // WebSocket-specific
  frames?: WebSocketFrame[];
  
  // EventSource-specific
  messages?: EventSourceMessage[];
}

const connections = new Map<string, ConnectionTracker>();

// Track by requestId
function onWebSocketCreated(params: WebSocketCreatedEvent) {
  connections.set(params.requestId, {
    requestId: params.requestId,
    type: 'WebSocket',
    url: params.url,
    startTime: Date.now(),
    status: 'pending',
    frames: []
  });
}

function onWebSocketFrame(params: WebSocketFrameReceivedEvent) {
  const conn = connections.get(params.requestId);
  if (conn?.type === 'WebSocket') {
    conn.frames.push(params.response);
  }
}
```

## Important Notes

1. **WebSocket `requestWillBeSent`**: May or may not fire. Always use `webSocketCreated` as primary signal.

2. **EventSource Reconnection**: Each reconnection gets a NEW `requestId`. Track by URL to group reconnections.

3. **Binary WebSocket Frames**: `payloadData` is base64 encoded for opcode 2. Must decode before display.

4. **EventSource MIME Type**: Should be `text/event-stream`. Check `response.mimeType` in `responseReceived`.

5. **Frame Concatenation**: Multiple SSE `data:` lines are concatenated with `\n` in the `data` field.

## Real-World Implementation Examples

### Playwright (Chromium)
```typescript
eventsHelper.addEventListener(session, 'Network.webSocketCreated', 
  e => this._page!.frameManager.onWebSocketCreated(e.requestId, e.url));
eventsHelper.addEventListener(session, 'Network.webSocketFrameReceived', 
  e => this._page!.frameManager.webSocketFrameReceived(e.requestId, e.response.opcode, e.response.payloadData));
```

### Node.js Inspector
```typescript
addListener(event: "Network.webSocketCreated", 
  listener: (message: InspectorNotification<Network.WebSocketCreatedEventDataType>) => void): this;
```

## TypeScript Type Definitions

```typescript
namespace Protocol.Network {
  type ResourceType = 
    | 'Document' | 'Stylesheet' | 'Image' | 'Media' | 'Font'
    | 'Script' | 'TextTrack' | 'XHR' | 'Fetch' | 'Prefetch'
    | 'EventSource' | 'WebSocket' | 'Manifest' | 'SignedExchange'
    | 'Ping' | 'CSPViolationReport' | 'Preflight' | 'FedCM' | 'Other';

  interface WebSocketCreatedEvent {
    requestId: RequestId;
    url: string;
    initiator?: Initiator;
  }

  interface WebSocketFrameReceivedEvent {
    requestId: RequestId;
    timestamp: MonotonicTime;
    response: WebSocketFrame;
  }

  interface WebSocketFrame {
    opcode: number;
    mask: boolean;
    payloadData: string;
  }

  interface EventSourceMessageReceivedEvent {
    requestId: RequestId;
    timestamp: MonotonicTime;
    eventName: string;
    eventId: string;
    data: string;
  }
}
```

## References

- **Official CDP Spec**: https://chromedevtools.github.io/devtools-protocol/tot/Network/
- **devtools-protocol npm**: https://www.npmjs.com/package/devtools-protocol
- **WebSocket RFC 6455**: https://tools.ietf.org/html/rfc6455
- **SSE Spec**: https://html.spec.whatwg.org/multipage/server-sent-events.html
- **Playwright**: https://github.com/microsoft/playwright
- **Puppeteer**: https://github.com/puppeteer/puppeteer
