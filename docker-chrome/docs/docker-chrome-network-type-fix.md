# Docker Chrome Network Type Field Investigation

## Purpose of the Research
To investigate why the 'type' field in NetworkRequest objects is empty and implement a fix to extract the resource type from CDP events properly.

## Key Findings/Notes

### Problem
The 'type' field in NetworkRequest objects was empty because the CDP event handler was not extracting the resource type from `Network.requestWillBeSent` events.

### Root Cause
The implementation extracted `event.request.url` and `event.request.method` but missed `event.type`, which contains the resource type (e.g., Document, Stylesheet, Image).

### Solution
Add `type: event.type` to the `NETWORK_REQUEST` broadcast in the CDP event handler in `docker-chrome/server/index.js`.

## Important Code Snippets/Structures

**Fix Implementation (`docker-chrome/server/index.js`):**
```javascript
client.on('Network.requestWillBeSent', (event) => {
    broadcast('NETWORK_REQUEST', {
        requestId: event.requestId,
        url: event.request.url,
        method: event.request.method,
        type: event.type, // Added this field
        timestamp: Date.now()
    });
});
```

**CDP Event Structure:**
The `Network.requestWillBeSent` event includes a top-level `type` field:
```json
{
  "requestId": "string",
  "request": { ... },
  "type": "ResourceType", // Missing field
  ...
}
```

## Conclusion/Next Steps
The fix involves a minor modification to `docker-chrome/server/index.js` to extract and broadcast the `type` field. This enables better filtering and categorization of network requests in the control panel.
