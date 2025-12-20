#!/bin/bash
set -e

# Parse command line arguments
USE_PROXY=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --proxy)
            USE_PROXY=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--proxy]"
            exit 1
            ;;
    esac
done

PROJECT_ID="corsali-development"
ZONE="us-central1-a"
VM_NAME="docker-chrome-vm"
IMAGE_NAME="gcr.io/${PROJECT_ID}/docker-chrome"
MACHINE_TYPE="e2-medium"
FIREWALL_RULE="allow-docker-chrome-8080"

echo "🚀 Deploying Docker Chrome to VM in ${PROJECT_ID}..."

# 1. Build and push image (same as Cloud Run)
echo "📦 Building container..."
gcloud builds submit --tag "${IMAGE_NAME}" . --project "${PROJECT_ID}"

# Build Chrome CLI args
CHROME_ARGS="--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1 --remote-allow-origins=* --no-first-run --start-fullscreen --start-maximized --block-new-web-contents --disable-infobars --disable-extensions --disable-dev-tools --allow-running-insecure-content"

# Proxy configuration (only if --proxy flag is passed)
if [ "$USE_PROXY" = true ]; then
    PROXY_SERVER="${PROXY_SERVER:-brd.superproxy.io:33335}"
    PROXY_USERNAME="${PROXY_USERNAME:-brd-customer-hl_b6c4198e-zone-residential1}"
    PROXY_PASSWORD="${PROXY_PASSWORD:-}"  # Set via environment variable
    CHROME_ARGS="${CHROME_ARGS} --proxy-server=${PROXY_SERVER}"
    echo "🔒 Proxy enabled: ${PROXY_SERVER}"
else
    PROXY_SERVER=""
    PROXY_USERNAME=""
    PROXY_PASSWORD=""
    echo "🌐 Proxy disabled (use --proxy flag to enable)"
fi

CHROME_ARGS="${CHROME_ARGS} about:blank"

# 2. Check if firewall rule exists
echo "🔥 Checking firewall rules..."
if ! gcloud compute firewall-rules describe "${FIREWALL_RULE}" --project "${PROJECT_ID}" &>/dev/null; then
    echo "Creating firewall rule ${FIREWALL_RULE}..."
    gcloud compute firewall-rules create "${FIREWALL_RULE}" \
        --project "${PROJECT_ID}" \
        --direction=INGRESS \
        --priority=1000 \
        --network=default \
        --action=ALLOW \
        --rules=tcp:8080 \
        --source-ranges=0.0.0.0/0 \
        --target-tags=docker-chrome
else
    echo "Firewall rule ${FIREWALL_RULE} already exists"
fi

# 3. Check if VM exists
echo "🖥️ Checking if VM exists..."
if gcloud compute instances describe "${VM_NAME}" --zone="${ZONE}" --project="${PROJECT_ID}" &>/dev/null; then
    echo "VM ${VM_NAME} exists. Updating container..."
    
    # Update the container on existing VM
    gcloud compute instances update-container "${VM_NAME}" \
        --zone="${ZONE}" \
        --project="${PROJECT_ID}" \
        --container-image="${IMAGE_NAME}" \
        --container-env="CHROME_CLI=${CHROME_ARGS}" \
        --container-env="CDP_PORT=9222" \
        --container-env="NO_DECOR=true" \
        --container-env="HARDEN_DESKTOP=true" \
        --container-env="DISABLE_MOUSE_BUTTONS=true" \
        --container-env="HARDEN_KEYBINDS=true" \
        --container-env="SELKIES_UI_SHOW_SIDEBAR=false" \
        --container-env="HARDEN_OPENBOX=true" \
        --container-env="SELKIES_MANUAL_WIDTH=430" \
        --container-env="SELKIES_MANUAL_HEIGHT=932" \
        --container-env="PROXY_SERVER=${PROXY_SERVER}" \
        --container-env="PROXY_USERNAME=${PROXY_USERNAME}" \
        --container-env="PROXY_PASSWORD=${PROXY_PASSWORD}"
    
    echo "✅ Container updated. Waiting for restart..."
    sleep 5
