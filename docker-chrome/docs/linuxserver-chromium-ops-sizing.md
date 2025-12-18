# LinuxServer Chromium Ops Sizing Recommendations

## Purpose
This report provides operational recommendations for sizing, monitoring, and scaling a `linuxserver/chromium` container running Chrome with Selkies WebRTC streaming and Playwright automation.

## Resource Sizing Estimates

### Base Requirements
| Component | CPU Cores | Memory |
|-----------|-----------|--------|
| Chrome (idle) | 0.2-0.5 | 200-500MB |
| Selkies Streaming | 0.5-1.0 | 100-200MB |
| Node.js (Playwright) | 0.1-0.3 | 50-100MB |
| **Total Baseline** | **1.0-2.0** | **400-800MB** |

### Scaling (Concurrent Contexts)
*   **1 Context:** 2.0-3.0 CPU, 1.5-2.5GB RAM
*   **3 Contexts:** 4.0-6.0 CPU, 3.0-5.0GB RAM
*   **5 Contexts:** 6.0-8.0 CPU, 5.0-7.0GB RAM

**Recommendation for typical automation (3-5 contexts):** 6.0 vCPUs limit, 8GB RAM limit.

## Monitoring Strategy
Key metrics to track:
1.  **Chrome Process Health:** `ps aux | grep chromium`, `curl localhost:9222/json/version`
2.  **Resource Usage:** Container stats, shared memory usage (`/dev/shm`), Playwright context count.
3.  **Failure Modes:**
    *   **Process Crash:** Recovery via supervisor or container restart (10-30s).
    *   **OOM:** Requires container restart (30-60s).
    *   **WebRTC Issues:** Check TURN server and network.

## Scaling & Performance
*   **Performance:** Use specific Chrome flags (`--disable-gpu`, `--disable-dev-shm-usage`, `--memory-pressure-off`) to optimize for containerized environments.
*   **Horizontal Scaling:** Use a load balancer with sticky sessions to distribute load across multiple container instances.

## Conclusion
The stack is viable but resource-intensive. Proper resource limits and proactive monitoring of the `/dev/shm` volume are critical for stability.
