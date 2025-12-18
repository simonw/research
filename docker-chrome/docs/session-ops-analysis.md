# Session Operations Analysis

## Purpose
Analysis of session management, reliability, and resource handling for the Dockerized Chrome streaming system, focusing on moving from a single global browser instance to a multi-tenant architecture.

## Key Findings

### Critical Issues
- **Global State**: Single `browser`, `context`, and `page` variables prevented concurrent sessions.
- **Resource Exhaustion**: 
  - Unbounded memory growth in response stores (no time-based cleanup).
  - Accumulation of persistent scripts and WebSocket clients.
  - Lack of CPU/Memory limits per session.
- **Zombie Processes**: No mechanism to clean up orphaned Chrome processes or handle graceful shutdowns.
- **Security**: No isolation between sessions; data could leak across users.

### Recommended Architecture
1.  **SessionManager**: A central class to manage session lifecycle (create, destroy, timeout).
2.  **Resource Limits**:
    - **Memory**: 100MB per session, 1GB total container limit.
    - **Concurrency**: Max 10 sessions per container.
    - **Timeouts**: 30-minute idle timeout.
3.  **Cleanup Strategy**: Time-based eviction for response stores (5min TTL) and automatic process reaping.

## Implementation Plan
1.  **Fix Leaks**: Implement TTL for response stores and limits for scripts.
2.  **Session Management**: Create `SessionManager` with isolation APIs.
3.  **Enforce Limits**: Add rate limiting (5 sessions/min) and backpressure.
4.  **Reliability**: Add health checks (every 30s) and zombie process prevention.

## Docker Configuration Recommendations
- `MAX_SESSIONS=10`
- `SESSION_TIMEOUT=1800000` (30 mins)
- `MAX_MEMORY_PER_SESSION=100MB`
- `RESPONSE_STORE_TTL=300000` (5 mins)
