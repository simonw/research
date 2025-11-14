# Android MITM MVP - Deployment Checklist

## Pre-Deployment Verification

- [x] Root cause identified: Proxy host misconfiguration (10.0.2.2 → 127.0.0.1)
- [x] Fix implemented in entrypoint.sh line 251
- [x] Investigation documented in notes.md and README.md
- [x] Changes committed to git (feat/android-mitm-mvp branch)
- [x] Backward compatibility verified (environment variable override works)

## Deployment Steps

### Step 1: Build New Docker Image

```bash
# SSH into GCP VM
gcloud compute ssh android-mitm-mvp \
  --zone us-central1-a \
  --project corsali-development \
  --ssh-key-file ~/.ssh/android-mitm-mvp

# On VM: Pull latest code and rebuild
cd /path/to/android-mitm-mvp
git pull origin feat/android-mitm-mvp
sudo docker build -t android-mitm-mvp:fixed-proxy .
```

### Step 2: Verify Fix Was Applied

```bash
# Verify the fix is in the Docker image
sudo docker run --rm android-mitm-mvp:fixed-proxy \
  grep "ANDROID_PROXY_HOST" /entrypoint.sh | grep "127.0.0.1"

# Expected output:
# ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"127.0.0.1"}
```

### Step 3: Stop Current Container

```bash
sudo docker stop android-mitm-mvp
sudo docker rm android-mitm-mvp
```

### Step 4: Launch New Container

```bash
sudo docker run -d \
  --name android-mitm-mvp \
  --privileged \
  --device /dev/kvm \
  -e WEB_VNC=true \
  -e APP_PACKAGE="com.android.chrome" \
  -e EMULATOR_ADDITIONAL_ARGS="-cores 4 -memory 8192 -no-snapshot -no-boot-anim -noaudio -gpu swiftshader_indirect" \
  -e EMULATOR_DATA_PARTITION="2048m" \
  -p 0.0.0.0:6080:6080 \
  -p 0.0.0.0:8081:8081 \
  android-mitm-mvp:fixed-proxy
```

### Step 5: Monitor Startup (5-10 minutes)

```bash
# Watch logs for success indicators
sudo docker logs -f android-mitm-mvp

# Look for these lines (in order):
# ✓ mitmproxy started
# ✓ Android booted
# ✓ Proxy configured: 127.0.0.1:8080
# ✓ Frida server running
# ✓ App launched with Frida
# ✓ Setup complete!
```

## Verification Tests

### Test 1: Proxy Configuration

```bash
# Check ADB can see the proxy
adb shell settings get global http_proxy

# Expected output: 127.0.0.1:8080
```

### Test 2: Network Connectivity

```bash
# Test HTTP connectivity through proxy
adb shell curl -x 127.0.0.1:8080 http://httpbin.org/ip

# Expected: JSON response with IP address
```

### Test 3: Frida Server Status

```bash
# On host (outside container)
frida-ps -U

# Expected: List of Android processes (no "unable to connect" error)
```

### Test 4: mitmproxy Web UI

```bash
# Test web UI accessibility
curl -s http://34.42.16.156:8081 | head -20

# Expected: HTML response (web interface loads)
```

### Test 5: Traffic Capture

```bash
# In Android via noVNC (http://34.42.16.156:6080):
# 1. Open Chrome
# 2. Navigate to https://httpbin.org/status/200
# 3. Check mitmproxy for captured traffic

curl http://34.42.16.156:8081/flows

# Expected: JSON with list of captured flows including httpbin.org request
```

## Success Indicators

All of these should be TRUE after deployment:

- [ ] Container starts without errors
- [ ] Logs show "✓ Setup complete!"
- [ ] mitmproxy web UI loads (http://34.42.16.156:8081)
- [ ] Android proxy is 127.0.0.1:8080
- [ ] Android can reach external URLs (curl test passes)
- [ ] Frida server is running (frida-ps -U returns process list)
- [ ] mitmproxy captures HTTPS traffic from Android
- [ ] Certificate pinning bypass active for Chrome
- [ ] No "connection refused" errors in logs

## Troubleshooting Guide

### Issue: "unable to connect to remote frida-server"

**Cause**: Likely still on old image without fix
**Fix**: 
```bash
# Verify correct image is running
sudo docker ps | grep android-mitm-mvp
# Should show android-mitm-mvp:fixed-proxy

# Rebuild if needed
sudo docker build -t android-mitm-mvp:fixed-proxy .
```

### Issue: "Proxy may not be configured correctly" warning

**Cause**: ADB can't communicate with Android system
**Fix**:
```bash
# Restart adb
adb kill-server
adb devices

# Check Android is online
adb shell echo "test"
```

### Issue: mitmproxy shows no traffic

**Cause**: Proxy might be listening on wrong interface
**Fix**:
```bash
# Verify mitmproxy is listening on 0.0.0.0:8080
sudo docker exec android-mitm-mvp ss -tlnp | grep 8080
# Should show: 0.0.0.0:8080 LISTEN
```

### Issue: Container fails to start

**Cause**: Possible conflicts or resource issues
**Fix**:
```bash
# Clean up and try again
sudo docker stop android-mitm-mvp || true
sudo docker rm android-mitm-mvp || true
sudo docker system prune -f
# Then re-run the launch command
```

## Rollback Plan

If anything goes wrong:

```bash
# Stop new container
sudo docker stop android-mitm-mvp
sudo docker rm android-mitm-mvp

# Find previous working image
sudo docker images | grep android-mitm-mvp

# Run previous version
sudo docker run -d \
  --name android-mitm-mvp \
  --privileged \
  --device /dev/kvm \
  -e WEB_VNC=true \
  -e APP_PACKAGE="com.android.chrome" \
  -p 0.0.0.0:6080:6080 \
  -p 0.0.0.0:8081:8081 \
  android-mitm-mvp:latest  # Or specific previous tag
```

The fix is backward compatible via environment variable, so there's no risk of breaking existing deployments if reverted.

## Performance Expectations

**Expected startup time**: 5-10 minutes
- ~30-60s: mitmproxy startup
- ~1-2m: Android emulator boot
- ~1-2m: Certificate installation and setup
- ~1m: Frida server initialization
- ~30-60s: App launch and connection

**Expected traffic rate**: 
- Initial: 1-5 requests/sec during app startup
- Steady state: Varies by app usage
- All traffic: Visible in mitmproxy UI (decrypted HTTPS)

## Documentation References

- Full analysis: `android-mitm-e2e-debug/README.md`
- Investigation notes: `android-mitm-e2e-debug/notes.md`
- Code changes: `android-mitm-e2e-debug/entrypoint.diff`

## Success Confirmation

Once deployment is complete and all tests pass:

1. Create a screenshot or screen recording showing:
   - mitmproxy web UI with captured traffic
   - Android browser making HTTPS request
   - Decrypted traffic visible in mitmproxy

2. Document:
   - Deployment date and time
   - VM instance name and IP
   - Any issues encountered and how resolved
   - Performance observations

3. Notify team of successful deployment

## Sign-Off

- [ ] Fix committed to feat/android-mitm-mvp branch
- [ ] Image built and tested
- [ ] All verification tests pass
- [ ] Documentation updated
- [ ] Team notified of deployment

**Estimated deployment time**: 20-30 minutes total
**Estimated test time**: 15-20 minutes (includes Android boot time)
**Total time from fix to verified deployment**: 45-60 minutes

