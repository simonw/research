#!/bin/bash
set -e

if [ -f .env ]; then
    source .env
else
    echo "⚠️  Warning: .env file not found. Cloudflare Tunnel and Proxy will not be configured."
fi

PROJECT_ID="corsali-development"
ZONE="us-central1-a"
VM_NAME="docker-chrome-vm"
IMAGE_NAME="gcr.io/${PROJECT_ID}/docker-chrome"
MACHINE_TYPE="e2-medium"
FIREWALL_RULE="allow-docker-chrome-8080"
HTTPS_URL="https://docker-chrome.vana.com"

echo "🚀 Deploying Docker Chrome to VM in ${PROJECT_ID}..."

echo "📦 Building container..."
gcloud builds submit --tag "${IMAGE_NAME}" . --project "${PROJECT_ID}"

CHROME_ARGS="--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1 --remote-allow-origins=* --no-first-run --start-fullscreen --start-maximized --block-new-web-contents --disable-infobars --disable-extensions --disable-dev-tools --allow-running-insecure-content"

# Proxy configuration - residential proxy disabled for now on the VM because it gets flagged as a bot less often than Cloud Run
#if [ -n "${PROXY_SERVER}" ]; then
#    CHROME_ARGS="${CHROME_ARGS} --proxy-server=${PROXY_SERVER}"
#    echo "🔒 Proxy enabled: ${PROXY_SERVER}"
#else
#    echo "🌐 Proxy disabled (set PROXY_SERVER in .env to enable)"
#fi

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

# 6. Set up Named Cloudflare Tunnel for HTTPS
if [ -n "$CLOUDFLARE_TUNNEL_TOKEN" ]; then
    echo "🔒 Starting Cloudflare Tunnel..."
    
    gcloud compute ssh "${VM_NAME}" \
        --zone="${ZONE}" \
        --project="${PROJECT_ID}" \
        --command="
        docker rm -f cloudflared 2>/dev/null || true
        
        docker run -d \
            --name cloudflared \
            --network host \
            --restart unless-stopped \
            cloudflare/cloudflared:latest \
            tunnel run --token ${CLOUDFLARE_TUNNEL_TOKEN}
        
        sleep 3
        if docker ps | grep -q cloudflared; then
            echo '✅ Cloudflared running'
        else
            echo '❌ Cloudflared failed to start'
            docker logs cloudflared
            exit 1
        fi
    "
    
    echo "✅ Tunnel connected to ${HTTPS_URL}"
else
    echo "⚠️  Cloudflare Tunnel not configured (no CLOUDFLARE_TUNNEL_TOKEN in .env)"
    HTTPS_URL="http://${EXTERNAL_IP}:8080"
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
echo "   HTTPS: ${HTTPS_URL}"
echo "   API:   ${HTTPS_URL}/api/status"
echo ""
echo "🔧 Control Pane:"
if [ -n "$CLOUDFLARE_TUNNEL_TOKEN" ]; then
    echo "   ?target=vm&ip=docker-chrome.vana.com"
else
    echo "   ?target=vm&ip=${EXTERNAL_IP}:8080"
fi
echo ""
echo "📋 Useful Commands:"
echo "   View logs:      gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT_ID} --command='docker logs \$(docker ps -q --filter ancestor=${IMAGE_NAME}) -f'"
if [ -n "$CLOUDFLARE_TUNNEL_TOKEN" ]; then
    echo "   Tunnel logs:    gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT_ID} --command='docker logs cloudflared'"
fi
echo "   SSH:            gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT_ID}"
echo "   Delete VM:      gcloud compute instances delete ${VM_NAME} --zone=${ZONE} --project=${PROJECT_ID}"

