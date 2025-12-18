# CDP Network Events Best Practices

## Purpose
Establish a set of best practices and architectural patterns for handling high-volume CDP network events efficiently and reliably, based on analysis of production implementations (Playwright, Puppeteer, Lighthouse).

## Key Findings
- **Event Volume:** A complex app can generate 500-1000+ events per page load. Streaming apps generate significantly more due to `dataReceived` events.
- **Ordering Issues:** `ExtraInfo` events (e.g., `requestWillBeSentExtraInfo`) often arrive out of order relative to the base event. Buffering and reconciliation are required.
- **Memory Leaks:** Unbounded maps for tracking requests are a common pitfall. Explicit cleanup on `loadingFinished` or `loadingFailed` is critical.
- **Redirects:** Redirects reuse the same `requestId`. Implementations must handle redirect chains by linking the new request to the previous one via `redirectResponse`.

## Recommendations
1. **Buffering:** Implement bidirectional buffering for `ExtraInfo` events.
2. **Filtering:** Apply `ResourceType` and URL pattern filters early to reduce noise.
3. **Sampling:** For high-volume events like `dataReceived`, use time-based or count-based sampling.
4. **Limits:** Use `Network.enable` parameters to set buffer limits (e.g., `maxTotalBufferSize`).

## Code Structure
The recommended architecture involves a `NetworkEventManager` class that handles:
- Event listener registration.
- ID-based request tracking.
- Event reconciliation.
- Terminal event cleanup.

## Conclusion
Robust network monitoring requires a stateful manager to handle protocol quirks (timing, redirects) and defensive coding to prevent memory exhaustion.
