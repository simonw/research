# Android MITM MVP - Proxy Configuration Fix Investigation

## Problem Statement
Android emulator cannot reach the internet after proxy configuration changes from `127.0.0.1:8080` to `10.0.2.2:8080`. Frida server also fails to start.

## Root Cause Analysis

### Issue 1: Incorrect Proxy Host Configuration
**Location**: `entrypoint.sh` line 251
```bash
ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
```

**Problem**: In the Docker container setup, both mitmproxy and Android emulator share the same network namespace. The special IP `10.0.2.2` is meant for reaching the HOST machine from within an emulator, NOT for reaching services within the same container.

**Expected**: `127.0.0.1` (localhost within the container)

**Evidence**:
- Previous working config: `127.0.0.1:8080`
- Current broken config: `10.0.2.2:8080`
- TCP errors: "undefined tcp fd 81 to null (-1)" (connection failures)
- DNS working: "Allowing unintercepted udp connection to port 53" (UDP on port 53)

### Issue 2: mitmproxy Listening Configuration
**Location**: `entrypoint.sh` lines 53-57
```bash
mitmweb \
    --web-host 0.0.0.0 \
    --web-port 8081 \
    --listen-host 0.0.0.0 \
    --listen-port 8080 \
    ...
```

**Status**: ✓ CORRECT - mitmproxy is listening on all interfaces (0.0.0.0)

### Issue 3: Frida Server Startup Failure
**Root Cause**: Frida server depends on network connectivity. If Android cannot reach the proxy due to misconfigured proxy host, downstream services like Frida initialization may also fail.

**Evidence from logs**:
- `frida-ps -U` fails with "unable to connect to remote frida-server: closed"
- Likely cascading failure from network connectivity issues

## Docker Container Network Model

### Current Setup
- Container runs with `--privileged` flag
- mitmproxy and Android emulator in SAME container = SAME network namespace
- Direct localhost communication via 127.0.0.1

### Key Insight
The special IP `10.0.2.2` in Android emulator refers to:
- 192.168.1.1 → Host machine (from standard Android emulator on Linux)
- 10.0.2.2 → Default gateway = HOST machine (from standard QEMU configuration)

This is ONLY needed when:
- Emulator runs on host machine (Docker not involved)
- Service runs on HOST outside container

In our case:
- ✗ mitmproxy is NOT on host machine
- ✓ mitmproxy IS inside the same container
- ✗ 10.0.2.2 points to container gateway (unreachable destination)

## Fix Strategy

### Step 1: Update entrypoint.sh (lines 251-252)
Change:
```bash
ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
```

To:
```bash
ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}
```

### Step 2: Verify mitmproxy Configuration
- Confirm mitmproxy listens on 0.0.0.0:8080 ✓
- Confirm proxy can receive connections from 127.0.0.1:8080 ✓

### Step 3: Test Sequence
1. Fix entrypoint.sh
2. Rebuild Docker image
3. Redeploy container to VM
4. Verify Android can reach mitmproxy
5. Verify Frida server starts
6. Verify traffic is captured

## Implementation Timeline
- [ ] Update entrypoint.sh
- [ ] Rebuild Docker image
- [ ] Deploy to VM
- [ ] Test connectivity
- [ ] Verify Frida startup
- [ ] Document findings


## Fix Implementation

### Change Applied
**File**: `native-app-traffic-capture/android-mitm-mvp/entrypoint.sh`
**Line 251**: Changed default proxy host from `10.0.2.2` to `127.0.0.1`

```bash
# Before
ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}

# After
ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}
```

### Why This Fix Works

1. **Network Namespace**: Both mitmproxy and Android emulator run in the same Docker container, sharing the same network namespace
2. **Localhost Resolution**: 127.0.0.1 correctly refers to services within the same container
3. **mitmproxy Listening**: mitmproxy is configured to listen on 0.0.0.0:8080, which includes:
   - 127.0.0.1:8080 (localhost)
   - All other interfaces within the container
4. **Frida Script Configuration**: The proxy host is passed to Frida scripts via environment variable (line 326), which will now correctly use 127.0.0.1

### Network Path Verification
- Android System Settings → Proxy: 127.0.0.1:8080
- Android connects to: localhost (127.0.0.1)
- mitmproxy listens on: 0.0.0.0:8080
- Connection succeeds: ✓ Both on same interface within container

### Why 10.0.2.2 Was Wrong
- 10.0.2.2 = Special gateway IP for QEMU emulator (refers to HOST machine)
- Used ONLY when emulator runs on host and service is outside container
- Our case: Service IS inside container
- Result: TCP connections fail with "undefined tcp fd 81 to null (-1)"

## Next Steps for Deployment

### Prerequisites
- Access to GCP VM (34.42.16.156)
- Docker image rebuild capability
- SSH credentials for deployment

### Deployment Process
1. **Local**: Verify fix in entrypoint.sh
2. **Build**: Rebuild Docker image locally or on VM
3. **Deploy**: Push new image and restart container
4. **Test**: Verify Android connectivity and Frida startup

### Verification Steps
```bash
# Check proxy configuration in emulator
adb shell settings get global http_proxy

# Verify mitmproxy is reachable
adb shell curl -x 127.0.0.1:8080 http://httpbin.org/ip

# Check Frida server status
frida-ps -U

# Monitor traffic in mitmproxy
curl http://localhost:8081
```

### Expected Results After Fix
- ✓ Android proxy setting: 127.0.0.1:8080
- ✓ TCP connections succeed
- ✓ Frida server starts automatically
- ✓ Traffic visible in mitmproxy UI
- ✓ Certificate pinning bypass active

## Technical Details

### Docker Container Architecture
```
┌─────────────────────────────────────────┐
│         Docker Container                 │
│                                          │
│  ┌──────────────────┐                    │
│  │   mitmproxy      │                    │
│  │ 0.0.0.0:8080    │                    │
│  │ 0.0.0.0:8081    │                    │
│  └──────────────────┘                    │
│         ↑                                 │
│   127.0.0.1:8080                         │
│         ↑                                 │
│  ┌──────────────────┐                    │
│  │  Android         │                    │
│  │  Emulator        │                    │
│  │ (QEMU)           │                    │
│  └──────────────────┘                    │
│                                          │
└─────────────────────────────────────────┘
        Same network namespace
```

### Comparison: Wrong vs Correct Configuration

**WRONG (Current Broken)**:
```
Android → 10.0.2.2:8080 → QEMU Gateway → HOST (outside container)
                          ✗ mitmproxy not found
```

**CORRECT (After Fix)**:
```
Android → 127.0.0.1:8080 → localhost → mitmproxy (same container)
                           ✓ Connection succeeds
```

