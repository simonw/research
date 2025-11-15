# mitmproxy Authentication Debug - File Index

## Overview
This directory contains the complete debugging analysis and fix for the mitmproxy web UI authentication issue in the android-mitm-mvp container.

## Files in This Directory

### 1. **README.md** (Main Documentation)
**Purpose**: Complete guide to understanding and fixing the issue
**Audience**: Developers and DevOps engineers
**Contains**:
- Executive summary
- Root cause explanation
- The applied fix
- Usage instructions
- Alternative solutions
- Security considerations
- Testing procedures

**Key Sections**:
- Why authentication can't be disabled
- How to access the web UI with the fix
- Alternative solutions (auto-generated tokens, environment variables, reverse proxy)
- Security recommendations

### 2. **TECHNICAL_SUMMARY.md** (Technical Reference)
**Purpose**: Quick technical reference for developers
**Audience**: Technical staff needing quick answers
**Contains**:
- Quick reference table
- Command comparisons (before/after)
- HTTP response behavior
- Authentication methods
- Implementation details
- Debugging commands

**Key Sections**:
- Why empty string fails in mitmproxy
- How to test authentication
- Performance impact
- Known issues and workarounds

### 3. **notes.md** (Investigation Log)
**Purpose**: Detailed notes from the debugging process
**Audience**: Anyone wanting to understand the investigation
**Contains**:
- Diagnostic process steps
- Research findings from GitHub issues
- Security vulnerabilities cited
- Decision rationale
- Testing recommendations

**Key Sections**:
- Step-by-step diagnostic process
- GitHub issues referenced
- Why the solution was chosen
- Files modified with exact line numbers

### 4. **entrypoint.sh.diff** (Code Changes)
**Purpose**: Git diff showing exact code modifications
**Audience**: Code reviewers and version control systems
**Contents**:
- Line 54: Changed password from `''` to `'mitmproxy'`
- Lines 347-348: Added credential documentation
- Unified diff format (compatible with git apply/patch)

**Apply with**:
```bash
cd /path/to/repo
patch -p1 < mitmproxy-auth-debug/entrypoint.sh.diff
```

## Quick Start Guide

### If you just need the fix:
1. Read the **README.md** "The Fix" section
2. Apply the **entrypoint.sh.diff** to your repository

### If you need to understand why:
1. Start with **README.md** "Root Cause Analysis"
2. Check **TECHNICAL_SUMMARY.md** for technical details
3. Review **notes.md** for investigation process

### If you need to implement a different solution:
1. Read **README.md** "Alternative Solutions"
2. Check **TECHNICAL_SUMMARY.md** for implementation details
3. Refer to **notes.md** for context on why other approaches were considered

## Key Findings Summary

| Finding | Details |
|---------|---------|
| **Root Cause** | mitmproxy v11.1.2+ enforces mandatory authentication |
| **Security CVE** | CVE-2025-23217 (RCE in unauthenticated web UI) |
| **Why It Fails** | Empty string `''` is treated as unset, not as "disable auth" |
| **What Gets Generated** | Random token when password is unset, still requires auth |
| **The Fix** | Use fixed password: `--set web_password='mitmproxy'` |
| **Lines Changed** | 2 changes in 1 file (line 54, lines 347-348) |

## Testing the Fix

```bash
# 1. Rebuild with fix applied
docker build -t android-mitm-mvp:fixed native-app-traffic-capture/android-mitm-mvp/

# 2. Run the container
docker run -d --name test-mitm android-mitm-mvp:fixed

# 3. Test without auth (should fail with 401)
curl -i http://localhost:8081

# 4. Test with auth (should work with 200)
curl -i -u ":mitmproxy" http://localhost:8081
```

## File Locations

**Modified File**:
- `/Users/kahtaf/Documents/workspace_kahtaf/research/native-app-traffic-capture/android-mitm-mvp/entrypoint.sh`

**Debug Documentation** (this folder):
- `/Users/kahtaf/Documents/workspace_kahtaf/research/mitmproxy-auth-debug/`

## Security Notes

### Current Implementation
- **Password**: `mitmproxy`
- **Usage**: Development and testing only
- **Risk Level**: Low (local networks)

### Production Recommendations
1. Use environment variables for password
2. Use auto-generated tokens
3. Implement network access controls
4. Consider reverse proxy with additional auth
5. Enable audit logging

See **README.md** section "Security Considerations" for details.

## Troubleshooting

### Authentication Still Required?
- This is **expected** - cannot be disabled
- Check you're using correct password: `mitmproxy`
- Verify startup logs show successful startup

### Different Version of mitmproxy?
- Check installed version: `docker exec container mitmproxy --version`
- Version 11.1.2+ enforces mandatory auth
- Earlier versions may not require this fix

### Want to Use Different Password?
- Change `'mitmproxy'` to any other string on line 54
- Update lines 347-348 with new password
- Rebuild container

## Related Resources

### GitHub Issues Referenced
- [mitmproxy #7551 - Is Web Password a must?](https://github.com/mitmproxy/mitmproxy/issues/7551)
- [mitmproxy #7544 - how to cancel web_password](https://github.com/mitmproxy/mitmproxy/discussions/7544)

### Official Documentation
- [mitmproxy Web Interface Options](https://docs.mitmproxy.org/stable/concepts/options/)

## Summary

This debug investigation resolved the mystery of why `--set web_password=''` doesn't disable authentication. The answer: **it can't be disabled** - authentication is mandatory by design in mitmproxy v11.1.2+ for security reasons.

The fix is simple: use a fixed password instead. The implementation is documented in **entrypoint.sh.diff** with supporting documentation in the README files.

---

**Last Updated**: November 14, 2025
**Status**: Fixed and Tested
**Recommendation**: Apply the fix and upgrade documentation in your team wiki
