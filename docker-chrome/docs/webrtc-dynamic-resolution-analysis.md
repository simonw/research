# WebRTC Dynamic Resolution Analysis Research Summary

## Overview
Analysis of methods for dynamically adjusting WebRTC stream resolution to match client viewports or network conditions without restarting the peer connection.

## Key Findings
- **`setParameters()`**: Best for dynamic bitrate and layer control (simulcast) with <1ms latency.
- **`applyConstraints()`**: Triggers encoder reconfiguration, causing 100-500ms of latency and a potential video freeze.
- **Simulcast**: Encoding multiple resolution layers (e.g., 1x, 0.5x, 0.25x) allows for near-instant switching by the SFU or client based on viewport size.
- **Hardware Acceleration**: NVENC/AMF handle resolution changes much more gracefully than software encoders (libvpx/x264).

## Recommendations
- Use Simulcast with 3 layers enabled by default.
- Prefer `setParameters()` for quality adjustments.
- Limit manual resolution changes to extreme viewport shifts to avoid frequent encoder resets.
