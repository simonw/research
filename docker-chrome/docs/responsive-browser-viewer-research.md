# Responsive Browser Viewer Research Summary

## Overview
Research into making the remote browser viewer (canvas/video stream) responsive across different devices and window sizes.

## Key Findings
- **Client-Side Scaling**: The simplest approach is to use CSS `object-fit: contain` or `fill` on the video/canvas element. This is zero-overhead for the server.
- **Server-Side Resizing**: Using `page.setViewportSize` allows for true "responsive" behavior where the page layout adapts. However, this triggers re-renders and potential layout shifts.
- **Interaction Mapping**: Mouse/touch coordinates must be re-mapped from the client's visual resolution to the server's actual resolution.
- **Debouncing**: Viewport resize events must be debounced (150ms-300ms) to prevent server-side thrashing during window dragging.

## Recommendations
- Implement a 200ms debounce for viewport resize events.
- Provide a "Fixed Ratio" toggle in the UI to prevent distortion.
- Automate coordinate mapping using the ratio of `videoElement.clientWidth` to `serverResolution`.
