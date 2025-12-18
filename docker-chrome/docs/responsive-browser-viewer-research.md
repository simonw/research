# Responsive Browser Viewer Research

## Purpose
This research investigates best practices for creating responsive iframe-based remote browser viewers, specifically focusing on the constraints and behaviors when resizing VNC or WebRTC streams (with a focus on Selkies-GStreamer).

## Key Findings
1.  **Selkies-GStreamer Resizing**:
    *   Supports dynamic resizing via the `SELKIES_ENABLE_RESIZE` environment variable.
    *   Has resolution limits: Standard DVI/HDMI limits apply (practically max 1920x1200 @ 60Hz). Higher resolutions require DisplayPort configuration.
2.  **WebRTC Stream Constraints**:
    *   WebRTC streams can change resolution mid-call, but this depends on the encoder (VP8/H.264) and browser implementation.
    *   Resizing often involves renegotiation or track constraints application (`track.applyConstraints`).
    *   Rapid resizing can cause artifacts or temporary stream interruptions; debouncing resize events is critical.
3.  **Frontend Patterns**:
    *   **ResizeObserver**: The modern standard for detecting container size changes to trigger stream resizing.
    *   **Debounce/Throttle**: Essential to prevent flooding the signaling server or encoder with resize requests during window drag operations.

## Conclusion
For a responsive remote browser experience, the streaming backend must support dynamic resolution changes. On the frontend, a `ResizeObserver` coupled with a debounced resize handler should communicate the new dimensions to the backend. Selkies-GStreamer provides native support for this via environment configuration, making it a strong candidate for this use case.
