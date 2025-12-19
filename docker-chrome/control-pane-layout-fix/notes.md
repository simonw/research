# Control Pane Layout Fix

## Objective
Fix the control pane layout so the browser frame displays the mobile viewport (430x932) at a usable size, prioritizing it over the network panel.

## Changes Made
1.  **Modified `control-pane/src/app/page.tsx`**:
    *   Changed the browser frame container to `flex-[3]` (was `flex-1`) to give it 75% of the available vertical space relative to the network panel.
    *   Added `flex justify-center items-center` to the browser frame container to center the mobile frame.
    *   Changed the network panel container to `flex-1` with `max-h-[300px]` to constrain its height.

2.  **Modified `control-pane/src/components/browser-frame.tsx`**:
    *   Removed the "430x932 @3x" overlay badge.
    *   Updated the container styles to `h-full` and `aspect-[430/932]` to ensure the frame fills the available height while maintaining the correct mobile aspect ratio.

## Result
The layout should now display a large, centered mobile browser frame that resembles a phone screen, with a smaller, scrollable network panel below it.
