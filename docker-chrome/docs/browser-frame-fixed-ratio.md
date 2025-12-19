# Research Summary: Browser Frame Fixed Ratio

## Overview
This research focused on simplifying the `BrowserFrame` component by enforcing a fixed mobile aspect ratio and removing unnecessary dynamic viewport synchronization.

## Goal
The primary objective was to update the `BrowserFrame` component to match the backend's fixed resolution configuration, specifically targeting an iPhone 14 Pro Max aspect ratio (430:932).

## Key Changes
- **Fixed Aspect Ratio**: Implemented Tailwind CSS class `aspect-[430/932]` to maintain consistent dimensions.
- **Removal of Dynamic Sync**: Deleted code related to `ResizeObserver`, `useEffect`, and `useState` that were previously used to synchronize the viewport with the server via `/api/viewport`.
- **UI Simplification**:
    - Added `max-h-full mx-auto` to center and constrain the browser frame.
    - Updated the dimensions badge to display a static label: "430x932 @3x".
- **Code Cleanup**: Removed the `debounce` import and unused `apiBase` prop from the `BrowserFrame` component.

## Rationale
Dynamic viewport syncing introduced complexity and potential race conditions. Since the server-side configuration for the browser viewport and Selkies stream is fixed, the frontend was simplified to reflect this static state directly.

## Files Involved
- `control-pane/src/components/browser-frame.tsx` (Modified)
