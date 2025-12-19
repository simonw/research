# Research Summary: WebRTC over TCP

## Overview
This research investigated the use of TCP as a transport protocol for WebRTC, specifically looking into **ICE-TCP** and **TURN/TCP** for environments where UDP traffic is restricted (a common scenario in corporate networks or certain cloud platforms like Cloud Run).

## Key Findings
- **ICE-TCP (RFC 6544)**: Allows WebRTC candidates to use TCP connections directly. While part of the standard, its implementation and reliability vary across different browser engines.
- **TURN/TCP**: Setting up a TURN server that listens on TCP (typically port 443 to mimic HTTPS) is a more mature and widely supported method for ensuring WebRTC connectivity in UDP-restricted environments.
- **Protocol Caveats**:
    - **Head-of-Line Blocking**: Unlike UDP, TCP ensures ordered delivery. If a packet is lost, all subsequent packets (including newer video frames) are held up, leading to significant latency spikes ("stutter").
    - **Congestion Control**: TCP's built-in congestion control can conflict with WebRTC's application-level congestion control (like GCC), leading to suboptimal bandwidth utilization.

## Conclusions
While UDP is always preferred for low-latency media, TCP support is essential for universal connectivity. **TURN/TCP** is the recommended fallback mechanism over direct ICE-TCP due to better compatibility with firewalls and browser implementations.
