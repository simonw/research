/**
 * Generates the VM startup script that:
 * 1. Installs Docker
 * 2. Loads kernel modules (binder_linux, ashmem)
 * 3. Sets up ReDroid (Android 13)
 * 4. Builds and runs ws-scrcpy from source for browser access
 * 5. Starts Cloudflare Tunnel for valid HTTPS without firewall changes
 * 6. Schedules auto-shutdown after TTL
 * 7. Reports progress via /status.json for frontend polling
 * 
 * OPTIMIZATIONS APPLIED:
 * - Removed apt-get upgrade (saves ~60s)
 * - Parallelized downloads during Android boot wait
 * - Removed wasteful sleep 30, using faster polling (saves ~30s)
 * - Download cloudflared early in background
 */
export function generateStartupScript(ttlMinutes: number): string {
  return `#!/bin/bash
set -e

# Log everything for debugging
exec > >(tee /var/log/startup-script.log) 2>&1
echo "Starting ReDroid setup at $(date)"
START_TIME=$(date +%s)

# Progress tracking function
# Writes status to /var/www/html/status.json for frontend polling
mkdir -p /var/www/html
update_status() {
  local stage="$1"
  local step="$2"
  local total_steps="$3"
  local message="$4"
  local tunnel_url="\${5:-}"
  local percent=$((step * 100 / total_steps))
  local timestamp=$(date +%s000)
  local elapsed=$(($(date +%s) - START_TIME))
  
  cat > /var/www/html/status.json << EOF
{
  "stage": "$stage",
  "step": $step,
  "totalSteps": $total_steps,
  "message": "$message",
  "percent": $percent,
  "timestamp": $timestamp,
  "tunnelUrl": "$tunnel_url",
  "elapsedSeconds": $elapsed
}
EOF
  echo "[PROGRESS] Step \$step/\$total_steps (\$percent%) [\${elapsed}s]: \$message"
}

TOTAL_STEPS=13

# Start a simple HTTP server to serve status.json for progress polling
# This runs in the background on port 8080
cd /var/www/html
nohup python3 -m http.server 8080 --bind 0.0.0.0 > /var/log/status-server.log 2>&1 &
cd /

# Open firewall for status server
ufw allow 8080/tcp || true

# Step 1: Update system (skip upgrade for ephemeral VMs - saves ~60s)
update_status "updating_system" 1 $TOTAL_STEPS "Updating system packages..."
apt-get update
# OPTIMIZATION: Skip apt-get upgrade for ephemeral VMs - base image is recent enough

# Step 2: Install Docker
update_status "installing_docker" 2 $TOTAL_STEPS "Installing Docker..."
apt-get install -y ca-certificates curl gnupg lsb-release
mkdir -p /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
apt-get update
apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Start Docker
systemctl start docker
systemctl enable docker

# Install ADB for device connection
apt-get install -y adb

# Step 3: Load kernel modules
update_status "loading_modules" 3 $TOTAL_STEPS "Loading kernel modules..."
apt-get install -y linux-modules-extra-$(uname -r) || true

# Load binder and ashmem modules
modprobe binder_linux devices="binder,hwbinder,vndbinder" || echo "binder_linux not available"
modprobe ashmem_linux || echo "ashmem not available, ReDroid will use memfd"

# Create binder devices
mkdir -p /dev/binderfs
mount -t binder binder /dev/binderfs || echo "binderfs mount failed, continuing..."

# Step 4: Pull ReDroid image
update_status "pulling_image" 4 $TOTAL_STEPS "Pulling ReDroid Docker image..."
docker pull redroid/redroid:14.0.0-latest

# Step 5: Start ReDroid container
update_status "starting_container" 5 $TOTAL_STEPS "Starting ReDroid container..."
docker run -d --name redroid \\
  --privileged \\
  -v /dev/binderfs:/dev/binderfs \\
  -p 5555:5555 \\
  redroid/redroid:14.0.0-latest \\
  androidboot.redroid_gpu_mode=guest \\
  androidboot.redroid_fps=30 \\
  androidboot.redroid_width=1080 \\
  androidboot.redroid_height=1920

# Step 6: Wait for Android boot - OPTIMIZED with parallel downloads
update_status "waiting_boot" 6 $TOTAL_STEPS "Waiting for Android to boot (preparing dependencies in parallel)..."

# OPTIMIZATION: Start background downloads while Android boots
echo "Starting parallel downloads during boot wait..."

# Download cloudflared in background
curl -L --output /usr/local/bin/cloudflared https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64 &
CLOUDFLARED_PID=$!

# Install Node.js in background (nodesource setup + install)
(
  curl -fsSL https://deb.nodesource.com/setup_20.x | bash - > /dev/null 2>&1
  apt-get install -y nodejs > /dev/null 2>&1
  echo "Node.js installed in background"
) &
NODEJS_PID=$!

# Clone custom ws-scrcpy fork in background (with hide-controls and auto-fullscreen)
(
  apt-get install -y build-essential python3 git > /dev/null 2>&1
  cd /opt
  git clone --depth 1 https://github.com/Kahtaf/ws-scrcpy.git > /dev/null 2>&1
  echo "ws-scrcpy (custom fork) cloned in background"
) &
WSSCRCPY_PID=$!

# Download APK in background
(
  mkdir -p /tmp/apps
  curl -L https://github.com/Kahtaf/research/raw/refs/heads/main/redroid/public/apps/org.vana.exporter-0.1.0-5.apk -o /tmp/apps/vana-exporter.apk > /dev/null 2>&1
  echo "APK downloaded in background"
) &
APK_PID=$!

# OPTIMIZATION: Removed sleep 30 - poll immediately with shorter intervals
for i in {1..120}; do
  if docker exec redroid sh -c "getprop sys.boot_completed" 2>/dev/null | grep -q "1"; then
    echo "ReDroid boot completed after $((i * 2)) seconds!"
    break
  fi
  echo "Waiting for ReDroid boot... attempt $i/120"
  sleep 2
done

# Connect ADB to ReDroid
adb connect 127.0.0.1:5555 || echo "ADB connect will be retried"

# Step 7: Build ws-scrcpy (wait for background downloads to complete)
update_status "building_scrcpy" 7 $TOTAL_STEPS "Building ws-scrcpy..."

# Wait for background processes
echo "Waiting for background downloads to complete..."
wait $CLOUDFLARED_PID && echo "cloudflared download complete" || echo "cloudflared download may have issues"
wait $NODEJS_PID && echo "Node.js install complete" || echo "Node.js install may have issues"  
wait $WSSCRCPY_PID && echo "ws-scrcpy clone complete" || echo "ws-scrcpy clone may have issues"
wait $APK_PID && echo "APK download complete" || echo "APK download may have issues"

chmod +x /usr/local/bin/cloudflared

# Build ws-scrcpy (clone already done in background)
echo "Building ws-scrcpy..."
cd /opt/ws-scrcpy
npm install
npm run dist

# Step 8: Start ws-scrcpy
update_status "starting_scrcpy" 8 $TOTAL_STEPS "Starting ws-scrcpy streaming server..."
nohup node dist/index.js --port 8000 --hostname 0.0.0.0 --adb-host 127.0.0.1 --adb-port 5037 > /var/log/ws-scrcpy.log 2>&1 &

# Wait for ws-scrcpy to start
sleep 3

# Step 9: Start Cloudflare Tunnel (already downloaded)
update_status "starting_tunnel" 9 $TOTAL_STEPS "Starting Cloudflare Tunnel for HTTPS..."

# Start cloudflared tunnel in quick mode (free, no account needed)
nohup /usr/local/bin/cloudflared tunnel --url http://localhost:8000 > /var/log/cloudflared.log 2>&1 &

# Wait for cloudflared to start and get the tunnel URL
echo "Waiting for Cloudflare tunnel to initialize..."

# OPTIMIZATION: Shortened initial wait
sleep 5

# Extract the tunnel URL from cloudflared logs
TUNNEL_URL=""
for i in {1..20}; do
  TUNNEL_URL=$(grep -o 'https://[^[:space:]]*\\.trycloudflare\\.com' /var/log/cloudflared.log 2>/dev/null | head -1 || echo "")
  if [ -n "$TUNNEL_URL" ]; then
    echo "Cloudflare tunnel URL: $TUNNEL_URL"
    break
  fi
  echo "Waiting for tunnel URL... attempt $i/20"
  sleep 2
done

if [ -z "$TUNNEL_URL" ]; then
  echo "WARNING: Could not get Cloudflare tunnel URL, falling back to direct IP"
  EXTERNAL_IP=$(curl -s http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip -H "Metadata-Flavor: Google")
  TUNNEL_URL="http://$EXTERNAL_IP:8000"
fi

# Save tunnel URL for API to read
echo "$TUNNEL_URL" > /var/www/html/tunnel_url.txt

# Open firewall ports (for direct access fallback and ADB)
ufw allow 5555/tcp || true
ufw allow 8000/tcp || true

# Step 10: Verify device is connected and stream is ready
update_status "verifying_stream" 10 $TOTAL_STEPS "Verifying Android device is connected..." "$TUNNEL_URL"

# Wait for ws-scrcpy to detect the device
# ws-scrcpy uses ADB to list devices, so we verify ADB connection is stable
echo "Verifying ADB device connection for streaming..."
for i in {1..30}; do
  # Check if ADB sees the device
  ADB_DEVICES=$(adb devices 2>/dev/null | grep -v "List of devices" | grep "device" | head -1)
  if [ -n "$ADB_DEVICES" ]; then
    echo "ADB device connected: $ADB_DEVICES"
    
    # Additional check: verify ws-scrcpy can reach the device
    # ws-scrcpy exposes a device list endpoint we can check
    SCRCPY_CHECK=$(curl -s http://localhost:8000/ 2>/dev/null | head -c 100)
    if [ -n "$SCRCPY_CHECK" ]; then
      echo "ws-scrcpy responding and device connected!"
      break
    fi
  fi
  echo "Waiting for device to be ready for streaming... attempt $i/30"
  sleep 2
done

# Final ADB reconnect to ensure stable connection
adb disconnect 127.0.0.1:5555 2>/dev/null || true
sleep 1
adb connect 127.0.0.1:5555
sleep 2

# Step 11: Install APK
update_status "installing_app" 11 $TOTAL_STEPS "Installing Vana Exporter app..." "$TUNNEL_URL"

# Ensure ADB is connected
adb connect 127.0.0.1:5555
sleep 2

# Install APK if download completed
if [ -f /tmp/apps/vana-exporter.apk ]; then
  echo "Installing Vana Exporter APK..."
  adb -s 127.0.0.1:5555 install -g /tmp/apps/vana-exporter.apk && echo "APK installed successfully" || echo "APK installation failed"
else
  echo "WARNING: APK file not found at /tmp/apps/vana-exporter.apk"
fi

# Step 12: Launch app
update_status "launching_app" 12 $TOTAL_STEPS "Launching Vana Exporter app..." "$TUNNEL_URL"

# Launch the app using am start (more reliable than monkey)
adb -s 127.0.0.1:5555 shell am start -n org.vana.exporter/.MainActivity && echo "App launched" || adb -s 127.0.0.1:5555 shell am start -a android.intent.action.MAIN -n org.vana.exporter/.MainActivity && echo "App launched (fallback)" || echo "App launch failed"

# Small delay to let app initialize
sleep 2

# Step 13: All done - device verified and ready!
TOTAL_TIME=$(($(date +%s) - START_TIME))
update_status "ready" 13 $TOTAL_STEPS "ReDroid is ready with Vana Exporter! (took \${TOTAL_TIME}s)" "$TUNNEL_URL"

# Schedule auto-DELETE after TTL (not just shutdown - we want the VM completely removed)
# Get VM metadata for self-deletion
INSTANCE_NAME=$(curl -s "http://metadata.google.internal/computeMetadata/v1/instance/name" -H "Metadata-Flavor: Google")
ZONE=$(curl -s "http://metadata.google.internal/computeMetadata/v1/instance/zone" -H "Metadata-Flavor: Google" | awk -F'/' '{print $NF}')
PROJECT=$(curl -s "http://metadata.google.internal/computeMetadata/v1/project/project-id" -H "Metadata-Flavor: Google")

echo "VM will self-delete in ${ttlMinutes} minutes: $INSTANCE_NAME in $ZONE"

# Use 'at' scheduler or fallback to sleep
(
  sleep $((${ttlMinutes} * 60))
  echo "TTL expired - deleting VM $INSTANCE_NAME"
  gcloud compute instances delete "$INSTANCE_NAME" --zone="$ZONE" --project="$PROJECT" --quiet
) &

echo "=== ReDroid setup complete at \$(date) (took \${TOTAL_TIME}s) ==="
echo "Access Android via: $TUNNEL_URL"
echo "ADB connect via: adb connect <VM_IP>:5555"
echo "Direct ws-scrcpy (fallback): http://<VM_IP>:8000"

# List running containers for debug
docker ps
`;
}
