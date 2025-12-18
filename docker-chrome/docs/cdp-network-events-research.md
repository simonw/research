# CDP Network Events Field Research

## Purpose
Deep dive into the specific fields and structures of CDP network events, with a focus on reliably identifying resource types and navigating the differences between `requestWillBeSent` and `responseReceived`.

## Key Findings
- **ResourceType Field:**
  - Field name is **`type`** (not `resourceType`).
  - Located at the **top level** of the event object.
  - **Optional** in `Network.requestWillBeSent`.
  - **Required** in `Network.responseReceived`.
- **Enum Values:** 19 possible values including `Document`, `XHR`, `Fetch`, `WebSocket`, `EventSource`, etc.
- **Navigation Detection:** A navigation request is identified when `requestId === loaderId` and `type === 'Document'`.

## Important Implementation Details
- **Access Pattern:** Always access `event.type`. In `requestWillBeSent`, check for existence first.
- **Browser Consistency:** The `type` field is consistent across Chromium-based browsers.
- **Puppeteer vs CDP:** Puppeteer exposes a lowercase `resourceType()` method, whereas raw CDP uses the PascalCase enum values directly.

## Conclusion
Reliable type identification depends on checking `event.type` primarily in `responseReceived` where it is guaranteed, or handling its optional nature in `requestWillBeSent`.
