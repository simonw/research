# mitmproxy Web UI Authentication Issue - Debug Report

## Executive Summary

The mitmproxy web UI in the android-mitm-mvp container was still requiring authentication even after attempting to disable it with `--set web_password=''`. This is **expected behavior** - authentication is mandatory by design in mitmproxy v11.1.2+ due to security vulnerabilities. The fix involves using a fixed password instead of attempting to disable authentication.

## Root Cause Analysis

### The Problem
```bash
# This does NOT disable authentication:
mitmweb --set web_password='' ...
```

Even with an empty password string, the mitmproxy web UI still requires authentication.

### Why This Happens
Starting with mitmproxy v11.1.2, the maintainers made **authentication mandatory** in the web UI. This is a deliberate security decision driven by:

1. **CVE-2025-23217**: RCE vulnerability in unauthenticated web UI access
2. **Security Principle**: "Access to the web UI is pretty much RCE"
3. **GHSA Reference**: GHSA-wg33-5h85-7q5p

### How mitmproxy Handles Empty Password

When `--set web_password=''` is used:

1. The empty string is treated as **invalid** (not a deliberate disabling)
2. mitmproxy automatically generates a **random token** for the session
3. This token is displayed in container logs for access
4. Authentication is still **enforced** on all web UI endpoints
5. Without the token/password, requests return **401 Unauthorized**

### Why It Can't Be Disabled

