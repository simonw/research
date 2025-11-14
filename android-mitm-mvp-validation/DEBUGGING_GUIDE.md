# Frida Server Startup Failure - Debugging Guide

## Problem Statement

The Android MITM MVP container fails at Step [5/8] when attempting to start Frida server:
- Error: `✗ Frida server failed to start`
- Exit Code: 1
- Root Cause: Unknown

The `frida-ps -U` command returns non-zero (failure), indicating Frida server is not responding.

## What We Know

✓ **Working**:
- Frida binary pushed to device (108.3 MB)
- File created at `/data/local/tmp/frida-server`
- Permissions set (755)
- ADB connection active

✗ **Failed**:
- Frida server startup check: `frida-ps -U` failed
- Container exited with code 1

## Hypothesis Testing (In Priority Order)

### Hypothesis 1: Frida Binary Incompatibility

**Theory**: Frida binary doesn't run on Android 13 x86_64 emulator

**Test Plan**:
```bash
# Step 1: SSH to VM
gcloud compute ssh android-mitm-mvp --zone=us-central1-a --project=corsali-development

# Step 2: Restart container with interactive shell
docker run -it --device /dev/kvm android-mitm-mvp:latest bash

# Step 3: Wait for boot and check environment
adb wait-for-device
adb shell getprop ro.build.version.release
adb shell getprop ro.product.cpu.abilist

# Step 4: Try running Frida manually with error output
adb shell /data/local/tmp/frida-server 2>&1 | head -20

# Step 5: Check if process starts at all
adb shell ps -A | grep frida

# Step 6: Check if it's using a port
adb shell netstat -tln 2>&1 | grep -E "27042|LISTEN"
```

**Expected Results**:
- If binary fails immediately: Will see error output in step 4
- If binary crashes: Process won't appear in step 5
- If port issue: Port won't appear in step 6

**How to Fix**:
- Download correct Frida binary for Android 13, x86_64, ARM ISA
- Verify Frida version compatibility

### Hypothesis 2: ADB Connectivity Drop

**Theory**: Heavy file operations (108MB) caused ADB to disconnect

**Test Plan**:
```bash
# In existing container after Frida push fails:
docker exec android-mitm-mvp bash -c 'adb devices'
docker exec android-mitm-mvp bash -c 'adb shell getprop sys.boot_completed'
docker exec android-mitm-mvp bash -c 'adb shell ls /data/local/tmp/frida-server'

# Check if ADB process is alive
docker exec android-mitm-mvp bash -c 'ps aux | grep adb'
```

**Expected Results**:
- If ADB still connected: Commands should work, files visible
- If ADB dropped: "offline" or "device not found"

**How to Fix**:
- Add `adb wait-for-device` before Frida startup
- Add delay (sleep 5) after file push before startup
- Check current entrypoint.sh (it may already have this)

### Hypothesis 3: SELinux or Permission Restrictions

**Theory**: Android 13 SELinux policies prevent Frida execution

**Test Plan**:
```bash
# Check Frida file permissions
docker exec android-mitm-mvp bash -c 'adb shell ls -la /data/local/tmp/frida-server'

# Check SE Linux status
docker exec android-mitm-mvp bash -c 'adb shell getenforce'

# Try executing with adb shell (may give better error)
docker exec android-mitm-mvp bash -c 'adb shell /data/local/tmp/frida-server 2>&1 | tee /tmp/frida-test.log'

# Inspect log
docker exec android-mitm-mvp bash -c 'cat /tmp/frida-test.log'
```

**Expected Results**:
- Permission denied → Need to set permissions
- SELinux blocking → Need to disable or modify policy
- Other error → Better error message for debugging

**How to Fix**:
- Ensure executable bit: `adb shell chmod 755`
- Disable SELinux: `adb shell setenforce 0` (if permitted)
- Use alternative installation location

### Hypothesis 4: Port Binding Issue

**Theory**: Port 27042 (Frida default) is in use or not accessible

