# Network Request Filtering UI Research

## Purpose
Researched best practices and patterns for filtering network requests in developer tools to design the filtering UI for the Docker Chrome Control Pane.

## Key Findings

### Filter Categories
Instead of exposing all raw CDP types, grouping them is more user-friendly:
*   **API:** XHR, Fetch
*   **Assets:** CSS, Image, Font, Media
*   **Docs:** Document (HTML)
*   **Realtime:** WebSocket, EventSource
*   **Other:** Everything else

### UI Patterns
*   **Button Groups/Chips:** Best for quick, mutually exclusive or simple multi-select toggles.
*   **Dropdowns:** Better for complex or rarely used filters to save space.
*   **Search/Text:** Essential for domain or name filtering.

### Implementation Strategy
1.  **Grouping Logic:** Map internal `ResourceType` (from CDP or MIME sniffer) to high-level filter groups.
2.  **Multi-select:** Allow users to see "API" and "Docs" simultaneously.
3.  **Visual Feedback:** Show counts (e.g., "X of Y requests") to indicate filtering is active.
4.  **Component:** A `NetworkFilter` component using a `<details>` dropdown with checkboxes was chosen for the implementation to balance capability with screen real estate.

## Outcome
These findings directly informed the implementation of the `NetworkFilter` component in the `docker-chrome` project.
