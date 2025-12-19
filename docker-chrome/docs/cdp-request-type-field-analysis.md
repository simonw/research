# CDP Request Type Field Analysis Research Summary

## Overview
A deep dive into the `ResourceType` enum in Chromium and how it maps to CDP network events.

## Key Findings
- **ResourceType Enum**: Includes `Document`, `Stylesheet`, `Image`, `Media`, `Font`, `Script`, `TextTrack`, `XHR`, `Fetch`, `EventSource`, `WebSocket`, `Manifest`, `SignedExchange`, `Ping`, `CSPViolationReport`, `Preflight`, `Other`.
- **Timing of Type Availability**: 
  - `requestWillBeSent`: Type is speculative and may change.
  - `responseReceived`: Type is determined by MIME sniffing or headers and is final.
- **Worker Requests**: Requests from Service Workers or Web Workers might have different type assignment patterns and need to be correlated via `frameId`.

## Recommendations
- Use `responseReceived.type` as the source of truth for UI display.
- Map many specific types into broader categories (e.g., `Stylesheet` + `Image` + `Font` -> "Assets").
