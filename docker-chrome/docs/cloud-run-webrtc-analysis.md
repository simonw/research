# Research Summary: WebRTC on Google Cloud Run

## Overview
This research investigated the feasibility of hosting WebRTC media servers (SFUs/MCUs) on Google Cloud Run. The analysis focused on protocol support, networking constraints, and timeout limitations.

## Key Findings
The research concluded that **Cloud Run is NOT suitable for hosting WebRTC media servers**. The primary blockers are fundamental architectural limitations of the platform:

1. **Protocol Restrictions**: Cloud Run ingress is strictly limited to HTTP-based protocols (HTTP/1.1, HTTP/2, gRPC, and WebSockets). It does **not support UDP traffic**, which is required for WebRTC media streams (RTP/RTCP).
2. **ICE & Networking**: WebRTC requires binding multiple UDP ports for ICE negotiation and media transmission. Cloud Run does not allow binding custom UDP ports or exposing port ranges.
3. **Timeout Limits**: Cloud Run imposes a hard 60-minute maximum timeout on all connections, including WebSockets. This limits the duration of any single streaming session.
4. **Signaling vs. Media**: While WebSockets (supported) can be used for signaling, they cannot effectively handle the high-throughput, low-latency requirements of raw media packets.

## Recommended Architecture (Hybrid)
To implement a scalable WebRTC solution on Google Cloud, a hybrid approach is recommended:
- **Cloud Run**: Ideal for the **Signaling Layer** (WebSockets) and REST APIs due to its auto-scaling and cost-efficiency.
- **Google Kubernetes Engine (GKE)**: Best for **Media Servers** (e.g., Janus, Mediasoup). GKE supports UDP LoadBalancer services and provides the necessary networking flexibility.
- **Compute Engine (VMs)**: Recommended for **STUN/TURN Servers** (e.g., coturn) to maintain full control over the network stack and static IP requirements.

## Conclusion
While Cloud Run is excellent for the control plane and signaling of a WebRTC application, it cannot facilitate the media plane. Use GKE or Compute Engine for the media-heavy components.
