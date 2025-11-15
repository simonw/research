# mitmproxy Authentication Debug - Notes

## Issue Statement
The mitmproxy web UI in the android-mitm-mvp container was still requiring authentication even after setting `--set web_password=''` flag.

## Diagnostic Process

### Step 1: Container Status Check
- Container `android-mitm-mvp` was not currently running
- Examined files from workspace instead

### Step 2: File Analysis
**Dockerfile** (lines 24):
```dockerfile
RUN PIP_BREAK_SYSTEM_PACKAGES=1 pip3 install --no-cache-dir mitmproxy frida-tools
```
- Installs latest available mitmproxy version (>= v11.1.2)
- Latest mitmproxy enforces mandatory web UI authentication

**entrypoint.sh** (lines 47-56):
```bash
mitmweb \
    --web-host 0.0.0.0 \
    --web-port 8081 \
    --listen-host 0.0.0.0 \
    --listen-port 8080 \
    --ssl-insecure \
    --set block_global=false \
    --set web_password='' \
    --no-web-open-browser \
    > "${MITM_LOG}" 2>&1 &
```

### Step 3: Research on mitmproxy Authentication

#### Searched GitHub Issues
1. **Issue #7551**: "Is Web Password a must?"
   - Answer: Yes, authentication is mandatory in v11.1.2+

2. **Discussion #7544**: "how to cancel web_password"
   - User tried to disable authentication
   - Maintainer stated: No official way to disable entirely
   - Reason: "Access to the web UI is pretty much RCE"

3. **Issue #7551**: Security implications
   - Referenced CVE-2025-23217
   - Vulnerability: RCE via unauthenticated web UI access
   - GHSA reference: GHSA-wg33-5h85-7q5p

#### Key Findings
- Empty string `''` for `web_password` is NOT accepted as valid
- When no valid password is set, mitmproxy:
  1. Generates a random token (security measure)
  2. Still enforces authentication on all endpoints
  3. Logs the token for browser auto-login
  4. Prevents programmatic/API access without the token

- mitmproxy maintainers explicitly refuse to add a "no auth" mode
- Quote: "I'm hesitant to provide an option that disables authentication"

## Root Cause

**The `--set web_password=''` parameter does not disable authentication.** This is intentional by design.

When mitmproxy v11.1.2+ detects an empty or unset password:
1. It treats it as invalid
2. Generates a random authentication token
3. Enforces auth on all web UI endpoints
4. This prevents accidental insecure deployments

## Solution Applied

Changed line 54 in entrypoint.sh from:
```bash
--set web_password='' \
```

To:
```bash
--set web_password='mitmproxy' \
```

Also updated the startup documentation (lines 347-348) to inform users:
```bash
echo "     Username: (leave blank)"
echo "     Password: mitmproxy"
```

## Why This Works

1. **Fixed password** is recognized by mitmproxy as valid
2. **'mitmproxy'** is simple, memorable, and suitable for development/testing
3. **Consistent** across container restarts
4. **Documented** in the startup output

## Alternative Solutions Considered

### Solution 2: Auto-Generated Token (More Secure)
Remove the `--set web_password` line entirely:
```bash
mitmweb \
    --web-host 0.0.0.0 \
    --web-port 8081 \
    ... \
```
Then retrieve token from logs:
```bash
docker exec android-mitm-mvp grep "Token:" /var/log/mitmproxy.log
```

**Pros**: Random token per session, more secure
**Cons**: Token changes on restart, requires log inspection

### Solution 3: Programmatic Access with Token Parameter
For API calls, use:
```
http://localhost:8081?token=mitmproxy
```

**Pros**: Allows programmatic traffic monitoring
**Cons**: Still requires password/token in URL

## Testing the Fix

```bash
# Rebuild the container with the fix
docker build -t android-mitm-mvp:v2 native-app-traffic-capture/android-mitm-mvp/

# Run it
docker run -d --name android-mitm-mvp-test android-mitm-mvp:v2

# Test web UI access (should show 401 without auth)
curl -v http://localhost:8081

# Test with password (Basic auth)
curl -v -u ":mitmproxy" http://localhost:8081

# Access via browser
# URL: http://localhost:8081
# Password prompt: leave username blank, enter "mitmproxy" as password
```

## Security Implications

**Why mitmproxy enforces authentication:**
1. Web UI provides full traffic manipulation capabilities
2. Could be used for man-in-the-middle attacks
3. Access to modify request/response bodies
4. Can intercept and modify sensitive data

**Recommendation**:
- For local development: Use fixed password 'mitmproxy' (current solution)
- For production: Use auto-generated tokens or reverse proxy with additional auth
- For cloud deployments: Consider network ACLs to restrict access

## Files Modified

1. `/Users/kahtaf/Documents/workspace_kahtaf/research/native-app-traffic-capture/android-mitm-mvp/entrypoint.sh`
   - Line 54: Changed `--set web_password=''` to `--set web_password='mitmproxy'`
   - Lines 347-348: Added password documentation to startup output

## Verification

The fix changes:
- **Before**: Authentication still required, random token generated
- **After**: Authentication required, uses consistent password 'mitmproxy'

Both require authentication (as intended by mitmproxy), but the fixed version:
- Consistent across restarts
- Known credential for documentation
- Suitable for development environment
