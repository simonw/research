/**
 * Generates the VM startup script that:
 * 1. Installs Docker
 * 2. Loads kernel modules (binder_linux, ashmem)
 * 3. Sets up ReDroid (Android 13)
 * 4. Builds and runs ws-scrcpy from source for browser access
 * 5. Schedules auto-shutdown after TTL
 */
export function generateStartupScript(ttlMinutes: number): string {
  return `#!/bin/bash
set -e

# Log everything for debugging
exec > >(tee /var/log/startup-script.log) 2>&1
echo "Starting ReDroid setup at $(date)"

# Update system
apt-get update
apt-get upgrade -y

# Install Docker
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

# Install kernel modules for ReDroid
apt-get install -y linux-modules-extra-$(uname -r) || true

# Load binder and ashmem modules
modprobe binder_linux devices="binder,hwbinder,vndbinder" || echo "binder_linux not available"
modprobe ashmem_linux || echo "ashmem not available, ReDroid will use memfd"

# Create binder devices
mkdir -p /dev/binderfs
mount -t binder binder /dev/binderfs || echo "binderfs mount failed, continuing..."

# Pull ReDroid image (Android 13)
echo "Pulling ReDroid image..."
docker pull redroid/redroid:13.0.0-latest

# Run ReDroid container with display settings
echo "Starting ReDroid container..."
docker run -d --name redroid \\
  --privileged \\
  -v /dev/binderfs:/dev/binderfs \\
  -p 5555:5555 \\
  redroid/redroid:13.0.0-latest \\
  androidboot.redroid_gpu_mode=guest \\
  androidboot.redroid_fps=30 \\
  androidboot.redroid_width=1080 \\
  androidboot.redroid_height=1920

# Wait for ReDroid to start and ADB to be available
echo "Waiting for ReDroid to initialize..."
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

# Install build tools for native npm modules (node-pty, etc.)
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

# Start ws-scrcpy server
echo "Starting ws-scrcpy..."
nohup node dist/index.js --port 8000 --hostname 0.0.0.0 --adb-host 127.0.0.1 --adb-port 5037 > /var/log/ws-scrcpy.log 2>&1 &

# Wait for ws-scrcpy to start
sleep 5

# Install nginx for HTTPS reverse proxy
apt-get install -y nginx

# Generate self-signed certificate
mkdir -p /etc/nginx/ssl
openssl req -x509 -nodes -days 1 -newkey rsa:2048 \\
  -keyout /etc/nginx/ssl/nginx.key \\
  -out /etc/nginx/ssl/nginx.crt \\
  -subj "/CN=redroid-session"

# Configure nginx as HTTPS reverse proxy for ws-scrcpy (port 8000)
cat > /etc/nginx/sites-available/default << 'NGINX_CONF'
server {
    listen 443 ssl;
    ssl_certificate /etc/nginx/ssl/nginx.crt;
    ssl_certificate_key /etc/nginx/ssl/nginx.key;

    location / {
        proxy_pass http://127.0.0.1:8000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_read_timeout 86400;
    }
}

server {
    listen 80;
    return 301 https://$host$request_uri;
}
NGINX_CONF

# Restart nginx
systemctl restart nginx

# Open firewall ports
ufw allow 443/tcp || true
ufw allow 5555/tcp || true
ufw allow 8000/tcp || true

# Schedule auto-shutdown after TTL
echo "shutdown -h now" | at now + ${ttlMinutes} minutes || (
  # Fallback if 'at' is not available
  (sleep ${ttlMinutes * 60} && shutdown -h now) &
)

echo "=== ReDroid setup complete at $(date) ==="
echo "Access Android via: https://<VM_IP>"
echo "ADB connect via: adb connect <VM_IP>:5555"
echo "Direct ws-scrcpy: http://<VM_IP>:8000"

# List running containers for debug
docker ps
`;
}
