# Docker Chrome Plan: Edge Cases & Risks

## Purpose
This research analyzed potential edge cases and failure modes for four planned features in `docker-chrome`:
1.  Network `type` propagation (CDP resource type).
2.  UI type filtering.
3.  Responsive viewport.
4.  Session create/delete lifecycle.

## Key Findings & Mitigations

### 1. Network Type Propagation
*   **Issue:** CDP's `Network.requestWillBeSent` often omits `type`, while `Network.responseReceived` requires it.
*   **Edge Cases:** Redirects, failures (DNS/TLS), CORS preflights, and streaming types (WebSocket/EventSource) can cause inconsistencies or missing data in the UI.
*   **Mitigation:** Treat `responseReceived.type` as canonical. Backfill the request record on the server. Allow `type` to be optional in the UI model. Listen for `Network.loadingFailed` to handle failures gracefully.

### 2. Type Filtering UI
*   **Issue:** Simple filters (e.g., "XHR") can hide related traffic like CORS preflights or WebSocket streams.
*   **Mitigation:** Implement two-level filtering:
    *   **Quick Filters:** `API` (XHR+Fetch+Preflight), `Assets` (Images/CSS/JS), `Realtime` (WS/SSE), `Doc`, `Other`.
    *   **Advanced:** Explicit CDP type multi-select.

### 3. Responsive Viewport
*   **Issue:** The current UI has a fixed mobile viewport. Making it responsive introduces risks of resize loops (thrashing) and performance cost.
*   **Mitigation:**
    *   Use `ResizeObserver` on the client with a 150-300ms debounce.
    *   Only sync with server on meaningful changes (>= 16px).
    *   Enforce server-side clamping (min/max).

### 4. Session Lifecycle
*   **Issue:** The server is currently single-session (global state). Supporting session resets or multiple sessions requires state isolation.
*   **Mitigation:**
    *   Implement "Single Active Session" with a reset endpoint.
    *   **Reset Process:** Close context -> Create new incognito context -> Re-apply scripts -> Start CDP -> Broadcast `SESSION_RESET`.
    *   **Safety:** Tag all events with `sessionId` so the UI can ignore stale events after a reset.

## Conclusion
The analysis provided a implementation checklist prioritizing safety and consistency, particularly emphasizing server-side state management (event merging, session IDs) to prevent UI glitches.
