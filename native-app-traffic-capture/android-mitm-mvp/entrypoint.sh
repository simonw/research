#!/bin/bash
# Entrypoint for Android MITM MVP container
set -euo pipefail

APP_PACKAGE=${APP_PACKAGE:-"com.android.chrome"}
DEVICE=${DEVICE:-"Samsung Galaxy S23"}
FRIDA_SCRIPTS_DIR=/frida-scripts
LOG_DIR=/var/log
MITM_LOG=${LOG_DIR}/mitmproxy.log
FRIDA_SERVER_LOG=${LOG_DIR}/frida-server.log
FRIDA_APP_LOG=${LOG_DIR}/frida-app.log
SUPERVISOR_LOG=${LOG_DIR}/supervisord.log

export PATH="${PATH}:/opt/android/platform-tools"

RUN_SCRIPT=${RUN_SCRIPT:-"${APP_PATH:-/home/androidusr/docker-android}/mixins/scripts/run.sh"}

mkdir -p "${LOG_DIR}"

if [ ! -e /dev/kvm ]; then
python3 - <<'PY'
from pathlib import Path

emulator_path = Path("/home/androidusr/docker-android/cli/src/device/emulator.py")
text = emulator_path.read_text()

if "-accel on" in text:
    text = text.replace("-accel on", "-accel off")

missing_kvm_msg = 'raise RuntimeError("/dev/kvm cannot be found!")'
replacement_msg = 'self.logger.warning("/dev/kvm cannot be found! Continuing without hardware acceleration.")'
if missing_kvm_msg in text:
    text = text.replace(missing_kvm_msg, replacement_msg)

emulator_path.write_text(text)
PY
    echo "⚠️  /dev/kvm not found; forcing software acceleration."
else
    echo "✓ /dev/kvm detected; keeping hardware acceleration enabled."
fi

cat <<'BANNER'
==========================================
Starting Android MITM MVP
==========================================
BANNER
echo "App: ${APP_PACKAGE}" | tee -a "${LOG_DIR}/startup.log"
echo "Device profile: ${DEVICE}" | tee -a "${LOG_DIR}/startup.log"

# 1. Start mitmproxy web interface
echo "[1/8] Starting mitmproxy..."
mitmweb \
    --web-host 0.0.0.0 \
    --web-port 8081 \
    --listen-host 0.0.0.0 \
    --listen-port 8080 \
    --ssl-insecure \
    --set block_global=false \
    --set web_password='mitmproxy' \
    --no-web-open-browser \
    > "${MITM_LOG}" 2>&1 &
MITM_PROXY_PID=$!

echo "Waiting for mitmproxy to initialize..."
for _ in $(seq 1 10); do
    if [ -f /root/.mitmproxy/mitmproxy-ca-cert.pem ]; then
        break
    fi
    sleep 1
done

echo "✓ mitmproxy started (web UI: http://localhost:8081)"

# 2. Start Android emulator
echo "[2/8] Starting Android emulator..."
export DEVICE
if [ ! -x "${RUN_SCRIPT}" ]; then
    echo "✗ Emulator bootstrap script not found at ${RUN_SCRIPT}"
    exit 1
fi

"${RUN_SCRIPT}" > "${SUPERVISOR_LOG}" 2>&1 &
RUN_SCRIPT_PID=$!

echo "Waiting for Android to boot (this may take ~60-90 seconds)..."
# Wait for device to appear (may be offline initially)
adb wait-for-device >/dev/null 2>&1 || true
echo "Device detected, waiting for it to come online..."

# Wait for device to transition from "offline" to "device" state
# This can take 1-2 minutes on cloud VMs without hardware acceleration
ONLINE_WAIT=0
MAX_ONLINE_WAIT=120  # 2 minutes for device to come online
DEVICE_STATE="unknown"
while [ ${ONLINE_WAIT} -lt ${MAX_ONLINE_WAIT} ]; do
    # Get device state - look for emulator-5554 line and extract state
    DEVICE_LINE=$(adb devices 2>/dev/null | grep "emulator" | head -1 || echo "")
    if [ -n "${DEVICE_LINE}" ]; then
        DEVICE_STATE=$(echo "${DEVICE_LINE}" | awk '{print $2}' || echo "unknown")
    else
        DEVICE_STATE="unknown"
    fi
    
    if [ "${DEVICE_STATE}" = "device" ]; then
        echo "✓ ADB device is online (after ${ONLINE_WAIT}s)"
        break
    fi
    if [ $((ONLINE_WAIT % 30)) -eq 0 ] && [ ${ONLINE_WAIT} -gt 0 ]; then
        echo " [${ONLINE_WAIT}s] Device still ${DEVICE_STATE}, waiting..."
    fi
    sleep 3
    ONLINE_WAIT=$((ONLINE_WAIT + 3))
