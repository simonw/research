# Playwright CDP Session Management Research

## Purpose
This research investigates Playwright's Chrome DevTools Protocol (CDP) connection capabilities for managing browser sessions. It aims to provide guidance on connecting to existing Chrome instances, managing browser contexts for isolation, resetting sessions, and implementing production-ready session lifecycle management.

## Key Findings
1.  **Default Context Persistence**: When connecting via `connectOverCDP`, there is always a default browser context (`browser.contexts()[0]`) that cannot be closed.
2.  **Disconnection**: Playwright uses `browser.close()` to disconnect from the CDP session without killing the browser process. There is no `disconnect()` method.
3.  **Shared State**: Multiple CDP connections to the same browser instance share the same underlying browser state (contexts and pages).
4.  **Isolation**: Browser contexts created via `browser.newContext()` are fully isolated (cookies, localStorage, cache).
5.  **Session Reset**:
    *   **Fastest**: Clear cookies and storage (navigate to `about:blank`, clear localStorage/sessionStorage).
    *   **Most Reliable**: Close and recreate the browser context.
6.  **Performance**: Creating new contexts has higher overhead (~100-200ms) compared to clearing state (~10-50ms), but offers better isolation guarantees.

## Important Code Structures

### Production-Ready Session Manager (`05-session-manager.ts`)
A `SessionManager` class was implemented to handle:
*   Initializing the CDP connection.
*   Creating isolated sessions (`browser.newContext`).
*   Resetting sessions (clearing cookies/storage).
*   Destroying sessions and cleanup.
*   Managing session timeouts.

```typescript
// Example usage pattern
const manager = new SessionManager('http://localhost:9222');
await manager.init();
const context = await manager.createSession('user-1');
// ... usage ...
await manager.destroySession('user-1');
```

## Conclusion
Playwright's CDP capabilities are robust for building browser-as-a-service architectures. The recommended pattern is to maintain a persistent connection to the browser and spawn a new, isolated context for each user session, ensuring proper cleanup to prevent memory leaks.
