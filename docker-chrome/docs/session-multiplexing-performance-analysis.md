# Session Multiplexing Performance Analysis

## Purpose
This analysis examines the performance bottlenecks of the current Docker Chrome architecture and evaluates the impact of planned optimizations for multi-session support, specifically focusing on network event broadcasting and client-side rendering.

## Key Findings
1.  **Current Bottlenecks**:
    *   **Broadcast Storm**: The server broadcasts all network events (200+) to all clients, regardless of relevance.
    *   **Client Overhead**: Clients perform O(N) filtering on these events, causing high CPU usage and blocking the main thread.
    *   **Rendering**: Rendering 200+ DOM nodes simultaneously causes memory pressure.
2.  **Proposed Optimizations**:
    *   **Server-Side Filtering**: Filter events at the source based on client subscriptions. Estimated 90% reduction in WebSocket traffic.
    *   **Event Batching**: Group events into 100ms batches to reduce framing overhead.
    *   **Compression**: Enable WebSocket `per-message-deflate`.
    *   **Context Pooling**: Reuse browser contexts to reduce session startup time (currently 500-2000ms).
3.  **Capacity Estimates**:
    *   Each additional browser context consumes 50-100MB RAM.
    *   A single container can realistically support 3-5 concurrent sessions with these optimizations.

## Roadmap
1.  **Phase 1 (Immediate)**: Implement server-side filtering, compression, and virtual scrolling on the client.
2.  **Phase 2**: Add async broadcasting with backpressure and context pooling.
3.  **Phase 3**: Advanced resource quotas and distributed sessions.
