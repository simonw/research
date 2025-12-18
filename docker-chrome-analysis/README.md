# Docker Chrome Network Capture Analysis

## Executive Summary

The docker-chrome project implements network capture via Chrome DevTools Protocol (CDP), but has critical gaps in field collection and resource type handling. The UI expects resource type information that the server fails to provide.

## Critical Issues Found

### 1. Missing Resource Type Field
- **Problem**: UI displays empty "Type" column for all requests
- **Root Cause**: Server doesn't capture `event.type` from CDP events
- **Impact**: Cannot categorize requests (XHR, Document, Image, Script, etc.)

### 2. Incomplete Request Data
- **Missing**: Request headers, POST body data, resource type
- **Available**: Only requestId, URL, method, timestamp

### 3. Incomplete Response Data  
- **Missing**: Response URL (may differ from request due to redirects), status text
- **Available**: Headers, status code, mimeType, body

## Network Event Flow Analysis

### Network.requestWillBeSent Handler (server/index.js:201-208)
```javascript
client.on('Network.requestWillBeSent', (event) => {
    broadcast('NETWORK_REQUEST', {
        requestId: event.requestId,
        url: event.request.url,
        method: event.request.method,
        timestamp: Date.now()
    });
});
```

**Missing Fields Available in CDP Event:**
- `event.type` → ResourceType (Document, XHR, Image, Script, etc.)
- `event.request.headers` → Request headers
- `event.request.postData` → Request body for POST/PUT

### Network.responseReceived Handler (server/index.js:210-232)
```javascript
client.on('Network.responseReceived', (event) => {
    // Store response metadata
    responseStore.set(event.requestId, {
        headers: event.response.headers,
        status: event.response.status,
        mimeType: event.response.mimeType
    });

    broadcast('NETWORK_RESPONSE', {
        requestId: event.requestId,
        status: event.response.status,
        mimeType: event.response.mimeType,
        timestamp: Date.now()
    });
});
```

**Missing Fields Available in CDP Event:**
- `event.type` → ResourceType (required field, always present)
- `event.response.url` → Final URL (may differ from request due to redirects)
- `event.response.statusText` → Status text ("OK", "Not Found", etc.)

## UI Expectations vs Server Reality

### UI NetworkRequest Interface (control-pane/src/lib/types.ts)
```typescript
export interface NetworkRequest {
  requestId: string;
  url: string;
  method: string;
  status?: number;
  type: string;        // ← EXPECTED but MISSING
  timestamp: number;
  responseBody?: string;
  responseHeaders?: Record<string, string>;
}
```

### Server NETWORK_REQUEST Payload
```javascript
{
  requestId: event.requestId,
  url: event.request.url,
  method: event.request.method,
  timestamp: Date.now()
  // type: event.type ← MISSING
}
```

### UI Display (network-panel.tsx:98)
```tsx
<div className="col-span-2 text-zinc-500 truncate">
  {req.type}  // ← Always undefined/empty
</div>
```

## Edge Cases & Failure Modes

### 1. Resource Type Classification Impossible
- Cannot distinguish between navigation requests, API calls, static assets
- UI shows blank "Type" column for all requests
- No way to filter requests by type (XHR only, images only, etc.)

### 2. Request/Response Correlation Issues
- No request headers available for debugging
- Cannot see POST data or request payloads
- Response URL may differ from request URL (redirects) but only request URL shown

### 3. Memory Management Issues
- responseStore limited to 200 items with FIFO eviction
- No cleanup on page navigation - stale data persists
- Potential memory leaks with large response bodies

### 4. Error Handling Gaps
- No handling of Network.loadingFailed events
- Failed requests not visible in UI
- No indication when response bodies are unavailable

### 5. Race Conditions
- NETWORK_REQUEST and NETWORK_RESPONSE sent separately
- UI must correlate by requestId
- Potential timing issues if events arrive out of order

## Recommended Fixes

### Immediate (Critical)
1. **Add Resource Type**: Capture `event.type` from both request/response events
2. **Add Request Headers**: Include `event.request.headers` in NETWORK_REQUEST
3. **Add Response URL**: Include `event.response.url` in NETWORK_RESPONSE

### Medium Priority
4. **Add Request Body**: Capture POST data when available
5. **Add Status Text**: Include `event.response.statusText`
6. **Handle Failed Requests**: Listen to Network.loadingFailed events

### Long-term
7. **Improve Memory Management**: Clear responseStore on navigation
8. **Add Request Filtering**: Allow blocking/filtering by resource type
9. **Performance Monitoring**: Track request timing and sizes

## Code Changes Required

### server/index.js - NETWORK_REQUEST broadcast
```javascript
broadcast('NETWORK_REQUEST', {
    requestId: event.requestId,
    url: event.request.url,
    method: event.request.method,
    type: event.type || 'Unknown',  // ← ADD THIS
    headers: event.request.headers, // ← ADD THIS
    timestamp: Date.now()
});
```

### server/index.js - NETWORK_RESPONSE broadcast  
```javascript
broadcast('NETWORK_RESPONSE', {
    requestId: event.requestId,
    status: event.response.status,
    mimeType: event.response.mimeType,
    type: event.type,  // ← ADD THIS (required field)
    url: event.response.url, // ← ADD THIS
    statusText: event.response.statusText, // ← ADD THIS
    timestamp: Date.now()
});
```

## Impact Assessment

- **Severity**: High - Core functionality broken (resource type classification)
- **Scope**: Affects all network monitoring use cases
- **User Experience**: Blank "Type" column makes UI confusing
- **Debugging**: Missing critical request/response data hinders troubleshooting

## Verification Steps

