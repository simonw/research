# Docker Browser Streaming Research

## Purpose
Evaluate production-ready solutions for streaming browser sessions from Docker containers, comparing CDP-based streaming vs. VNC-based solutions.

## Key Findings
- **Browserless.io:** The gold standard for programmatic streaming. Uses CDP and WebSockets. Good for automation and headless tasks requiring occasional visibility.
- **VNC Solutions:**
  - **KasmVNC:** Modern, web-native, supports better compression and DLP features. Best for full desktop/interactive capability.
  - **noVNC:** Traditional, widely compatible but lower performance/fidelity.
- **Cloud Compatibility:** Both approaches work on Cloud Run/K8s, but WebSocket handling requires sticky sessions or appropriate load balancing.

## Conclusion
- For **Automation/DevTools** use cases: Adopt the **Browserless** architecture (CDP-based).
- For **User Interaction/Remote Desktop** use cases: Adopt **KasmVNC**.
