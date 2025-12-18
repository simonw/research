# LinuxServer Chromium Container Research

## Purpose
Investigated the `lscr.io/linuxserver/chromium` Docker container to understand its capabilities for dynamic Chrome management, remote debugging, and session handling.

## Key Findings

### Architecture
*   **Base:** Built on `baseimage-selkies` (Debian-based) using KasmVNC/Selkies for WebRTC streaming.
*   **Startup:** Uses a one-shot `autostart` script to launch `wrapped-chromium`.
*   **Init System:** Uses `s6-overlay` but does not supervise the Chrome process directly for restarts.

### Configuration
*   **Static:** Configuration is primarily done via environment variables (`CHROME_CLI`) at startup.
*   **Remote Debugging:** Supported by passing `--remote-debugging-port=9222` in `CHROME_CLI`, but requires exposing the port in the Docker config.
*   **Dynamic Management:** **Not supported.** There is no built-in API to restart Chrome or change flags without restarting the entire container.

### Comparison
| Feature | LinuxServer Chromium | Selenium/Browserless |
|---------|---------------------|----------------------|
| **UI** | Excellent (WebRTC) | None / Basic VNC |
| **Automation** | Via generic CDP | First-class WebDriver/API |
| **Dynamic Config** | No | Yes |

## Conclusion
This container is excellent for interactive use cases (remote desktop, manual testing) but ill-suited for heavy automation requiring dynamic environment changes (like rotating proxies or flags) without external orchestration to restart the container.