After fixes:
1. Load a web page and check Network panel
2. Verify "Type" column shows values like "Document", "XHR", "Image", "Script"
3. Check that request headers are available
4. Verify response URLs match expectations (especially for redirects)
5. Test with POST requests to ensure request bodies are captured
# Docker Chrome Selkies WebRTC Analysis

## Executive Summary

This analysis examines the docker-chrome project's Selkies GStreamer WebRTC configuration, focusing on viewport/resize hooks, component architecture, and performance characteristics.

## Key Findings

### 1. Selkies Integration Status: PARTIAL

**Current State**: The project references Selkies GStreamer but implements only partial integration.

**Evidence**:
- README mentions "selkies-gstreamer + Chromium" architecture
- Dockerfile builds from `linuxserver/chromium` base, not full Selkies
- No dedicated Selkies server process in supervisord config
- WebRTC handled via Node.js proxy instead of native Selkies pipeline

**Missing Components**:
- GStreamer backend (`/opt/gstreamer`)
- Python signaling server (`selkies_gstreamer`)
- HTML5 web client (`/opt/gst-web`)
- Proper encoder configuration

### 2. Viewport/Resize Implementation

**Location**: `docker-chrome/server/index.js:184`
```javascript
await page.setViewportSize({ width: 375, height: 667 });
```

**Current Behavior**:
- **Static viewport**: Hardcoded to iPhone SE dimensions (375×667px)
- **No dynamic resizing**: Fixed size regardless of client window
- **No resize hooks**: No mechanism for client-driven viewport changes

**Where Implemented**:
- In `connectToBrowser()` function during Chrome CDP connection
- Applied immediately after page creation
- No subsequent viewport adjustments

### 3. Component Architecture

**Server Layer** (`server/index.js`):
- Node.js Express server with WebSocket support
- CDP connection to Chrome (--remote-debugging-port=9222)
- WebRTC proxying and network capture
- Viewport configuration

**Browser Layer** (`src/components/browser-frame.tsx`):
- React iframe component for stream display
- Fixed aspect ratio (16:9) regardless of actual viewport
- No resize event handling

**Chrome Layer**:
- Runs with extension injection (`--load-extension=/opt/extension`)
- Supervisord managed with Xvfb display server
- No GPU acceleration configured

### 4. Performance Issues & Mitigation

**Current Performance Issues**:

1. **Software Encoding Bottleneck**
   - No GPU acceleration (NVENC/VA-API)
   - CPU-bound H.264 encoding
   - High latency and low frame rates

2. **Resolution Limitations**
   - Fixed 375×667 viewport (iPhone SE)
   - No adaptive resolution
   - Poor quality for modern displays

3. **Missing Performance Knobs**
   - No bitrate configuration
   - No framerate control
   - No encoder selection

**Mitigation Strategies** (from Selkies research):

1. **GPU Acceleration**:
   ```bash
   -e SELKIES_ENCODER=nvh264enc  # NVIDIA NVENC
   -e SELKIES_ENCODER=vah264enc  # VA-API (Intel/AMD)
   --gpus 1 --runtime nvidia     # GPU passthrough
   ```

2. **Resolution Optimization**:
   ```bash
   -e SELKIES_ENABLE_RESIZE=true  # Dynamic resize
   -e DISPLAY_SIZEW=1920         # Base resolution
   -e DISPLAY_SIZEH=1080
   ```

3. **Bitrate/Quality Control**:
   ```bash
   -e SELKIES_VIDEO_BITRATE=8000    # 8 Mbps
   -e SELKIES_FRAMERATE=60          # 60 FPS
   -e SELKIES_AUDIO_BITRATE=128000  # 128 kbps
   ```

## Architecture Comparison

### Current Implementation
```
Client Browser → iframe → Node.js Server → CDP → Chrome (375×667 viewport)
```

### Full Selkies Architecture (Missing)
```
Client Browser → WebRTC → NGINX → Python Server → GStreamer → X11 → Chrome
```

## Recommendations

### Immediate Fixes
1. **Complete Selkies Integration**: Implement full Selkies stack instead of partial proxy
2. **Dynamic Viewport**: Add client-side resize detection and server-side viewport updates
3. **GPU Support**: Configure hardware encoding for better performance

### Performance Optimizations
1. **Adaptive Streaming**: Match resolution/bitrate to client capabilities
2. **TURN Server**: Configure for production NAT traversal
3. **Resource Limits**: Set CPU/memory constraints for predictable performance

### Architecture Improvements
1. **Proper WebRTC Pipeline**: Use native Selkies GStreamer instead of Node.js proxy
2. **Configurable Viewport**: Make viewport responsive to client window changes
3. **Performance Monitoring**: Add metrics for latency, frame rate, and bandwidth usage

## Investigation Methodology

This analysis was conducted through:
- Code examination of docker-chrome repository
- Cross-reference with Selkies GStreamer documentation
- Architecture pattern analysis
- Performance bottleneck identification
- Component interaction mapping

## Files Analyzed
- `docker-chrome/server/index.js` - Core server logic
- `docker-chrome/Dockerfile` - Container configuration
- `docker-chrome/scripts/supervisord.conf` - Process management
- `selkies-gstreamer-research/` - Reference documentation
- `docker-chrome/src/components/browser-frame.tsx` - Client display

## Conclusion

The docker-chrome project shows promise but has significant gaps in Selkies integration and performance optimization. The current viewport implementation is rudimentary, and the WebRTC streaming lacks the full performance benefits of the Selkies GStreamer pipeline. Completing the Selkies integration and implementing dynamic viewport resizing would significantly improve the system's capabilities and performance.
