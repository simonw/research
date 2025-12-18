# Dynamic Viewport Resizing & WebSocket Multiplexing Edge Cases

## Purpose of the Research
To act as an edge-case oracle identifying failure modes in dynamic viewport resizing (CDP `page.setViewportSize`) and WebSocket multiplexing for multi-session scenarios.

## Key Findings/Notes

### Failure Modes
1.  **Renegotiation Storms**: TLS renegotiation triggered by viewport changes and multiplexing recovery can cause high CPU usage and connection exhaustion.
2.  **Event-Loop Backpressure**: Synchronous viewport operations block the Node.js event loop (200-1000ms blocking), leading to WebSocket buffer growth.
3.  **Cross-Session Leaks**: Improper isolation can allow viewport state, auth context, or messages to leak between sessions.

### Impact Assessment
- **Renegotiation Storms**: CPU spike to 95%, connection pool exhaustion.
- **Event-Loop Backpressure**: Lag > 50ms, memory expansion.
- **Cross-Session Leaks**: Critical security and integrity risks.

## Important Code Snippets/Structures

**Mitigation: Connection Pooling with Affinity**
```typescript
class ConnectionPool {
  getConnection(sessionId: string): WebSocket {
    // ... pool logic ...
    const ws = this.createConnection(sessionId);
    pool.push(ws);
    return ws;
  }
}
```

**Mitigation: Async Viewport Operations**
```typescript
class AsyncViewportManager {
  async setViewport(sessionId: string, viewport: Viewport): Promise<void> {
    // Queue and debounce viewport changes
    // Execute off event loop
  }
}
```

## Conclusion/Next Steps

### Mitigation Strategies
1.  **Renegotiation**: Implement connection pooling with affinity, exponential backoff, and TLS session resumption.
2.  **Backpressure**: Use async viewport operations, manage WebSocket buffers, and monitor the event loop.
3.  **Isolation**: Implement session isolation middleware and validate message routing.

**Priority**: Phase 1 (Critical) - Immediate safeguards for connection pooling and session isolation.
