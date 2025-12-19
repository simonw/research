# CDP Network Traffic Capture Research - Notes

## Search Strategy
- Searched official CDP documentation
- Examined Puppeteer and chrome-remote-interface implementations
- Found real-world examples from GitHub (Playwright, Electron, Node.js)
- Reviewed official Chrome DevTools Protocol specification

## Key Findings

### 1. Core Network Events
The CDP Network domain provides these essential events:
- `Network.requestWillBeSent` - Fired when page is about to send HTTP request
- `Network.responseReceived` - Fired when HTTP response is available
- `Network.loadingFinished` - Fired when HTTP request has finished loading
- `Network.loadingFailed` - Fired when HTTP request has failed to load
- `Network.dataReceived` - Fired when data chunk was received

### 2. WebSocket Events
- `Network.webSocketCreated` - Fired upon WebSocket creation
- `Network.webSocketWillSendHandshakeRequest` - Fired when WebSocket is about to initiate handshake
- `Network.webSocketHandshakeResponseReceived` - Fired when WebSocket handshake response becomes available
- `Network.webSocketFrameSent` - Fired when WebSocket message is sent
- `Network.webSocketFrameReceived` - Fired when WebSocket message is received
- `Network.webSocketClosed` - Fired when WebSocket is closed
- `Network.webSocketFrameError` - Fired when WebSocket message error occurs

### 3. Getting Response Bodies
- Use `Network.getResponseBody` method with requestId
- Must wait for `Network.loadingFinished` event before calling
- Returns body as string and base64Encoded flag
- Can fail for certain resource types (cached, redirected, etc.)

## Implementation Patterns Found

### Pattern 1: Puppeteer Approach
- Uses NetworkManager class to coordinate events
- Tracks requests by requestId
- Handles redirects and extra info events
- Source: https://github.com/puppeteer/puppeteer/blob/main/packages/puppeteer-core/src/cdp/NetworkManager.ts

### Pattern 2: chrome-remote-interface Approach
- Simple event-based API
- Direct CDP protocol mapping
- Minimal abstraction layer
- Source: https://github.com/cyrus-and/chrome-remote-interface

### Pattern 3: Playwright Approach
- Separate network managers for Chromium, Firefox, WebKit
- Sophisticated request/response correlation
- Handles COOP (Cross-Origin-Opener-Policy) edge cases
- Source: https://github.com/microsoft/playwright

## Libraries Comparison

| Library | Abstraction Level | WebSocket Support | Response Body | Complexity |
|---------|------------------|-------------------|---------------|------------|
| chrome-remote-interface | Low | Yes | Manual | Low |
| Puppeteer | Medium | Yes | Built-in | Medium |
| Playwright | High | Yes | Built-in | High |

## Important Gotchas
1. Must enable Network domain before events fire: `Network.enable()`
2. Response bodies only available after `loadingFinished`
3. Some responses can't be retrieved (304, cached, etc.)
4. WebSocket frames contain base64-encoded data for binary messages
5. requestId is consistent across redirects
6. Extra info events (`requestWillBeSentExtraInfo`, `responseReceivedExtraInfo`) provide additional headers/cookies

## Next Steps
- Implement basic CDP network capture
- Test with WebSocket connections
- Handle edge cases (redirects, failures, caching)
