# Session Reset Implementation

## Purpose
Implemented a mechanism to reset the browser session (clearing state, cookies, and history) directly from the UI, ensuring a "fresh" testing environment without restarting the container.

## Key Implementation Details

### Backend & WebSocket
*   **Session Isolation:** Introduced a `sessionId`. The frontend tracks the active ID and ignores events from stale sessions (e.g., those arriving immediately after a reset).
*   **Event Merging:** Network events (`REQUEST`, `RESPONSE`, `FAILED`) are merged by `requestId` on the client. This ensures that if a response arrives before the request processing completes, data is not lost.
*   **Reset Logic:**
    *   Triggers via `POST /api/session/reset`.
    *   Server closes the current context and opens a new one.
    *   Broadcasts `SESSION_RESET` event.

### Frontend (React)
*   **UI Controls:** Added a "Reset Session" button with a confirmation dialog to the System Status panel.
*   **State Management:**
    *   Listens for `SESSION_RESET` to clear the network request log immediately.
    *   Uses `useRef` for `activeSessionId` to handle WebSocket messages without stale closures.
    *   Caps the request log at 200 items to prevent memory leaks.

## Code Snippet: Event Merging Strategy
```typescript
// Inside WebSocket message handler
const existsIndex = prev.findIndex(r => r.requestId === payload.requestId);
if (existsIndex !== -1) {
  // Merge into existing record
  const updated = [...prev];
  updated[existsIndex] = { ...updated[existsIndex], ...payload };
  return updated;
}
// Append new record
```

## Status
Feature is fully implemented and verified.
