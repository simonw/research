# WebSocket Multiplexing Safeguards

## Purpose
Investigation into edge cases and security safeguards for multiplexing multiple sessions over a single WebSocket connection, aiming to prevent data leaks, routing errors, and denial-of-service (DoS) attacks.

## Key Findings

### Identified Edge Cases
1.  **Message Routing Bugs**: Messages delivered to the wrong session due to ID collisions or race conditions.
2.  **Cross-Session Leaks**: Shared buffers or contexts leaking data between isolated sessions.
3.  **Authorization Gaps**: Permissions checked only at connection level, not per-session.
4.  **DoS Vulnerability**: Single high-volume session starving others of bandwidth/processing.

### Recommended Safeguards
- **Session ID Prefixing**: Use fixed-length, validated session IDs for O(1) routing.
- **Buffer Isolation**: Maintain separate, zero-copy buffers for each session.
- **Per-Session Auth**: Validate permissions on every operation/message within a session context.
- **Traffic Control**: Implement Token Bucket rate limiting per session and Weighted Fair Queuing.

## Architecture Proposal

**Message Format:**
`[SessionID:8bytes][MessageType:1byte][Payload:variable]`

**MultiplexManager Logic:**
- Validate Session ID.
- Check Rate Limit (consume token).
- Verify Auth.
- Route to Session Buffer.

## Configuration Defaults
- **Max Sessions/Connection**: 100
- **Session Timeout**: 300s
- **Rate Limit**: 1000 requests/min per session
- **Max Message Size**: 64KB
