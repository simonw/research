#!/bin/bash
set -e

echo "=========================================="
echo " Dockerify Android + MITM Setup"
echo "=========================================="

# Configuration
export ANDROID_PROXY_HOST=${ANDROID_PROXY_HOST:-"10.0.2.2"}
export ANDROID_PROXY_PORT=${ANDROID_PROXY_PORT:-"8080"}

# [1/7] Start mitmproxy web interface
echo "[1/7] Starting mitmproxy..."
mitmweb \
    --web-host 0.0.0.0 \
    --web-port 8081 \
    --listen-host 0.0.0.0 \
    --listen-port 8080 \
    --ssl-insecure \
    --set block_global=false \
    --set web_password='mitmproxy' \
    --no-web-open-browser \
    > /var/log/mitmproxy.log 2>&1 &

echo "Waiting for mitmproxy to generate certificate..."
for i in $(seq 1 15); do
    if [ -f /root/.mitmproxy/mitmproxy-ca-cert.pem ]; then
        echo "✓ mitmproxy started (web UI: http://localhost:8081)"
        break
    fi
    sleep 1
done

# [2/7] Start supervisord (manages Android emulator + ws-scrcpy)
echo "[2/7] Starting Android emulator via supervisord..."
/usr/bin/supervisord -c /etc/supervisor/conf.d/supervisord.conf &
SUPERVISORD_PID=$!

# [3/7] Wait for Android to boot
echo "[3/7] Waiting for Android to boot completely..."
adb wait-for-device
sleep 10  # Give it a moment after device appears

# Wait for boot completion
BOOT_WAIT=0
MAX_WAIT=180
while [ $BOOT_WAIT -lt $MAX_WAIT ]; do
    BOOT_COMPLETED=$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' || echo "")
    if [ "$BOOT_COMPLETED" = "1" ]; then
        echo "✓ Android boot completed (${BOOT_WAIT}s)"
        break
    fi
    if [ $((BOOT_WAIT % 30)) -eq 0 ] && [ $BOOT_WAIT -gt 0 ]; then
        echo "  [${BOOT_WAIT}s] Still waiting for boot..."
    fi
    sleep 3
    BOOT_WAIT=$((BOOT_WAIT + 3))
done

# [4/7] Install mitmproxy CA certificate
echo "[4/7] Installing mitmproxy CA certificate..."
if [ ! -f /root/.mitmproxy/mitmproxy-ca-cert.pem ]; then
    echo "⚠️  Certificate not found, skipping installation"
else
    # Generate hash for certificate
    CERT_HASH=$(openssl x509 -inform PEM -subject_hash_old -in /root/.mitmproxy/mitmproxy-ca-cert.pem 2>/dev/null | head -1)

    # Since dockerify has root/Magisk, we can try system installation
    adb root 2>/dev/null || echo "  Already running as root"
    sleep 2
    adb remount 2>/dev/null || echo "  Remount may have failed, continuing..."

    # Push to system certs
    adb push /root/.mitmproxy/mitmproxy-ca-cert.pem /sdcard/${CERT_HASH}.0 2>/dev/null
    adb shell "su 0 sh -c 'mount -o rw,remount /system'" 2>/dev/null || true
    adb shell "su 0 sh -c 'cp /sdcard/${CERT_HASH}.0 /system/etc/security/cacerts/'" 2>/dev/null || true
    adb shell "su 0 sh -c 'chmod 644 /system/etc/security/cacerts/${CERT_HASH}.0'" 2>/dev/null || true

    # If system installation fails, fall back to user certs
    if [ $? -ne 0 ]; then
        echo "  System cert installation failed, using user cert store..."
        adb shell "mkdir -p /data/local/tmp/certs"
        adb push /root/.mitmproxy/mitmproxy-ca-cert.pem /data/local/tmp/certs/${CERT_HASH}.0
        adb shell "su 0 sh -c 'cp /data/local/tmp/certs/${CERT_HASH}.0 /data/misc/user/0/cacerts-added/'"
        adb shell "su 0 sh -c 'chmod 644 /data/misc/user/0/cacerts-added/${CERT_HASH}.0'"
    fi

    echo "✓ Certificate installed (hash: ${CERT_HASH})"
fi

# [5/7] Configure iptables for system-wide traffic redirection
echo "[5/7] Configuring iptables for system-wide traffic redirection..."
adb shell "su 0 sh -c 'echo 1 > /proc/sys/net/ipv4/ip_forward'" 2>/dev/null || true
adb shell "su 0 sh -c 'iptables -w 10 -t nat -F OUTPUT'" 2>/dev/null || true
adb shell "su 0 sh -c 'iptables -w 10 -t nat -A OUTPUT -p tcp --dport 80 -j DNAT --to-destination ${ANDROID_PROXY_HOST}:${ANDROID_PROXY_PORT}'" && \
adb shell "su 0 sh -c 'iptables -w 10 -t nat -A OUTPUT -p tcp --dport 443 -j DNAT --to-destination ${ANDROID_PROXY_HOST}:${ANDROID_PROXY_PORT}'" && \
echo "✓ iptables rules configured" || echo "⚠️  iptables configuration may have failed"

# Verify iptables rules
IPTABLES_RULES=$(adb shell "su -c 'iptables -t nat -L OUTPUT -n'" 2>/dev/null | grep DNAT || echo "")
if echo "$IPTABLES_RULES" | grep -q "${ANDROID_PROXY_HOST}"; then
    echo "✓ iptables system-wide redirection active"
else
    echo "⚠️  iptables rules verification failed"
fi

# [6/7] Start Frida server
echo "[6/7] Starting Frida server..."
# Download and install frida-server for Android
FRIDA_VERSION="16.5.9"
ARCH="x86_64"
FRIDA_SERVER_URL="https://github.com/frida/frida/releases/download/${FRIDA_VERSION}/frida-server-${FRIDA_VERSION}-android-${ARCH}.xz"

