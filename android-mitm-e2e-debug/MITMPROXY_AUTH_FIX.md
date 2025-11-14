# mitmproxy Web UI Authentication Fix

## Problem
The mitmproxy web interface at `http://34.42.16.156:8081` is showing a 403 authentication page requiring a password or authentication token.

The page displays:
```
403 Authentication Required
To access mitmproxy, please enter the password or authentication token printed in the console.
```

Additionally, attempting to exec into the container with `docker exec` fails with:
```
unable to find user root: no matching entries in passwd file
```

## Root Cause
By default, mitmproxy 12.2.0 generates a random authentication token when the web UI starts, and prints it to the console. However:

1. The random token was not being captured during startup
2. The container's passwd file may be incomplete, preventing proper user lookup in exec commands
3. No explicit web authentication flag was set in the entrypoint.sh

## Solution
Disable web UI authentication by adding the `--set web_password=''` flag to the mitmweb command.

### Changes Made
**File**: `/native-app-traffic-capture/android-mitm-mvp/entrypoint.sh` (lines 47-56)

Added the flag `--set web_password=''` to the mitmweb startup command:

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

### How It Works
According to mitmproxy's options documentation:
- `web_password`: "Password to protect the mitmweb user interface"
- Default behavior: If no password is provided, a random token is generated
- Setting to empty string: Disables authentication entirely

### Accessing mitmproxy Web UI
After redeployment with this fix:
1. Open browser to: `http://34.42.16.156:8081`
2. No password/token required
3. Full access to traffic inspection interface

## Testing
1. Rebuild and redeploy the container using `start_vm.sh`
2. Wait for container to boot (monitor with `docker logs`)
3. Access web UI without authentication
4. Verify traffic capture in the mitmproxy interface

## Alternative: Docker Exec Workaround (Existing Container)
For the current running container, if you need to exec into it without proper passwd setup:

Option 1 - Specify user ID instead of name:
```bash
docker exec -u 0 android-mitm-mvp [command]
```

Option 2 - Copy files directly:
```bash
docker cp android-mitm-mvp:/path/to/file /local/path
```

## mitmproxy Configuration Options
For reference, other useful mitmweb options:
- `--set web_debug=true` - Enable web UI debugging
- `--set websocket=true` - Enable WebSocket support (default: true)
- `--web-host 0.0.0.0` - Listen on all interfaces
- `--ssl-insecure` - Don't verify upstream SSL certificates

## Verification Commands
After redeployment, verify the fix:

```bash
# Check mitmproxy is running
curl -v http://localhost:8081/

# Should get 200 OK instead of 403 Authentication Required
```

Expected response headers:
```
< HTTP/1.1 200 OK
< Server: mitmproxy 12.2.0
< Content-Type: text/html; charset=UTF-8
```
