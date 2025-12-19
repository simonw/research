# CDP Network Events Research - Notes

## Research Sources

### Official Documentation
- Chrome DevTools Protocol Network domain: https://chromedevtools.github.io/devtools-protocol/tot/Network/
- Reviewed all Network.* events, types, and methods

### Real-World Implementations
- Playwright (Chromium, Firefox, WebKit network managers)
- Puppeteer (NetworkManager, NetworkEventManager)
- Google Lighthouse (network-recorder.js, network-request.js)
- Node.js Inspector (network_http.js, network_http2.js, network_undici.js)

## Key Findings

### Event Volume Characteristics

From official CDP docs and implementations:

1. **Core Request Lifecycle Events** (per request):
   - `Network.requestWillBeSent` - Always fires first
   - `Network.responseReceived` - When headers arrive
   - `Network.loadingFinished` OR `Network.loadingFailed` - Terminal event
   - `Network.dataReceived` - Can fire 0-N times (for streaming)

2. **ExtraInfo Events** (optional, timing-sensitive):
   - `Network.requestWillBeSentExtraInfo` - Raw headers, cookies, timing
   - `Network.responseReceivedExtraInfo` - Raw response headers, blocked cookies
   - These may arrive BEFORE or AFTER their corresponding base events

3. **Cache Events**:
   - `Network.requestServedFromCache` - Fires for cached requests

### Event Ordering Challenges

From Puppeteer's NetworkEventManager.ts:
```typescript
// Events can arrive out of order:
// requestWillBeSent, requestPaused, requestPaused, ...
// Need to buffer and reconcile
```

From Playwright's crNetworkManager.ts:
```typescript
// Store events in maps for reconciliation:
private _requestIdToRequestWillBeSentEvent = new Map<string, { sessionInfo: SessionInfo, event: Protocol.Network.requestWillBeSentPayload }>();
```

### ResourceType Field

Official CDP types show these values:
- Document, Stylesheet, Image, Media, Font, Script
- TextTrack, XHR, Fetch, Prefetch, EventSource, WebSocket
- Manifest, SignedExchange, Ping, CSPViolationReport, Preflight
- FedCM, Other

### Event Volume Estimates

From Lighthouse network-request.js comments:
- Each request generates minimum 3 events (requestWillBeSent, responseReceived, loadingFinished)
- With ExtraInfo enabled: 5-7 events per request
- With streaming (dataReceived): potentially 10+ events per request
- Typical page load: 50-200 requests = 150-1400+ events

## Best Practices Identified

### 1. Event Buffering & Reconciliation

**Pattern from Puppeteer NetworkEventManager:**
```typescript
#requestWillBeSentMap = new Map<NetworkRequestId, Protocol.Network.RequestWillBeSentEvent>();
#requestPausedMap = new Map<NetworkRequestId, ...>();
#responseReceivedMap = new Map<NetworkRequestId, ...>();
```

**Why:** Events arrive out of order, especially ExtraInfo events

### 2. Request ID Tracking

**Pattern from all implementations:**
- Use `requestId` as primary key
- Handle redirects (new requestId, but linked via `redirectResponse`)
- Track `loaderId` for navigation context

### 3. Filtering Strategies

**By ResourceType (from Lighthouse):**
```javascript
// Filter out noise
if (resourceType === 'Image' || resourceType === 'Font') {
  // Skip or sample
}
```

**By URL Pattern:**
```javascript
// Ignore analytics, tracking
if (url.includes('analytics') || url.includes('tracking')) {
  return;
}
```

### 4. Sampling for High-Volume Events

**dataReceived sampling (from implementations):**
```typescript
// Don't process every dataReceived event
let dataReceivedCount = 0;
if (++dataReceivedCount % 10 === 0) {
  // Process every 10th event
}
```

### 5. Memory Management

