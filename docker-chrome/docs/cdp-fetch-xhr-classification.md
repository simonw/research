# CDP Fetch vs XHR Classification Research Summary

## Overview
This research focused on distinguishing between `Fetch` and `XHR` requests using Chrome DevTools Protocol (CDP) events, particularly when the `type` field is missing or ambiguous.

## Key Findings
- **`Network.requestWillBeSent.type`**: Often missing in early lifecycle or worker contexts.
- **Canonical Source**: `Network.responseReceived.type` is more reliable and required when it fires.
- **Fallback Logic**: 
  - If `type` is missing, use headers (`X-Requested-With: XMLHttpRequest` for XHR).
  - Use URL extensions as a secondary heuristic.
  - CORS preflights (`OPTIONS`) should be handled separately to avoid skewing latency metrics.

## Recommendations
- Implement server-side consolidation of CDP events by `requestId`.
- Backfill `type` from response events to request records.
- Group "API" requests (Fetch + XHR) in the UI for better developer experience.