**Test Plan**:
```bash
# Check if port is open
docker exec android-mitm-mvp bash -c 'adb shell netstat -tln | grep 27042'

# Check all listening ports
docker exec android-mitm-mvp bash -c 'adb shell netstat -tln | head -20'

# Try Frida with explicit port
docker exec android-mitm-mvp bash -c 'adb shell /data/local/tmp/frida-server -l 0.0.0.0:27043 2>&1 &'
sleep 2
docker exec android-mitm-mvp bash -c 'adb shell netstat -tln | grep 27043'
```

**Expected Results**:
- Port in use → Will see another process
- Port inaccessible → netstat won't show it even if running
- Different port works → Original port is the issue

**How to Fix**:
- Kill conflicting process
- Use different port in Frida startup
- Check if device has port binding restrictions

### Hypothesis 5: Frida Version Mismatch

**Theory**: Host frida-ps version doesn't match device frida-server version

**Test Plan**:
```bash
# Check frida-ps version on host
frida-ps --version

# Check frida-server version (might crash trying to run)
docker exec android-mitm-mvp bash -c 'adb shell /data/local/tmp/frida-server --version 2>&1'

# Check if they're compatible
# frida-ps from version X can connect to frida-server from version X or X±minor

# Try using frida CLI directly to spawn
docker exec android-mitm-mvp bash -c 'frida -f com.android.chrome --no-pause 2>&1 | head -20'
```

**Expected Results**:
- Version mismatch → Error message about incompatibility
- Match → Connection succeeds
- Different approach → Spawn mode might work when server mode doesn't

**How to Fix**:
- Match versions: `pip install frida==X.X.X`
- Download corresponding server binary
- Use spawn mode instead of server mode

## Debugging Steps (Sequential)

### Step 0: Restart Container (Fresh State)
```bash
gcloud compute ssh android-mitm-mvp --zone=us-central1-a --project=corsali-development

# Remove old container
docker rm -f android-mitm-mvp

# Run fresh container
docker run -d \
  --name android-mitm-mvp \
  --privileged \
  --device /dev/kvm \
  -p 0.0.0.0:6080:6080 \
  -p 0.0.0.0:8081:8081 \
  android-mitm-mvp:latest
```

### Step 1: Monitor Boot
```bash
# Watch container startup
watch -n 2 'docker logs android-mitm-mvp | tail -20'

# Wait for boot to complete (see "Android booted" message)
# This should take 2-3 minutes
```

### Step 2: Capture Frida Failure
```bash
# Once boot complete, wait for Frida to fail
watch -n 5 'docker logs android-mitm-mvp | tail -30'

# Once you see "Frida server failed to start", proceed to debugging
```

### Step 3: Collect Evidence
```bash
# Save container logs
docker logs android-mitm-mvp > /tmp/android-mitm-logs.txt

# Check internal logs (if container still accessible)
# This might fail if container exited too quickly
docker exec android-mitm-mvp bash -c 'cat /var/log/frida-server.log' 2>&1 > /tmp/frida-server.log

# Check if Frida is running on device
docker exec android-mitm-mvp bash -c 'adb shell ps -A | grep frida' > /tmp/frida-processes.txt

# Check listening ports
docker exec android-mitm-mvp bash -c 'adb shell netstat -tln' > /tmp/device-netstat.txt

# Check if file exists
docker exec android-mitm-mvp bash -c 'adb shell ls -la /data/local/tmp/frida-server' > /tmp/frida-file-stat.txt
```

### Step 4: Manual Frida Test
```bash
# If container still running, test Frida manually
docker exec android-mitm-mvp bash -c '
  echo "=== Booting check ==="
  adb shell getprop sys.boot_completed

  echo "=== Frida file check ==="
  adb shell ls -la /data/local/tmp/frida-server

  echo "=== Manual startup with output ==="
  adb shell timeout 5 /data/local/tmp/frida-server 2>&1 || true

  echo "=== Process check ==="
  adb shell ps -A | grep frida || echo "No frida process"

  echo "=== Port check ==="
  adb shell netstat -tln | grep 27042 || echo "Port not found"
'
```

### Step 5: Version Check
```bash
docker exec android-mitm-mvp bash -c 'frida-ps --version'
docker exec android-mitm-mvp bash -c '
  adb shell /data/local/tmp/frida-server --version 2>&1 || echo "Server version command failed"
'
```