**Pattern from Playwright:**
```typescript
_onLoadingFinished(event) {
  const request = this._requestIdToRequest.get(event.requestId);
  // Clean up immediately after terminal event
  this._requestIdToRequest.delete(event.requestId);
  this._requestIdToResponseReceivedPayloadEvent.delete(event.requestId);
}
```

### 6. ExtraInfo Handling

**Pattern from Puppeteer:**
```typescript
// Store ExtraInfo events separately
#requestWillBeSentExtraInfoMap = new Map<NetworkRequestId, Protocol.Network.RequestWillBeSentExtraInfoEvent[]>();

// Reconcile when base event arrives
#onRequestWillBeSent(event) {
  const extraInfos = this.#requestWillBeSentExtraInfoMap.get(event.requestId);
  // Merge data
}
```

### 7. Enable Only What You Need

**From CDP docs:**
```typescript
// Network.enable parameters:
{
  maxTotalBufferSize?: number,      // Limit memory usage
  maxResourceBufferSize?: number,   // Per-resource limit
  maxPostDataSize?: number,         // Limit POST data capture
}
```

### 8. Handle Redirects Properly

**Pattern from all implementations:**
```typescript
_onRequestWillBeSent(event) {
  if (event.redirectResponse) {
    // This is a redirect, handle the response for previous request
    const redirectedFrom = this._requests.get(event.requestId);
    redirectedFrom.handleRedirect(event.redirectResponse);
  }
  // Create new request for the redirect target
}
```

## Performance Considerations

### Event Volume by Page Type

**Simple page (10 requests):**
- ~30 events (3 per request)

**Complex SPA (100 requests):**
- ~300-700 events (with ExtraInfo)

**Streaming application:**
- 1000+ events (many dataReceived)

### Filtering Recommendations

1. **Always filter:**
   - Preflight requests (unless debugging CORS)
   - Ping requests
   - CSPViolationReport

2. **Consider filtering:**
   - Images (unless analyzing performance)
   - Fonts (unless analyzing loading)
   - Analytics/tracking requests

3. **Never filter:**
   - Document requests
   - XHR/Fetch (API calls)
   - Script/Stylesheet (critical resources)

### Sampling Strategies

**Time-based sampling:**
```typescript
const SAMPLE_INTERVAL_MS = 100;
let lastSampleTime = 0;

if (Date.now() - lastSampleTime > SAMPLE_INTERVAL_MS) {
  processEvent(event);
  lastSampleTime = Date.now();
}
```

**Count-based sampling:**
```typescript
const SAMPLE_RATE = 10; // Process 1 in 10
let eventCount = 0;

if (++eventCount % SAMPLE_RATE === 0) {
  processEvent(event);
}
```

## Common Pitfalls

1. **Not handling ExtraInfo timing:**
   - ExtraInfo can arrive before OR after base event
   - Must buffer both directions

2. **Memory leaks:**
   - Forgetting to clean up maps after loadingFinished/loadingFailed
   - Not setting size limits on buffers

3. **Missing redirects:**
   - Redirect creates new requestId
   - Must link via redirectResponse field

4. **Ignoring cache events:**
   - Cached requests may not fire responseReceived
   - Must handle requestServedFromCache

5. **Not filtering enough:**
   - Processing every dataReceived event kills performance
   - Filter by resourceType early

## Implementation Checklist

- [ ] Enable Network domain with appropriate buffer limits
- [ ] Set up event listeners for core events (requestWillBeSent, responseReceived, loadingFinished/Failed)
- [ ] Implement request ID tracking with Maps
- [ ] Handle ExtraInfo events with buffering
- [ ] Implement redirect handling
- [ ] Add resourceType filtering
- [ ] Add URL pattern filtering
- [ ] Implement sampling for high-volume events (dataReceived)
- [ ] Set up memory cleanup on terminal events
- [ ] Handle cache events (requestServedFromCache)
- [ ] Add error handling for malformed events
- [ ] Implement metrics/monitoring for event volume
