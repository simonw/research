# Docker Chrome Control Pane Development Notes

## Initial Setup
- Created project structure based on `redroid` reference.
- Target directory: `docker-chrome/control-pane`
- Stack: Next.js 15+, Tailwind CSS 4, TypeScript.

## Design Decisions
- Theme: Dark (#111111) as requested.
- Layout: 
    - Left col: Iframe (Mobile viewport simulation) + Network Panel.
    - Right col: Controls (Navigation, Injection, Status).
- Components:
    - `BrowserFrame`: Simulates a phone device.
    - `NetworkPanel`: Scrollable list of WebSocket events.
    - `ControlPanel`: Inputs for interacting with the remote browser.

## Integration Details
- Cloud Run URL: `https://docker-chrome-432753364585.us-central1.run.app`
- WebSocket: `wss://docker-chrome-432753364585.us-central1.run.app/ws`
- No local API routes; direct fetch/WebSocket to remote.

## Implementation Steps
1.  **Configuration**: Created `package.json`, `tsconfig.json`, `tailwind.config.ts`. Used versions compatible with `redroid` where possible.
2.  **Types**: Defined `NetworkRequest`, `Status`, `PersistentScript` interfaces.
3.  **Components**:
    *   `BrowserFrame`: Styled with CSS borders to look like a phone.
    *   `NetworkPanel`: Table-like layout using CSS Grid, auto-scrolling to bottom on new requests.
    *   `ControlPanel`: Forms for navigation and injection, plus persistent script management using the API.
4.  **Page**: `src/app/page.tsx` handles the WebSocket connection to avoid multiple connections, passing the stream of requests to `NetworkPanel`.

## Challenges & Solutions
-   **WebSocket State**: Decided to keep the WS connection in the top-level `page.tsx` component to ensure `NetworkPanel` just receives data and doesn't manage connection logic.
-   **Scrolling**: Used `useRef` and `useEffect` in `NetworkPanel` to auto-scroll to the bottom when new requests arrive.
-   **Theme**: Copied the dark aesthetic requested, ensuring `bg-[#111111]` matches the `redroid` feel.