if [ ! -f /tmp/frida-server ]; then
    wget -q "${FRIDA_SERVER_URL}" -O /tmp/frida-server.xz
    unxz /tmp/frida-server.xz
    chmod +x /tmp/frida-server
fi

adb push /tmp/frida-server /data/local/tmp/ 2>/dev/null
adb shell "su 0 sh -c 'chmod 755 /data/local/tmp/frida-server'" 2>/dev/null
adb shell "su 0 sh -c '/data/local/tmp/frida-server &'" 2>/dev/null &

sleep 3
echo "✓ Frida server started"

# [7/8] Configure Frida scripts
echo "[7/8] Configuring Frida scripts..."
# Update config.js with proxy settings and certificate
if [ -f /root/frida-scripts/config.js.template ]; then
    CERT_CONTENT=$(cat /root/.mitmproxy/mitmproxy-ca-cert.pem)
    sed -e "s|PROXY_HOST.*|PROXY_HOST = '${ANDROID_PROXY_HOST}';|" \
        -e "s|PROXY_PORT.*|PROXY_PORT = ${ANDROID_PROXY_PORT};|" \
        /root/frida-scripts/config.js.template > /root/frida-scripts/config.js

    # Insert certificate content
    python3 - <<'PY'
import re
config_path = '/root/frida-scripts/config.js'
cert_path = '/root/.mitmproxy/mitmproxy-ca-cert.pem'

with open(cert_path) as f:
    cert_content = f.read()

with open(config_path) as f:
    config = f.read()

# Replace the placeholder certificate
config = re.sub(
    r"const CERT_PEM = \`[^`]*\`;",
    f"const CERT_PEM = \`{cert_content}\`;",
    config
)

with open(config_path, 'w') as f:
    f.write(config)
PY
    echo "✓ Frida scripts configured"
fi

# [8/8] Install Google Apps (YouTube and other APKs)
echo "[8/8] Installing Google Apps..."
cd /tmp

# Install all APKs from /tmp/apks/ directory (pre-baked into image)
if [ -d /tmp/apks ] && [ -n "$(ls -A /tmp/apks/*.apk 2>/dev/null)" ]; then
    echo "  Found pre-baked APKs in /tmp/apks/"
    for apk in /tmp/apks/*.apk; do
        if [ -f "$apk" ]; then
            apk_size=$(stat -c%s "$apk" 2>/dev/null || stat -f%z "$apk" 2>/dev/null || echo 0)
            # Check if file is a valid APK (at least 1MB, or check magic bytes)
            if [ "$apk_size" -gt 1000000 ] || [ "$(head -c 2 "$apk" 2>/dev/null | od -An -tx1 | tr -d ' ')" = "504b" ]; then
                apk_name=$(basename "$apk")
                echo "  Installing $apk_name..."
                # Use adb install -r to replace if exists, -g to grant all permissions
                install_output=$(adb install -r -g "$apk" 2>&1)
                if echo "$install_output" | grep -qE "(Success|INSTALL_SUCCEEDED|already installed)"; then
                    echo "  ✓ $apk_name installed successfully"
                else
                    echo "  ⚠️  $apk_name installation: $(echo "$install_output" | tail -1)"
                fi
            else
                echo "  ⚠️  Skipping $apk_name (invalid APK file, size: ${apk_size} bytes)"
            fi
        fi
    done
fi

# Also check for APKs in /tmp (for manual installation)
if [ -f /tmp/youtube.apk ]; then
    apk_size=$(stat -c%s /tmp/youtube.apk 2>/dev/null || stat -f%z /tmp/youtube.apk 2>/dev/null || echo 0)
    if [ "$apk_size" -gt 1000000 ]; then
        if ! adb shell pm list packages | grep -q "com.google.android.youtube"; then
            echo "  Installing YouTube from /tmp/youtube.apk..."
            if adb install -r -g /tmp/youtube.apk 2>&1 | grep -qE "(Success|INSTALL_SUCCEEDED)"; then
                echo "  ✓ YouTube installed successfully"
            else
                echo "  ⚠️  YouTube installation failed"
            fi
        else
            echo "  ✓ YouTube already installed"
        fi
    fi
fi

# [9/9] Start ws-scrcpy web UI
echo "[9/9] Starting ws-scrcpy web UI..."
if [ -d /root/ws-scrcpy/dist ] && [ -f /root/ws-scrcpy/dist/index.js ]; then
    cd /root/ws-scrcpy/dist
    nohup node index.js --port 8000 > /var/log/ws-scrcpy.log 2>&1 &
    WS_SCRCPY_PID=$!
    sleep 5
    if ps -p $WS_SCRCPY_PID > /dev/null 2>&1; then
        echo "✓ ws-scrcpy started (web UI: http://localhost:8000)"
    else
        echo "⚠️  ws-scrcpy may have failed to start (check /var/log/ws-scrcpy.log)"
    fi
else
    echo "⚠️  ws-scrcpy not found, skipping web UI"
fi

echo ""
echo "=========================================="
echo "🎉 Setup complete!"
echo "=========================================="
echo "Services:"
echo "  - ws-scrcpy:  http://localhost:8000"
echo "  - mitmproxy:  http://localhost:8081 (password: mitmproxy)"
echo "  - ADB:        adb connect localhost:5555"
echo ""
echo "Proxy: ${ANDROID_PROXY_HOST}:${ANDROID_PROXY_PORT}"
echo "Frida: Available for app hooking"
echo "iptables: System-wide traffic redirection active"
echo "=========================================="

# Keep container running by waiting on supervisord
wait $SUPERVISORD_PID