done

if [ "${DEVICE_STATE}" != "device" ]; then
    echo "⚠️  Device still ${DEVICE_STATE} after ${ONLINE_WAIT}s, proceeding anyway..."
fi

echo "Starting boot completion check..."

BOOT_TIMEOUT=0
MAX_BOOT_WAIT=300
BOOT_COMPLETED=""
BOOTANIM_STATE=""
ITERATION=0

# Use a for loop with explicit iteration count to guarantee exit
# Use brace expansion instead of seq for better compatibility  
echo "Boot check loop starting (max ${MAX_BOOT_WAIT}s)..."
for i in {1..100}; do
    ITERATION=$i
    BOOT_TIMEOUT=$((i * 3))
    
    # Temporarily disable exit on error for adb commands
    set +e
    BOOT_COMPLETED="$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' || echo '')"
    BOOTANIM_STATE="$(adb shell getprop init.svc.bootanim 2>/dev/null | tr -d '\r' || echo '')"
    set -e
    
    # Always show progress every iteration for debugging
    echo -n "."
    
    # Debug output every 5 iterations
    if [ $((i % 5)) -eq 0 ]; then
        echo ""
        echo " [${BOOT_TIMEOUT}s] boot_completed=${BOOT_COMPLETED:-empty}, bootanim=${BOOTANIM_STATE:-empty}"
    fi
    
    if [ "${BOOT_COMPLETED}" = "1" ] || [ "${BOOTANIM_STATE}" = "stopped" ]; then
        echo ""
        echo "✓ Android booted (boot_completed=${BOOT_COMPLETED}, bootanim=${BOOTANIM_STATE})"
        break
    fi
    
    # Inject key events early to help with boot progression
    if [ "${BOOT_TIMEOUT}" -eq 30 ]; then
        echo ""
        echo " [30s] Injecting wake event..."
        adb shell input keyevent 224 >/dev/null 2>&1 || true
    fi
    if [ "${BOOT_TIMEOUT}" -eq 45 ]; then
        echo ""
        echo " [45s] Injecting unlock/home events..."
        adb shell input keyevent 82 >/dev/null 2>&1 || true
        adb shell input keyevent 3 >/dev/null 2>&1 || true
    fi
    
    if [ "${BOOT_TIMEOUT}" -ge "${MAX_BOOT_WAIT}" ]; then
        echo ""
        echo "⚠️  Boot detection timed out after ${BOOT_TIMEOUT}s; continuing anyway"
        break
    fi
    
    sleep 3
done

echo ""
echo "✓ Boot wait completed after ${ITERATION} iterations (${BOOT_TIMEOUT}s)"
echo "✓ Proceeding with setup (boot_completed=${BOOT_COMPLETED:-not_set}, bootanim=${BOOTANIM_STATE:-unknown})"
sleep 60

# 2b. Unlock device, dismiss setup wizard, and return to home screen
echo "[2b/8] Unlocking device and dismissing setup wizard..."
adb shell input keyevent 224 >/dev/null 2>&1 || true   # Wake device
sleep 1
adb shell input keyevent 82 >/dev/null 2>&1 || true    # Unlock (menu)
sleep 1
adb shell input keyevent 3 >/dev/null 2>&1 || true     # Home
sleep 1
# Dismiss setup wizard with multiple strategies
adb shell input keyevent 4 >/dev/null 2>&1 || true     # Back button
sleep 0.5
adb shell input tap 700 2000 >/dev/null 2>&1 || true  # Tap "Skip" if visible (bottom right)
sleep 0.5
adb shell input tap 500 1800 >/dev/null 2>&1 || true   # Tap "Next" if visible (bottom center)
sleep 0.5
adb shell input keyevent 4 >/dev/null 2>&1 || true     # Back again
sleep 0.5
adb shell input keyevent 3 >/dev/null 2>&1 || true     # Home to ensure we're on launcher
sleep 1
adb shell input swipe 500 1600 500 400 200 >/dev/null 2>&1 || true  # Swipe up to dismiss any overlays
echo "✓ Device unlocked and setup wizard dismissed"

