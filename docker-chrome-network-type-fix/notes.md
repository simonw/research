# Network Request Capture Investigation

## Current Implementation Analysis

### CDP Events Being Listened To:
- ✅ Network.requestWillBeSent
- ✅ Network.responseReceived  
- ✅ Network.loadingFinished

### Fields Being Extracted:

**Network.requestWillBeSent:**
- ✅ requestId
- ✅ url  
- ✅ method
- ✅ timestamp
- ❌ **MISSING: type** (resource type like 'Document', 'Stylesheet', 'Image', etc.)

**Network.responseReceived:**
- ✅ requestId
- ✅ status
- ✅ mimeType
- ✅ headers
- ✅ timestamp

**Network.loadingFinished:**
- ✅ requestId
- ✅ response body

### NetworkRequest Type Definition:
```typescript
export interface NetworkRequest {
  requestId: string;
  url: string;
  method: string;
  status?: number;
  type: string;  // <-- This field exists but is never populated
  timestamp: number;
  responseBody?: string;
  responseHeaders?: Record<string, string>;
}
```

## Root Cause

The CDP `Network.requestWillBeSent` event includes a top-level `type` field that indicates the resource type, but the current handler doesn't extract it:

```javascript
client.on('Network.requestWillBeSent', (event) => {
    broadcast('NETWORK_REQUEST', {
        requestId: event.requestId,
        url: event.request.url,
        method: event.request.method,
        timestamp: Date.now()
        // MISSING: type: event.type
    });
});
```

## CDP Protocol Reference

According to Chrome DevTools Protocol, the `Network.requestWillBeSent` event has this structure:

```json
{
  "requestId": "string",
  "loaderId": "string", 
  "documentURL": "string",
  "request": {
    "url": "string",
    "method": "string",
    // ... other request fields
  },
  "timestamp": "number",
  "wallTime": "number",
  "initiator": {...},
  "redirectHasExtraInfo": "boolean",
  "redirectResponse": {...},
  "type": "ResourceType",  // <-- This is the field we're missing
  "frameId": "string",
  "hasUserGesture": "boolean"
}
```

Where `ResourceType` can be:
- Document, Stylesheet, Image, Media, Font, Script, TextTrack, XHR, Fetch, Prefetch, EventSource, WebSocket, Manifest, SignedExchange, Ping, CSPViolationReport, Preflight, FedCM, Other

## Solution

Add the missing `type` field extraction:

```javascript
client.on('Network.requestWillBeSent', (event) => {
    broadcast('NETWORK_REQUEST', {
        requestId: event.requestId,
        url: event.request.url,
        method: event.request.method,
        type: event.type,  // <-- ADD THIS LINE
        timestamp: Date.now()
    });
});
```

## Testing

After implementing the fix:
1. Load a web page in the Docker Chrome instance
2. Check network requests in the control panel
3. Verify that the 'type' field now shows proper resource types instead of empty strings

## Impact

This fix will provide better network request classification and filtering capabilities in the Docker Chrome control panel.