else
    echo "VM ${VM_NAME} does not exist. Creating..."
    
    # Create VM with container
    gcloud compute instances create-with-container "${VM_NAME}" \
        --zone="${ZONE}" \
        --project="${PROJECT_ID}" \
        --machine-type="${MACHINE_TYPE}" \
        --tags=docker-chrome \
        --container-image="${IMAGE_NAME}" \
        --container-env="CHROME_CLI=${CHROME_ARGS}" \
        --container-env="CDP_PORT=9222" \
        --container-env="NO_DECOR=true" \
        --container-env="HARDEN_DESKTOP=true" \
        --container-env="DISABLE_MOUSE_BUTTONS=true" \
        --container-env="HARDEN_KEYBINDS=true" \
        --container-env="SELKIES_UI_SHOW_SIDEBAR=false" \
        --container-env="HARDEN_OPENBOX=true" \
        --container-env="SELKIES_MANUAL_WIDTH=430" \
        --container-env="SELKIES_MANUAL_HEIGHT=932" \
        --container-env="PROXY_SERVER=${PROXY_SERVER}" \
        --container-env="PROXY_USERNAME=${PROXY_USERNAME}" \
        --container-env="PROXY_PASSWORD=${PROXY_PASSWORD}"
    
    echo "✅ VM created. Waiting for container to start..."
    sleep 15
fi

# 4. Get external IP
echo "📍 Getting external IP..."
EXTERNAL_IP=$(gcloud compute instances describe "${VM_NAME}" \
    --zone="${ZONE}" \
    --project="${PROJECT_ID}" \
    --format='get(networkInterfaces[0].accessConfigs[0].natIP)')

# 5. Wait for container to be ready
echo "⏳ Waiting for container to be ready..."
for i in {1..30}; do
    if curl -s --connect-timeout 2 "http://${EXTERNAL_IP}:8080/healthz" | grep -q '"ok":true'; then
        echo "✅ Container is ready!"
        break
    fi
    echo "   Waiting... ($i/30)"
    sleep 3
done

# 6. Set up Cloudflare Tunnel for HTTPS
echo "🔒 Setting up Cloudflare Tunnel for HTTPS..."

# SSH into VM and set up cloudflared using Docker (COS has read-only filesystem)
TUNNEL_URL=$(gcloud compute ssh "${VM_NAME}" \
    --zone="${ZONE}" \
    --project="${PROJECT_ID}" \
    --command='
    # Check if cloudflared container is already running with a tunnel
    if docker ps --filter name=cloudflared --format "{{.Names}}" | grep -q cloudflared; then
        echo "cloudflared container already running" >&2
        # Get existing tunnel URL from container logs
        EXISTING_URL=$(docker logs cloudflared 2>&1 | grep -o "https://[^[:space:]]*\.trycloudflare\.com" | tail -1)
        if [ -n "$EXISTING_URL" ]; then
            echo "$EXISTING_URL"
            exit 0
        fi
    fi
    
    # Remove any stopped cloudflared container
    docker rm -f cloudflared 2>/dev/null || true
    
    # Start cloudflared tunnel using Docker container
    echo "Starting cloudflared container..." >&2
    docker run -d --name cloudflared --network host \
        cloudflare/cloudflared:latest tunnel --url http://localhost:8080
    
    # Wait for tunnel URL
    for i in $(seq 1 30); do
        TUNNEL_URL=$(docker logs cloudflared 2>&1 | grep -o "https://[^[:space:]]*\.trycloudflare\.com" | head -1)
        if [ -n "$TUNNEL_URL" ]; then
            echo "$TUNNEL_URL"
            exit 0
        fi
        sleep 1
    done
    
    echo "Failed to get tunnel URL" >&2
    docker logs cloudflared >&2
    exit 1
' 2>&1)

# Extract the URL from output (last https URL)
HTTPS_URL=$(echo "$TUNNEL_URL" | grep -o 'https://[^[:space:]]*\.trycloudflare\.com' | tail -1)

if [ -z "$HTTPS_URL" ]; then
    echo "⚠️ Could not get Cloudflare Tunnel URL"
    echo "   Debug output: $TUNNEL_URL"
    HTTPS_URL="(tunnel setup failed - check VM logs)"
fi

echo ""
echo "✅ Deployment complete!"
echo ""
echo "📌 VM Details:"
echo "   Name: ${VM_NAME}"
echo "   Zone: ${ZONE}"
echo "   IP:   ${EXTERNAL_IP}"
echo ""
echo "🌐 Access URLs:"
echo "   HTTPS (Cloudflare): ${HTTPS_URL}"
echo "   HTTP (direct):      http://${EXTERNAL_IP}:8080"
echo "   API Status:         ${HTTPS_URL}/api/status"
echo ""
echo "🔧 Control Pane (use HTTPS URL for streaming):"
echo "   ?target=vm&ip=${HTTPS_URL#https://}"
echo ""
echo "📋 Useful Commands:"
echo "   View logs:      gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT_ID} --command='sudo docker logs \$(docker ps -q) -f'"
echo "   Tunnel logs:    gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT_ID} --command='docker logs cloudflared'"
echo "   SSH:            gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT_ID}"
echo "   Delete VM:      gcloud compute instances delete ${VM_NAME} --zone=${ZONE} --project=${PROJECT_ID}"

