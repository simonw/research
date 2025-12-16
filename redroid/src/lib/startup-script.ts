/**
 * Generates the VM startup script that:
 * 1. Installs Docker
 * 2. Loads kernel modules (binder_linux, ashmem)
 * 3. Sets up ReDroid (Android 13)
 * 4. Builds and runs ws-scrcpy from source for browser access
 * 5. Starts Cloudflare Tunnel for valid HTTPS without firewall changes
 * 6. Schedules auto-shutdown after TTL
 * 7. Reports progress via /status.json for frontend polling
 */
export function generateStartupScript(ttlMinutes: number): string {
  return `#!/bin/bash
set -e

# Log everything for debugging
exec > >(tee /var/log/startup-script.log) 2>&1
echo "Starting ReDroid setup at $(date)"

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
  
  cat > /var/www/html/status.json << EOF
{
  "stage": "$stage",
  "step": $step,
  "totalSteps": $total_steps,
  "message": "$message",
  "percent": $percent,
  "timestamp": $timestamp,
  "tunnelUrl": "$tunnel_url"
}
EOF
  echo "[PROGRESS] Step $step/$total_steps ($percent%): $message"
}

TOTAL_STEPS=10

# Start a simple HTTP server to serve status.json for progress polling
# This runs in the background on port 8080
cd /var/www/html
nohup python3 -m http.server 8080 --bind 0.0.0.0 > /var/log/status-server.log 2>&1 &
cd /

# Open firewall for status server
ufw allow 8080/tcp || true

# Step 1: Update system
update_status "updating_system" 1 $TOTAL_STEPS "Updating system packages..."
apt-get update
apt-get upgrade -y

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
docker pull redroid/redroid:13.0.0-latest

# Step 5: Start ReDroid container
update_status "starting_container" 5 $TOTAL_STEPS "Starting ReDroid container..."
docker run -d --name redroid \\
  --privileged \\
  -v /dev/binderfs:/dev/binderfs \\
  -p 5555:5555 \\
  redroid/redroid:13.0.0-latest \\
  androidboot.redroid_gpu_mode=guest \\
  androidboot.redroid_fps=30 \\
  androidboot.redroid_width=1080 \\
  androidboot.redroid_height=1920

# Step 6: Wait for Android boot
update_status "waiting_boot" 6 $TOTAL_STEPS "Waiting for Android to boot..."
sleep 30

# Wait for ReDroid boot to complete
for i in {1..60}; do
  if docker exec redroid sh -c "getprop sys.boot_completed" 2>/dev/null | grep -q "1"; then
    echo "ReDroid boot completed!"
    break
  fi
  echo "Waiting for ReDroid boot... attempt $i/60"
  sleep 5
done

# Connect ADB to ReDroid
adb connect 127.0.0.1:5555 || echo "ADB connect will be retried"

# Step 7: Install build tools
update_status "building_scrcpy" 7 $TOTAL_STEPS "Installing build tools for ws-scrcpy..."
apt-get install -y build-essential python3 git

# Install Node.js for ws-scrcpy build
echo "Installing Node.js..."
curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
apt-get install -y nodejs

# Build ws-scrcpy from source
echo "Building ws-scrcpy from source..."
cd /opt
git clone https://github.com/NetrisTV/ws-scrcpy.git
cd ws-scrcpy
npm install
npm run dist

# Step 8: Start ws-scrcpy
update_status "starting_scrcpy" 8 $TOTAL_STEPS "Starting ws-scrcpy streaming server..."
nohup node dist/index.js --port 8000 --hostname 0.0.0.0 --adb-host 127.0.0.1 --adb-port 5037 > /var/log/ws-scrcpy.log 2>&1 &

# Wait for ws-scrcpy to start
sleep 5

# Step 9: Install and start Cloudflare Tunnel
update_status "starting_tunnel" 9 $TOTAL_STEPS "Starting Cloudflare Tunnel for HTTPS..."

# Download and install cloudflared
curl -L --output /usr/local/bin/cloudflared https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64
chmod +x /usr/local/bin/cloudflared

# Start cloudflared tunnel in quick mode (free, no account needed)
# This creates a temporary URL like https://random-name.trycloudflare.com
# The --url flag tells it to proxy to local ws-scrcpy
nohup /usr/local/bin/cloudflared tunnel --url http://localhost:8000 > /var/log/cloudflared.log 2>&1 &

# Wait for cloudflared to start and get the tunnel URL
echo "Waiting for Cloudflare tunnel to initialize..."
sleep 10

# Extract the tunnel URL from cloudflared logs
TUNNEL_URL=""
for i in {1..30}; do
  TUNNEL_URL=$(grep -o 'https://[^[:space:]]*\\.trycloudflare\\.com' /var/log/cloudflared.log 2>/dev/null | head -1 || echo "")
  if [ -n "$TUNNEL_URL" ]; then
    echo "Cloudflare tunnel URL: $TUNNEL_URL"
    break
  fi
  echo "Waiting for tunnel URL... attempt $i/30"
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

# Step 10: All done!
update_status "ready" 10 $TOTAL_STEPS "ReDroid is ready!" "$TUNNEL_URL"

# Schedule auto-shutdown after TTL
echo "shutdown -h now" | at now + ${ttlMinutes} minutes || (
  # Fallback if 'at' is not available
  (sleep ${ttlMinutes * 60} && shutdown -h now) &
)

echo "=== ReDroid setup complete at $(date) ==="
echo "Access Android via: $TUNNEL_URL"
echo "ADB connect via: adb connect <VM_IP>:5555"
echo "Direct ws-scrcpy (fallback): http://<VM_IP>:8000"

# List running containers for debug
docker ps
`;
}