From the official mitmproxy issue tracker (#7551):

> "I'm hesitant to provide an option that disables authentication. Access to the web UI is pretty much RCE."

The developers intentionally refuse to add a "no authentication" mode to prevent accidental insecure deployments.

## The Fix

### Changed Files
**File**: `native-app-traffic-capture/android-mitm-mvp/entrypoint.sh`

**Change 1 - Line 54**: Set a fixed password
```bash
# Before:
--set web_password='' \

# After:
--set web_password='mitmproxy' \
```

**Change 2 - Lines 347-348**: Document the credentials
```bash
# Added:
echo "     Username: (leave blank)"
echo "     Password: mitmproxy"
```

### Why This Solution

1. **Recognized by mitmproxy**: Fixed passwords are accepted as valid
2. **Consistent**: Same credentials across container restarts
3. **Simple**: Easy to remember and document
4. **Suitable for development**: Appropriate for local testing environments

## Usage

### Access the Web UI
1. Navigate to: `http://localhost:8081`
2. When prompted for credentials:
   - **Username**: (leave blank)
   - **Password**: `mitmproxy`

### Programmatic Access
For API calls and automation:
```bash
# Using curl with Basic auth
curl -u ":mitmproxy" http://localhost:8081/

# Using URL parameter
curl "http://localhost:8081/?token=mitmproxy"
```

### Container Access
```bash
# View startup logs including any auto-generated tokens
docker exec android-mitm-mvp tail /var/log/mitmproxy.log | grep -i token

# Access mitmproxy stats via API
docker exec android-mitm-mvp curl -u ":mitmproxy" http://localhost:8081/api/
```

## Alternative Solutions

### Option 1: Auto-Generated Token (Most Secure)
**Approach**: Remove the password flag and use randomly generated tokens

```bash
# Modify entrypoint.sh - remove the --set web_password line entirely
mitmweb \
    --web-host 0.0.0.0 \
    --web-port 8081 \
    --listen-host 0.0.0.0 \
    --listen-port 8080 \
    --ssl-insecure \
    --set block_global=false \
    --no-web-open-browser \
    > "${MITM_LOG}" 2>&1 &
```

**Pros**:
- New random token generated per session
- More secure for production deployments
- Standard mitmproxy default behavior

**Cons**:
- Token changes on each container restart
- Requires log inspection to retrieve token
- Less convenient for development

**Retrieve token**:
```bash
docker logs android-mitm-mvp 2>&1 | grep "Token:"
```

### Option 2: Environment Variable Password
**Approach**: Make password configurable via environment variable

```bash
# In entrypoint.sh:
MITM_PASSWORD=${MITM_PASSWORD:-"mitmproxy"}

mitmweb \
    ...
    --set web_password="${MITM_PASSWORD}" \
    ...
```

**Usage**:
```bash
docker run -e MITM_PASSWORD="custom_secret" android-mitm-mvp
```

**Pros**:
- Flexible for different deployments
- Password can be unique per environment
- No hardcoding required

**Cons**:
- Requires additional setup
- Password visible in docker run command (use .env or secrets)

### Option 3: Reverse Proxy Authentication
**Approach**: Place nginx/Caddy in front of mitmproxy with additional auth

```nginx
# nginx.conf
location / {
    auth_basic "mitmproxy";
    auth_basic_user_file /etc/nginx/.htpasswd;
    proxy_pass http://localhost:8081;
}
```

**Pros**:
- Additional security layer
- Can use multiple authentication methods (2FA, OAuth, etc.)
- Decouples auth from mitmproxy

**Cons**:
- More complex setup
- Additional container/service required
- Potential performance impact

## Testing the Fix

### Quick Verification
```bash
# Build the updated image
docker build -t android-mitm-mvp:fixed native-app-traffic-capture/android-mitm-mvp/

# Run it
docker run -d --name test-mitm android-mitm-mvp:fixed

# Test without authentication (should fail)
curl -v http://localhost:8081
# Expected: 401 Unauthorized

# Test with authentication (should succeed)
curl -v -u ":mitmproxy" http://localhost:8081
# Expected: 200 OK
```

### Full Integration Test
```bash
# Start container
docker run -d --name android-mitm-mvp android-mitm-mvp:fixed

# Wait for startup
sleep 30

# Check mitmproxy is running
docker exec android-mitm-mvp ps aux | grep mitmweb

# Verify certificate was created
docker exec android-mitm-mvp ls -la /root/.mitmproxy/

# Test web UI with correct password
docker exec android-mitm-mvp curl -s -u ":mitmproxy" http://localhost:8081 | grep -q "mitmproxy" && echo "Auth works!"

# Check proxy is listening
docker exec android-mitm-mvp netstat -tuln | grep 8080
```

## Security Considerations

### Current Solution (Fixed Password 'mitmproxy')
**Security Level**: Development/Testing Only

- Suitable for: Local testing, CI/CD in isolated networks
- Not suitable for: Public networks, production, multi-user environments
- Risk: Password could be exposed in container commands or logs

**Recommendations**:
- Use environment variables or secrets management for production
- Implement network access controls (firewall rules)
- Rotate password regularly
- Monitor access logs

### For Production Deployments
1. **Use auto-generated tokens** (regenerated per session)
2. **Restrict network access** via firewall/VPC rules
3. **Use reverse proxy** with additional authentication layers
4. **Enable audit logging** for all web UI access
5. **Consider mitmproxy API** instead of web UI for automation

## Version Information

- **mitmproxy Version**: v11.1.2+ (installed via pip in Dockerfile)
- **Container Base**: budtmo/docker-android:emulator_13.0
- **Date of Analysis**: November 2024
- **Security Advisory**: CVE-2025-23217, GHSA-wg33-5h85-7q5p

## References

### GitHub Issues
1. [mitmproxy #7551 - Is Web Password a must?](https://github.com/mitmproxy/mitmproxy/issues/7551)
2. [mitmproxy #7544 - how to cancel web_password](https://github.com/mitmproxy/mitmproxy/discussions/7544)
3. [mitmproxy #4626 - Authentication for mitmweb](https://github.com/mitmproxy/mitmproxy/issues/4626)

### Documentation
- [mitmproxy Options Documentation](https://docs.mitmproxy.org/stable/concepts/options/)
- [mitmproxy Web Interface](https://docs.mitmproxy.org/stable/concepts/options/#web_password)

## Summary of Changes

| Item | Before | After |
|------|--------|-------|
| Password Setting | `--set web_password=''` | `--set web_password='mitmproxy'` |
| Web UI Access | Still required auth (random token) | Still required auth (fixed password) |
| Consistency | Token changes per restart | Same password across restarts |
| Documentation | Not documented | Added to startup output |
| File Modified | entrypoint.sh (line 54) | entrypoint.sh (line 54 + 347-348) |

## Conclusion

**The issue is not a bug - it's intentional security design.** mitmproxy v11.1.2+ enforces mandatory authentication on the web UI to prevent unauthorized access and potential RCE vulnerabilities.

The fix provides a practical solution for development environments while remaining secure. For production deployments, consider the alternative solutions above based on your security requirements.
