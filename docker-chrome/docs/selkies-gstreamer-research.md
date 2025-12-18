# Selkies GStreamer Research

## Purpose
This project researched **Selkies-GStreamer**, an open-source, low-latency, high-performance WebRTC-based remote desktop streaming platform for Linux. The goal was to understand its architecture, capabilities, and suitability for streaming browser sessions.

## Key Findings
1.  **Architecture**:
    *   **GStreamer Backend**: Handles screen capture (`ximagesrc`), audio capture (`pulsesrc`), and encoding.
    *   **Python Signaling Server**: Manages WebRTC negotiation (SDP, ICE) via WebSockets.
    *   **Web Client**: Vue.js-based frontend that renders the stream and captures input (keyboard, mouse, gamepad).
    *   **NGINX**: Serves the frontend and proxies WebSocket traffic.
2.  **Performance & Encoding**:
    *   Supports hardware acceleration: NVIDIA NVENC (`nvh264enc`) and VA-API (`vah264enc`).
    *   Software fallback available (`x264enc`, `vp8enc`).
    *   Capable of 60+ FPS at 1080p with low latency.
3.  **Deployment**:
    *   Docker-native design with multi-stage builds.
    *   **TURN Server**: Critical for NAT traversal in production environments. Can use internal coTURN or external server.
    *   Configurable via environment variables (resolution, encoder, frame rate, auth).
4.  **Integration**:
    *   Can be embedded in other applications via `<iframe>` or by using the `selkies-core.js` library directly.
    *   Supports dynamic resizing to match client window size.

## File Structure & Components
*   `/opt/gstreamer`: GStreamer pipeline components.
*   `/opt/gst-web`: Web client static files.
*   `selkies_gstreamer`: Python package for the signaling server.
*   `selkies_joystick_interposer.so`: `LD_PRELOAD` library for gamepad support.

## Conclusion
Selkies-GStreamer is a powerful solution for high-fidelity remote desktop streaming. Its support for GPU acceleration and WebRTC makes it superior to VNC for interactive applications like browsing. It is well-suited for containerized deployments, though requiring a TURN server adds some infrastructure complexity.
