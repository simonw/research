# WebSocket Fanout/Multiplexing Research

## Purpose
This research explores best practices for high-performance WebSocket fanout (broadcasting), multiplexing, and backpressure handling to support scalable real-time applications.

## Key Findings
1.  **Backpressure is Critical**:
    *   Without backpressure, fast producers overwhelm slow consumers, leading to unbounded memory growth and crashes.
    *   **Pattern**: Check `ws.bufferedAmount` before sending. If it exceeds a threshold (e.g., 64KB), pause sending or drop messages (if lossy is acceptable).
    *   **Drain Events**: Wait for the buffer to drain (bufferedAmount decreases) before resuming sends.
2.  **Library Differences**:
    *   **`ws` (Node.js)**: Requires manual implementation of backpressure logic.
    *   **`uWebSockets.js`**: C++ based, significantly higher performance, built-in backpressure API (`getBufferedAmount`, `drain` event), and efficient Pub/Sub.
3.  **Multiplexing**:
    *   Running multiple logical streams over a single WebSocket connection is efficient but requires a framing protocol on top of WebSockets.
4.  **Framing Overhead**:
    *   WebSocket framing overhead is negligible (2-14 bytes per message), making it efficient for small, frequent messages like mouse coordinates or network events.

## Conclusion
For the Docker Chrome project, implementing backpressure is mandatory for stability. Moving to `uWebSockets.js` could offer significant performance gains for high-fanout scenarios (broadcasting network events to many clients) if Node.js `ws` proves insufficient.
