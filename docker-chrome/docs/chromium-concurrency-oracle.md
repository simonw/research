# Chromium Concurrency Capacity Oracle

## Purpose
Investigation to determine the safe number of concurrent Playwright browser contexts that can run within a `linuxserver/chromium` container, considering memory, CPU, and stability constraints.

## Key Findings

### Capacity Estimates
- **Conservative (Recommended)**: 2-3 concurrent contexts.
- **Moderate**: 4-6 concurrent contexts (requires monitoring).
- **Aggressive (High Risk)**: 7-10 concurrent contexts.

### Resource Constraints
- **Memory**: Each context adds ~50-100MB overhead. Containers have a hard 2GB `shm_size` limit (default).
- **CPU**: Scales linearly; ~0.5-1.0 cores required per context depending on activity.
- **Base Overhead**: ~400-800MB (Idle Chrome + Selkies).

### Limiting Factors
1.  **Shared Memory**: The 2GB `/dev/shm` limit is the primary bottleneck for renderer processes.
2.  **Chrome Architecture**: Multi-process model means high overhead per context.
3.  **CDP Limits**: Single WebSocket connection shared across all contexts.

## Measurement Plan
A testing strategy was defined using a custom load script (`concurrent-contexts.js`) to measure:
- **Context Creation Time**: monitoring for timeouts >2s.
- **Memory Usage**: Tracking JS heap and system memory under load.
- **Failure Rates**: Identifying the tipping point for OOM kills or CDP disconnections.

## Production Config Recommendation
For a standard deployment:
- **Limits**: 3-5 contexts per container.
- **Resources**: 6-8 CPU cores, 8-12GB RAM.
- **Strategy**: Horizontal scaling (add more containers) rather than vertical scaling (stuffing more contexts).
