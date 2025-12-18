# CDP Network Capture Research

## Purpose
Investigate methods for capturing network traffic using the Chrome DevTools Protocol (CDP), specifically within the context of Playwright and Puppeteer, to support HAR export and advanced network analysis.

## Key Findings
- **Key Libraries:** Playwright and Puppeteer are the primary high-level tools, both exposing low-level CDP sessions.
- **Core CDP Events:**
  - `Network.requestWillBeSent`: Initial request details.
  - `Network.responseReceived`: Response headers.
  - `Network.loadingFinished` / `Network.loadingFailed`: Request completion status.
  - `Network.getResponseBody`: Method to fetch content.
- **Session Management:**
  - Playwright allows attaching CDP sessions to pages (`page.context().newCDPSession(page)`) or browsers.
  - Connection to existing Chrome instances is supported via `connectOverCDP`.

## Implementation Patterns
The research identified a standard pattern for capturing bodies:
1. Create CDP session.
2. Enable Network domain: `await cdpSession.send('Network.enable')`.
3. Listen for `requestWillBeSent` to track IDs.
4. On completion, call `Network.getResponseBody` with the specific `requestId`.

## Conclusion
Standard CDP events provide sufficient granularity for full network capture. The primary challenge is managing the correlation between request IDs and handling the asynchronous nature of response body retrieval.