# Reduce UI animation overhead for responsiveness
echo "[2c/8] Disabling system animations for performance..."
adb shell settings put global window_animation_scale 0 >/dev/null 2>&1 || true
adb shell settings put global transition_animation_scale 0 >/dev/null 2>&1 || true
adb shell settings put global animator_duration_scale 0 >/dev/null 2>&1 || true
echo "✓ System animations disabled"

# 3. Install system CA certificate
echo "[3/8] Installing mitmproxy CA as system certificate..."
adb root >/dev/null 2>&1 || true
adb wait-for-device >/dev/null 2>&1
sleep 2
adb remount >/dev/null 2>&1 || true

CERT_PATH=/root/.mitmproxy/mitmproxy-ca-cert.pem
if [ ! -f "${CERT_PATH}" ]; then
    echo "✗ mitmproxy CA not found at ${CERT_PATH}"
    exit 1
fi

CERT_HASH=$(openssl x509 -inform PEM -subject_hash_old -in "${CERT_PATH}" | head -1)
if [ -z "${CERT_HASH}" ]; then
    echo "✗ Unable to calculate certificate hash"
    exit 1
fi

CERT_REMOTE_TMP=/data/local/tmp/mitmproxy-ca-cert.pem
adb push "${CERT_PATH}" "${CERT_REMOTE_TMP}" >/dev/null
if adb shell "cp ${CERT_REMOTE_TMP} /system/etc/security/cacerts/${CERT_HASH}.0" >/dev/null 2>&1; then
    adb shell "chmod 644 /system/etc/security/cacerts/${CERT_HASH}.0" >/dev/null
    echo "✓ System CA installed (hash: ${CERT_HASH}.0)"
else
    echo "⚠️ Unable to write to /system; falling back to user certificate store"
    adb shell "mkdir -p /data/misc/user/0/cacerts-added" >/dev/null
    adb shell "cp ${CERT_REMOTE_TMP} /data/misc/user/0/cacerts-added/${CERT_HASH}.0" >/dev/null
    adb shell "chmod 644 /data/misc/user/0/cacerts-added/${CERT_HASH}.0" >/dev/null
    adb shell "chown root:root /data/misc/user/0/cacerts-added/${CERT_HASH}.0" >/dev/null
    echo "✓ User CA installed (hash: ${CERT_HASH}.0)"
fi

# 4. Configure proxy
echo "[4/8] Configuring proxy..."
adb shell settings put global http_proxy 127.0.0.1:8080 >/dev/null
adb shell settings put global global_http_proxy_host 127.0.0.1 >/dev/null
adb shell settings put global global_http_proxy_port 8080 >/dev/null

echo "✓ Proxy configured"

# 5. Start Frida server
echo "[5/8] Starting Frida server..."
adb push /frida-server /data/local/tmp/frida-server >/dev/null
adb shell chmod 755 /data/local/tmp/frida-server >/dev/null
adb shell /data/local/tmp/frida-server > "${FRIDA_SERVER_LOG}" 2>&1 &
FRIDA_SERVER_PID=$!

sleep 5
if frida-ps -U >/dev/null 2>&1; then
    echo "✓ Frida server running"
else
    echo "✗ Frida server failed to start"
    exit 1
fi

# 6. Configure Frida scripts with mitmproxy CA
echo "[6/8] Configuring Frida scripts..."
python3 - <<'PY'
from pathlib import Path
import re

config_path = Path("/frida-scripts/config.js")
cert_path = Path("/root/.mitmproxy/mitmproxy-ca-cert.pem")
cert = cert_path.read_text()
config = config_path.read_text()

escaped_cert = cert.replace('`', '\\`')

config = re.sub(
    r"const CERT_PEM = `.*?`;",
    f"const CERT_PEM = `{escaped_cert}`;",
    config,
    flags=re.S
)
config = re.sub(r"const PROXY_HOST = '.*?';", "const PROXY_HOST = '127.0.0.1';", config)
config = re.sub(r"const PROXY_PORT = \d+;", "const PROXY_PORT = 8080;", config)
config = re.sub(r"const DEBUG_MODE = (true|false);", "const DEBUG_MODE = true;", config)
config = re.sub(r"const IGNORED_NON_HTTP_PORTS = \[.*?\];", "const IGNORED_NON_HTTP_PORTS = [];", config, flags=re.S)
config = re.sub(r"const BLOCK_HTTP3 = (true|false);", "const BLOCK_HTTP3 = true;", config)
config = re.sub(r"const PROXY_SUPPORTS_SOCKS5 = (true|false);", "const PROXY_SUPPORTS_SOCKS5 = false;", config)

