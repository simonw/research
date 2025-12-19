# CDP Network Events: Best Practices Guide

Comprehensive guide based on official Chrome DevTools Protocol documentation and production implementations from Playwright, Puppeteer, Lighthouse, and Node.js.

## Executive Summary

**Event Volume:** A typical page load generates 150-1400+ network events  
**Key Challenge:** Events arrive out-of-order, especially ExtraInfo events  
**Critical Practice:** Clean up Maps immediately on terminal events to prevent memory leaks  
**Performance Tip:** Filter by ResourceType early, sample dataReceived events  

## Event Lifecycle

### Standard Request (3 events minimum)

```
Network.requestWillBeSent → Network.responseReceived → Network.loadingFinished
```

### With ExtraInfo (5 events)

```
Network.requestWillBeSent ⟷ Network.requestWillBeSentExtraInfo
Network.responseReceived  ⟷ Network.responseReceivedExtraInfo
Network.loadingFinished
```

**⟷ = May arrive in ANY order** - This is the #1 source of bugs

### Redirect Flow

```
Network.requestWillBeSent (original)
Network.responseReceived (3xx redirect)
Network.requestWillBeSent (same requestId, contains redirectResponse field)
Network.responseReceived (final)
Network.loadingFinished
```

## ResourceType Field

**Official CDP values:**
- `Document`, `Stylesheet`, `Image`, `Media`, `Font`, `Script`
- `XHR`, `Fetch`, `EventSource`, `WebSocket`
- `Ping`, `CSPViolationReport`, `Preflight`, `Other`

**Filtering recommendations:**
- ❌ Never filter: `Document`, `XHR`, `Fetch`, `Script`
- ⚠️ Maybe filter: `Image`, `Font`, `Stylesheet`, `Media`
- ✅ Always filter: `Ping`, `CSPViolationReport`, `Preflight`

## Best Practices

### 1. Bidirectional ExtraInfo Buffering

**Problem:** ExtraInfo events may arrive BEFORE or AFTER base events

**Solution from Puppeteer:**

```typescript
const requestBuffer = new Map();
const extraInfoBuffer = new Map();

session.on('Network.requestWillBeSent', (event) => {
  const extraInfo = extraInfoBuffer.get(event.requestId);
  if (extraInfo) {
    processRequest(event, extraInfo);
    extraInfoBuffer.delete(event.requestId);
  } else {
    requestBuffer.set(event.requestId, event);
  }
});

session.on('Network.requestWillBeSentExtraInfo', (event) => {
  const request = requestBuffer.get(event.requestId);
  if (request) {
    processRequest(request, event);
    requestBuffer.delete(event.requestId);
  } else {
    extraInfoBuffer.set(event.requestId, event);
  }
});
```

