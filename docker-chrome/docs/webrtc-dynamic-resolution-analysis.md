# WebRTC Dynamic Resolution Analysis

## Purpose
This research investigates the performance implications and best practices for dynamic resolution and viewport resizing in WebRTC streaming, particularly for remote browser viewports.

## Key Findings
1.  **Renegotiation vs. Adaptation**:
    *   `setParameters()` allows changing encoding parameters (bitrate, active state) without a full ICE renegotiation, but cannot change the resolution envelope established in the Offer/Answer.
    *   `replaceTrack()` is efficient for switching sources (e.g., different resolution streams) without renegotiation.
2.  **Simulcast**:
    *   The industry standard (Jitsi, Mediasoup, Twilio) is using 3-layer simulcast (1x, 0.5x, 0.25x resolutions).
    *   Layers are dynamically enabled/disabled based on bandwidth or client capabilities.
3.  **Latency**:
    *   Changing resolution often requires the encoder to generate a new keyframe (IDR frame), causing a latency spike.
    *   Hardware encoders (NVENC) may have higher reconfiguration latency than software encoders.
    *   Jitter buffer management is critical during these transitions.

## Conclusion
For responsive remote browsing, "Simulcast" or "SVC" (Scalable Video Coding) are preferred over frequent SDP renegotiations. If the source resolution changes (e.g., browser resize), the pipeline must handle this gracefully, potentially by renegotiating track constraints or using `replaceTrack` if the source track itself changes.