config_path.write_text(config)
PY

echo "✓ Frida scripts configured"

# 7. Launch app with Frida
echo "[7/8] Launching ${APP_PACKAGE} with Frida unpinning..."
frida -U \
    -l "${FRIDA_SCRIPTS_DIR}/config.js" \
    -l "${FRIDA_SCRIPTS_DIR}/native-connect-hook.js" \
    -l "${FRIDA_SCRIPTS_DIR}/native-tls-hook.js" \
    -l "${FRIDA_SCRIPTS_DIR}/android/android-proxy-override.js" \
    -l "${FRIDA_SCRIPTS_DIR}/android/android-system-certificate-injection.js" \
    -l "${FRIDA_SCRIPTS_DIR}/android/android-certificate-unpinning.js" \
    -l "${FRIDA_SCRIPTS_DIR}/android/android-certificate-unpinning-fallback.js" \
    -l "${FRIDA_SCRIPTS_DIR}/android/android-disable-root-detection.js" \
    -f "${APP_PACKAGE}" \
    --no-pause \
    > "${FRIDA_APP_LOG}" 2>&1 &
FRIDA_APP_PID=$!

sleep 3
echo "✓ App launched with Frida"

# 8. Verify traffic capture setup
echo "[8/8] Verifying traffic capture setup..."
sleep 2

# Check if app is running
if adb shell pidof "${APP_PACKAGE}" >/dev/null 2>&1; then
    echo "✓ ${APP_PACKAGE} is running"
else
    echo "⚠️  ${APP_PACKAGE} may not be running (check logs)"
fi

# Check proxy configuration
PROXY_CHECK=$(adb shell settings get global http_proxy 2>/dev/null | tr -d '\r' || echo "")
if [ -n "${PROXY_CHECK}" ] && [ "${PROXY_CHECK}" != "null" ]; then
    echo "✓ Proxy configured: ${PROXY_CHECK}"
else
    echo "⚠️  Proxy may not be configured correctly"
fi

# Check mitmproxy is responding
if curl -s http://localhost:8081 >/dev/null 2>&1; then
    echo "✓ mitmproxy web UI is accessible"
else
    echo "⚠️  mitmproxy web UI may not be responding"
fi

# Generate a test request if Chrome is running
if [ "${APP_PACKAGE}" = "com.android.chrome" ]; then
    echo "Triggering test navigation in Chrome..."
    adb shell am start -a android.intent.action.VIEW -d "https://www.google.com" "${APP_PACKAGE}" >/dev/null 2>&1 || true
    sleep 2
    echo "✓ Test navigation triggered - check mitmproxy UI for captured traffic"
fi

echo ""
echo "=========================================="
echo "🎉 Setup complete!"
echo "=========================================="
echo ""
echo "Open these URLs in your browser:"
echo "  📱 Android Screen (noVNC): http://localhost:6080"
echo "  🔍 Traffic Inspector (mitmproxy): http://localhost:8081"
echo "     Username: (leave blank)"
echo "     Password: mitmproxy"
echo ""
echo "Traffic Capture Status:"
echo "  - App: ${APP_PACKAGE}"
echo "  - Proxy: 127.0.0.1:8080"
echo "  - Frida: Active with certificate unpinning"
echo ""
echo "Logs are available at:"
echo "  ${MITM_LOG}"
echo "  ${FRIDA_SERVER_LOG}"
echo "  ${FRIDA_APP_LOG}"
echo "  ${SUPERVISOR_LOG}"
echo ""
echo "To capture traffic:"
echo "  1. Open mitmproxy UI (http://localhost:8081)"
echo "  2. Navigate in the Android app (via noVNC or automated)"
echo "  3. View decrypted HTTPS traffic in mitmproxy"
echo ""
echo "Press Ctrl+C to stop. Keeping container alive by tailing logs..."

tail -F "${MITM_LOG}" "${FRIDA_APP_LOG}" "${SUPERVISOR_LOG}"