**Evidence:** [Puppeteer NetworkEventManager.ts](https://github.com/puppeteer/puppeteer/blob/main/packages/puppeteer-core/src/cdp/NetworkEventManager.ts#L74-L90)

### 2. Memory Management - CRITICAL

**Problem:** Maps grow unbounded causing memory leaks

**Solution from Playwright:**

```typescript
_onLoadingFinished(event) {
  const request = this._requestIdToRequest.get(event.requestId);
  if (request) {
    request.finish();
    // CRITICAL: Clean up immediately
    this._requestIdToRequest.delete(event.requestId);
    this._requestIdToRequestWillBeSentEvent.delete(event.requestId);
    this._requestIdToResponseReceivedPayloadEvent.delete(event.requestId);
  }
}

_onLoadingFailed(event) {
  const request = this._requestIdToRequest.get(event.requestId);
  if (request) {
    request.fail(event.errorText);
    // CRITICAL: Clean up immediately
    this._requestIdToRequest.delete(event.requestId);
    this._requestIdToRequestWillBeSentEvent.delete(event.requestId);
    this._requestIdToResponseReceivedPayloadEvent.delete(event.requestId);
  }
}
```

**Evidence:** [Playwright crNetworkManager.ts](https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/chromium/crNetworkManager.ts#L480-L510)

### 3. Enable Network with Buffer Limits

```typescript
await session.send('Network.enable', {
  maxTotalBufferSize: 10 * 1024 * 1024,      // 10MB total
  maxResourceBufferSize: 5 * 1024 * 1024,    // 5MB per resource
  maxPostDataSize: 1024 * 1024,              // 1MB POST data
});
```

**Official docs:** [Network.enable](https://chromedevtools.github.io/devtools-protocol/tot/Network/#method-enable)

### 4. Handle Redirects

```typescript
_onRequestWillBeSent(event) {
  if (event.redirectResponse) {
    // Update previous request with redirect response
    const previous = this._requests.get(event.requestId);
    if (previous) {
      previous.handleRedirect(event.redirectResponse);
    }
  }
  
  // Create new request for redirect target
  const request = new Request(event);
  this._requests.set(event.requestId, request);
}
```

**Evidence:** [Firefox ffNetworkManager.ts](https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/firefox/ffNetworkManager.ts#L55-L75)

### 5. Multi-Level Filtering

```typescript
// Level 1: ResourceType
const IGNORE_TYPES = new Set(['Ping', 'CSPViolationReport', 'Preflight']);
if (IGNORE_TYPES.has(event.type)) return;

// Level 2: URL patterns
const IGNORE_PATTERNS = [
  /google-analytics\.com/,
  /doubleclick\.net/,
  /\.woff2?$/,
];
if (IGNORE_PATTERNS.some(p => p.test(event.request.url))) return;

// Level 3: Sample dataReceived (process 1 in 10)
if (event.method === 'Network.dataReceived') {
  if (++dataCount % 10 !== 0) return;
}
```

### 6. Handle Cache Events

```typescript
onRequestServedFromCache(event) {
  const request = this._requests.get(event.requestId);
  if (request) {
    request.fromMemoryCache = true;
  }
}

onResponseReceived(event) {
  const request = this._requests.get(event.requestId);
  if (request) {
    if (request.fromMemoryCache) {
      request.updateFromCache(event.response);
    } else {
      request.onResponseReceived(event);
    }
  }
}
```

**Evidence:** [Lighthouse network-recorder.js](https://github.com/GoogleChrome/lighthouse/blob/main/core/lib/network-recorder.js#L120-L145)

## Event Volume Estimates

| Page Type | Requests | Events (basic) | Events (ExtraInfo) |
|-----------|----------|----------------|-------------------|
| Simple page | 10-20 | 30-60 | 50-100 |
| Medium SPA | 50-100 | 150-300 | 250-500 |
| Complex app | 100-200 | 300-600 | 500-1000 |
| Streaming app | 50+ | 500-2000+ | 1000-5000+ |

## Sampling Strategies

### Count-Based Sampling

```typescript
class CountSampler {
  private eventCounts = new Map<string, number>();
  private readonly sampleRate: number;
  
  constructor(sampleRate: number = 10) {
    this.sampleRate = sampleRate;
  }
  
  shouldSample(requestId: string): boolean {
    const count = (this.eventCounts.get(requestId) || 0) + 1;
    this.eventCounts.set(requestId, count);
    return count % this.sampleRate === 0;
  }
  
  cleanup(requestId: string) {
    this.eventCounts.delete(requestId);
  }
}
```

### Adaptive Sampling

```typescript
class AdaptiveSampler {
  private eventCounts = new Map<string, number>();
  
  shouldSample(requestId: string): boolean {
    const count = (this.eventCounts.get(requestId) || 0) + 1;
    this.eventCounts.set(requestId, count);
    
    // Adapt sample rate based on event count
    let sampleRate = 1;
    if (count > 100) sampleRate = 20;
    else if (count > 50) sampleRate = 10;
    else if (count > 10) sampleRate = 5;
    
    return count % sampleRate === 0;
  }
}
```

## Common Pitfalls

### ❌ Not Handling ExtraInfo Timing

```typescript
// WRONG: Assumes ExtraInfo arrives after base event
onRequestWillBeSent(event) {
  this.requests.set(event.requestId, event);
}

onRequestWillBeSentExtraInfo(event) {
  const request = this.requests.get(event.requestId);
  request.mergeExtraInfo(event); // May be undefined!
}
```

### ❌ Memory Leaks

```typescript
// WRONG: Never cleans up
onLoadingFinished(event) {
  const request = this.requests.get(event.requestId);
  request.finish();
  // Missing: this.requests.delete(event.requestId);
}
```

### ❌ Missing Redirects

```typescript
// WRONG: Treats redirect as new request
onRequestWillBeSent(event) {
  this.requests.set(event.requestId, new Request(event));
  // Lost the redirect chain!
}
```

### ❌ Ignoring Cache Events

```typescript
// WRONG: Assumes responseReceived always fires
onRequestWillBeSent(event) {
  this.pendingRequests.set(event.requestId, event);
}

onResponseReceived(event) {
  const pending = this.pendingRequests.get(event.requestId);
  this.completeRequest(pending, event);
  // Cached requests may never reach here!
}
```

### ❌ Not Filtering dataReceived

```typescript
// WRONG: Processes every dataReceived event
onDataReceived(event) {
  const request = this.requests.get(event.requestId);
  request.addDataChunk(event.dataLength);
  this.updateUI(request); // Called 100+ times per large file!
}
```

## Implementation Checklist

### Setup
- [ ] Enable Network domain with buffer limits
- [ ] Set up event listeners for core events
- [ ] Initialize Maps for request tracking
- [ ] Initialize Maps for ExtraInfo buffering

### Event Handling
- [ ] Implement bidirectional ExtraInfo buffering
- [ ] Handle redirect chains properly
- [ ] Handle cache events (requestServedFromCache)
- [ ] Implement terminal event cleanup (loadingFinished/Failed)

### Performance
- [ ] Add ResourceType filtering
- [ ] Add URL pattern filtering
- [ ] Implement sampling for dataReceived events
- [ ] Set memory limits on buffers

### Error Handling
- [ ] Handle missing requestId lookups
- [ ] Handle malformed events gracefully
- [ ] Add timeout for orphaned ExtraInfo events
- [ ] Log unexpected event sequences

### Monitoring
- [ ] Track event volume metrics
- [ ] Monitor Map sizes for memory leaks
- [ ] Track ExtraInfo timing mismatches
- [ ] Alert on excessive event rates

## References

### Official Documentation
- [Chrome DevTools Protocol - Network Domain](https://chromedevtools.github.io/devtools-protocol/tot/Network/)

### Production Implementations
- [Playwright - Chromium Network Manager](https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/chromium/crNetworkManager.ts)
- [Puppeteer - Network Manager](https://github.com/puppeteer/puppeteer/blob/main/packages/puppeteer-core/src/cdp/NetworkManager.ts)
- [Puppeteer - Network Event Manager](https://github.com/puppeteer/puppeteer/blob/main/packages/puppeteer-core/src/cdp/NetworkEventManager.ts)
- [Lighthouse - Network Recorder](https://github.com/GoogleChrome/lighthouse/blob/main/core/lib/network-recorder.js)
- [Node.js - Inspector Network](https://github.com/nodejs/node/blob/main/lib/internal/inspector/network_http.js)

---

**Last Updated:** December 2025  
**CDP Version:** Tip-of-Tree (latest)
