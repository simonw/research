# Responsive BrowserFrame Implementation

## Purpose
Replaced the fixed-size phone mock in the control pane with a fully responsive browser frame that adapts to the container size and synchronizes with the backend Chrome instance.

## Key Features
*   **Responsive Resize:** Uses `ResizeObserver` to detect container size changes.
*   **Debounced Sync:** updates are sent to the backend (`/api/viewport`) only after 300ms of stability to prevent performance thrashing.
*   **Smart Updates:** Only sends updates if the dimension change is significant (>16px).
*   **Visual Feedback:** Displays current viewport dimensions in a subtle overlay.
*   **Constraints:** Clamps dimensions between server-side supported limits (320x480 to 1920x1080).

## Implementation Details

### Debounce Utility
A typed debounce function was added to manage high-frequency resize events.

```typescript
export function debounce<T extends (...args: any[]) => void>(
  func: T,
  wait: number
): (...args: Parameters<T>) => void {
  let timeout: NodeJS.Timeout | null = null;
  return function (...args: Parameters<T>) {
    if (timeout) {
      clearTimeout(timeout);
    }
    timeout = setTimeout(() => {
      func(...args);
    }, wait);
  };
}
```

### Component Logic
The `BrowserFrame` component maintains local state for dimensions and uses a `useRef` to track the last sent dimensions, ensuring efficient network usage. It defaults to an `aspect-video` ratio but allows vertical resizing via CSS.

## Next Steps
This implementation lays the groundwork for variable-resolution streaming and better desktop emulation support in the future.
