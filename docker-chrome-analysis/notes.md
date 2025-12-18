# Docker Chrome Network Capture Investigation Notes

## Investigation Timeline

### Phase 1: Initial Exploration (2025-01-XX)
- Examined docker-chrome folder structure
- Identified key components: server/index.js, control-pane UI, extension
- Found network capture implementation in server/index.js setupNetworkCapture()

### Phase 2: Code Analysis (2025-01-XX)
- Analyzed Network.requestWillBeSent handler (lines 201-208)
- Found missing resource type field (`event.type`)
- Discovered UI expects `type` field but server doesn't provide it
- Identified incomplete field capture (missing headers, request bodies, etc.)

### Phase 3: CDP Research (2025-01-XX)  
- Referenced existing CDP research in cdp-network-events-research/
- Confirmed `event.type` is available in both request/response events
- Found ResourceType enum with 19 possible values
- Verified field locations and requirements

### Phase 4: UI Analysis (2025-01-XX)
- Examined NetworkPanel component expecting `req.type` field
- Found NetworkRequest interface requiring `type: string`
- Confirmed blank "Type" column in UI due to missing data

## Key Findings

### Missing Fields in Current Implementation

**NETWORK_REQUEST payload missing:**
- `type`: ResourceType (Document, XHR, Image, Script, etc.)
- `headers`: Request headers object
- `postData`: Request body for POST/PUT requests

**NETWORK_RESPONSE payload missing:**
- `type`: ResourceType (required field, always available)
- `url`: Final response URL (may differ from request due to redirects)
- `statusText`: HTTP status text ("OK", "Not Found", etc.)

### Edge Cases Identified

1. **Resource Type Classification**: Impossible to categorize requests
2. **Request Debugging**: No headers or POST data visible
3. **Redirect Handling**: Response URL may differ from request URL
4. **Memory Management**: No cleanup on navigation, potential leaks
5. **Error Handling**: Failed requests not captured
6. **Race Conditions**: Separate request/response events may arrive out of order

## Technical Details

### CDP Event Structure Reference
- `Network.requestWillBeSent.type`: Optional ResourceType
- `Network.responseReceived.type`: Required ResourceType  
- Both at top-level of event object, not nested in request/response

### Current Server Logic
```javascript
// Only captures basic fields
broadcast('NETWORK_REQUEST', {
    requestId: event.requestId,
    url: event.request.url,
    method: event.request.method,
    timestamp: Date.now()
});
```

### Required Server Logic
```javascript
// Should capture all available fields
broadcast('NETWORK_REQUEST', {
    requestId: event.requestId,
    url: event.request.url,
    method: event.request.method,
    type: event.type || 'Unknown',
    headers: event.request.headers,
    postData: event.request.postData,
    timestamp: Date.now()
});
```

## Impact Assessment

- **Functional**: Core network monitoring broken (no resource types)
- **UX**: Confusing blank "Type" column
- **Debugging**: Missing critical request/response data
- **Performance**: Potential memory leaks from uncleared response store

## Next Steps Identified

1. **Immediate**: Add `event.type` to both request and response broadcasts
2. **Short-term**: Add request headers and response URL
3. **Medium-term**: Add request bodies and error handling
4. **Long-term**: Improve memory management and add filtering

## Files Examined

- `docker-chrome/server/index.js`: Network capture implementation
- `docker-chrome/control-pane/src/components/network-panel.tsx`: UI display
- `docker-chrome/control-pane/src/lib/types.ts`: Type definitions
- `cdp-network-events-research/README.md`: CDP protocol reference
- `cdp-network-capture/01-basic-network-capture.ts`: Working examples

## Validation Approach

After implementing fixes:
1. Load test page with various resource types
2. Verify "Type" column populated (Document, XHR, Image, Script, etc.)
3. Check request headers available in details view
4. Test POST requests show request body
5. Verify redirect URLs captured correctly
