# Android Boot Issue - Summary & Next Steps

## Problem
The Android emulator container gets stuck during boot wait, preventing the script from proceeding to certificate installation, proxy configuration, and app launch.

## What We've Tried (All Failed)

### 1. **Boot Wait Timeout Reduction**
- **Attempt**: Reduced timeout from 240s to 90s
- **Result**: Script still hangs before reaching timeout logic
- **Issue**: Script never gets past `adb wait-for-device` call

### 2. **Device State Detection**
- **Attempt**: Added logic to wait for device to transition from "offline" to "device" state
- **Result**: Script gets stuck at `adb wait-for-device` which blocks indefinitely
- **Issue**: `adb wait-for-device` waits for device to appear, but device stays "offline" for 1-2 minutes

### 3. **Loop Structure Changes**
- **Attempt**: Changed from `while true` to `for i in {1..30}` to guarantee exit
- **Result**: Loop never executes because script hangs before reaching it
- **Issue**: Blocking happens before the loop starts

### 4. **Output Buffering & Debugging**
- **Attempt**: Added progress messages, debug output every 5 iterations
- **Result**: No output appears because script hangs earlier
- **Issue**: `adb wait-for-device` blocks without producing output

### 5. **Error Handling**
- **Attempt**: Added `set +e` around adb commands, `|| true` fallbacks
- **Result**: Doesn't help because `adb wait-for-device` succeeds (device appears) but device stays offline
- **Issue**: The command succeeds but device state is "offline", not "device"

## Root Cause Analysis

The fundamental issue is:
1. `adb wait-for-device` returns successfully when device appears (even if offline)
2. Device appears as "offline" for 1-2 minutes before transitioning to "device" state
3. Our script tries to check device state AFTER `adb wait-for-device`, but the blocking call prevents progress
4. The original mvp-guide.md approach was simpler: `adb wait-for-device` → `sleep 15` → check boot_completed

## Current State
- Container starts successfully
- Mitmproxy starts (port 8081)
- Emulator process launches
- Device appears in `adb devices` but shows as "offline"
- Script hangs at `adb wait-for-device` line (line 79)
- Never reaches device state checking or boot completion logic

## Suggested Next Steps

### Option 1: Simplify to Original Approach (Recommended)
Revert to the simpler approach from mvp-guide.md:
```bash
adb wait-for-device
sleep 15  # Give device time to come online
# Then check boot_completed
```
This worked before and is simpler. The device will come online during the sleep period.

### Option 2: Non-Blocking Device Wait
Replace `adb wait-for-device` with a polling loop:
```bash
# Poll for device instead of blocking wait
for i in {1..60}; do
    if adb devices | grep -q "device$"; then
        break
    fi
    sleep 2
done
```

### Option 3: Use Base Image's Built-in Wait
Check if the docker-android base image has a built-in wait mechanism we should use instead of our custom logic.

### Option 4: Increase Initial Sleep
After `adb wait-for-device`, increase the sleep to 60-90 seconds to allow device to fully come online before checking boot state.

### Option 5: Check Base Image Documentation
Review budtmo/docker-android documentation for recommended boot wait patterns. The base image may have supervisor scripts that handle this.

## Recommendation

**Try Option 1 first** - revert to the simpler approach that worked in mvp-guide.md. It's less "smart" but more reliable. The key insight is that `adb wait-for-device` + a fixed sleep period is more reliable than trying to detect device state transitions.

