# Research Summary: WebRTC Desktop Streaming Patterns

## Overview
This research analyzes high-performance desktop streaming architectures using WebRTC, drawing from production-grade projects like Selkies-GStreamer, Neko, and KasmVNC. The focus is on achieving sub-100ms latency and high-quality remote desktop experiences.

## Core Architecture
### 1. Capture Pipeline (X11)
- **Primary Source**: `ximagesrc` is the standard for X11, leveraging the **XSHM (Shared Memory)** extension for efficient frame capture.
- **Crucial Setting**: `use-damage=0`. While region tracking (XDamage) seems efficient, it frequently causes dropouts in H.264 streams. Full-frame capture at high refresh rates is preferred.
- **Cursor Handling**: Best practice is to hide the server-side hardware cursor (`show-pointer=0`) and render it on the client-side to eliminate "cursor lag."

### 2. Encoding Hierarchy
1. **NVENC (NVIDIA)**: Highest performance, lowest latency using `nvh264enc` or `nvh265enc`. Requires `cudaupload` and `cudaconvert`.
2. **VAAPI (Intel/AMD)**: Solid hardware alternative using `vaapih264enc`.
3. **Software (x264/VP8)**: Reliable fallback. Requires `tune=zerolatency` and `preset=ultrafast` to maintain interactivity.

### 3. Latency Optimization
- **GOP Strategy**: Use an infinite GOP or very long intervals. Forcing I-frames frequently spikes bandwidth and latency.
- **Zero B-Frames**: Never use B-frames (`bframes=0`) as they require buffering multiple frames, adding ~33-66ms of lag.
- **Jitter Buffer**: Set `webrtcbin` latency to 0 to process packets immediately upon arrival.

## System Interaction
- **Input Injection**: The **X11 XTest extension** is the universal standard for injecting mouse clicks, key presses, and scroll wheel events.
- **Dynamic Resizing**: Accomplished via **XRandR**. Implementation requires:
    1. Stopping the capture pipeline.
    2. Generating a new X11 mode via `cvt`.
    3. Applying the mode via `xrandr`.
    4. Re-creating the GStreamer pipeline with new resolution capabilities.
- **Width Alignment**: Xorg requires the screen width to be a multiple of 8.

## WebRTC vs. VNC
| Feature | WebRTC | Traditional VNC (RFB) |
|---------|--------|-----------------------|
| **Latency** | <100ms | 200ms - 500ms |
| **Compression** | H.264 / VP9 / AV1 (Inter-frame) | JPEG / Tight (Intra-frame) |
| **Acceleration** | Full Hardware (GPU) | Primarily CPU-bound |
| **Congestion** | Supported (GCC) | Limited |

## Conclusion
WebRTC is the superior protocol for remote browser and desktop streaming. Success depends on a tight integration between GStreamer, X11 extensions (XTest/XRandR), and hardware-specific encoders.