## Logging Improvements

### Modify entrypoint.sh to Capture Error Output

Replace lines 240-253 in entrypoint.sh:

```bash
# 5. Start Frida server
echo "[5/8] Starting Frida server..."
adb push /frida-server /data/local/tmp/frida-server >/dev/null
adb shell chmod 755 /data/local/tmp/frida-server >/dev/null

# Start server with output capture
echo "Starting Frida server process..."
adb shell /data/local/tmp/frida-server > "${FRIDA_SERVER_LOG}" 2>&1 &
FRIDA_SERVER_PID=$!

# Give it time to start
sleep 5

# Check if it's running with detailed output
echo "Checking Frida server status..."
if adb shell ps -A 2>/dev/null | grep -q frida-server; then
    echo "✓ Frida server process detected"
else
    echo "⚠️  Frida server process not found in ps output"
fi

# Try frida-ps with detailed output
if frida-ps -U >/dev/null 2>&1; then
    echo "✓ Frida server running"
else
    echo "✗ Frida-ps check failed"
    echo "Attempting frida-ps with error output:"
    frida-ps -U 2>&1 || true

    # Additional debugging
    echo ""
    echo "=== Debug Information ==="
    echo "ADB devices:"
    adb devices
    echo ""
    echo "Device boot status:"
    adb shell getprop sys.boot_completed || true
    echo ""
    echo "Frida server log:"
    cat "${FRIDA_SERVER_LOG}" || echo "Log file not accessible"
    echo ""
    echo "Device netstat (port 27042):"
    adb shell netstat -tln 2>&1 | grep -E "27042|LISTEN" || echo "Port not found"

    exit 1
fi
```

## Quick Diagnostic Commands

Run these in order when container is running/stuck at Frida step:

```bash
# 1. Basic connectivity
gcloud compute ssh android-mitm-mvp --zone=us-central1-a --project=corsali-development \
  -- docker exec android-mitm-mvp bash -c 'adb devices'

# 2. Boot status
gcloud compute ssh android-mitm-mvp --zone=us-central1-a --project=corsali-development \
  -- docker exec android-mitm-mvp bash -c 'adb shell getprop sys.boot_completed'

# 3. File check
gcloud compute ssh android-mitm-mvp --zone=us-central1-a --project=corsali-development \
  -- docker exec android-mitm-mvp bash -c 'adb shell ls -la /data/local/tmp/ | grep frida'

# 4. Manual startup
gcloud compute ssh android-mitm-mvp --zone=us-central1-a --project=corsali-development \
  -- docker exec android-mitm-mvp bash -c 'adb shell timeout 3 /data/local/tmp/frida-server 2>&1 || true'

# 5. Process check
gcloud compute ssh android-mitm-mvp --zone=us-central1-a --project=corsali-development \
  -- docker exec android-mitm-mvp bash -c 'adb shell ps -A | grep frida || echo "Not found"'
```

## Expected Outcomes

Depending on which hypothesis is correct, you'll see:

| Issue | Error Message | Fix |
|-------|---------------|-----|
| Binary incompatible | "Permission denied" or "Bad magic number" | Download correct binary |
| ADB disconnected | "offline" or "no devices found" | Restart ADB, wait for reconnect |
| SELinux blocking | "Permission denied" or "Access denied" | Relax SELinux or move binary |
| Port in use | Frida silent (no output), port bound to other process | Kill conflicting process or use different port |
| Version mismatch | "Protocol error" or "Version mismatch" | Match frida-ps and frida-server versions |

## Resolution Checklist

- [ ] Collected all debug logs and error messages
- [ ] Ran all diagnostic commands above
- [ ] Identified which hypothesis matches symptoms
- [ ] Applied appropriate fix
- [ ] Verified fix works with manual test
- [ ] Updated entrypoint.sh with better logging
- [ ] Tested full E2E flow
- [ ] Documented solution

## Resources

- Frida Documentation: https://frida.re/
- Android SELinux: https://source.android.com/security/selinux
- Android 13 Changes: https://developer.android.com/about/versions/13
- Emulator Debugging: https://developer.android.com/studio/run/emulator-commandline
