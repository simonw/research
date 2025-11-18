#!/bin/bash
set -e

VM_NAME="android-mitm-mvp"
ZONE="us-central1-a"
PROJECT="corsali-development"

echo "=========================================="
echo "Deploying Waydroid MITM to GCP"
echo "=========================================="

# Step 1: Transfer waydroid-mitm files to VM
echo "[1/6] Transferring waydroid-mitm files to VM..."
gcloud compute scp --recurse --zone=${ZONE} --project=${PROJECT} \
    ../native-app-traffic-capture/waydroid-mitm ${VM_NAME}:~/

# Step 2: Transfer YouTube APK
echo "[2/6] Transferring YouTube APK..."
gcloud compute scp --zone=${ZONE} --project=${PROJECT} \
    youtube.apk ${VM_NAME}:~/waydroid-mitm/apks/

# Step 3: Run setup scripts (as root)
echo "[3/6] Running VM setup (this may take 30-45 minutes)..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- \
    "cd ~/waydroid-mitm && sudo bash scripts/01-setup-vm.sh"

echo "[4/6] Initializing Waydroid (this may take 15-20 minutes)..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- \
    "cd ~/waydroid-mitm && sudo bash scripts/02-init-waydroid.sh"

echo "[5/6] Setting up MITM proxy..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- \
    "cd ~/waydroid-mitm && sudo bash scripts/03-setup-mitm.sh"

echo "[6/6] Installing certificates..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- \
    "cd ~/waydroid-mitm && sudo bash scripts/04-install-certs.sh"

echo ""
echo "=========================================="
echo "Deployment Complete!"
echo "=========================================="
echo ""
echo "Next steps:"
echo "1. Setup headless VNC: sudo bash scripts/05-setup-headless.sh"
echo "2. Install Frida: sudo bash scripts/06-install-frida.sh"
echo "3. Connect via VNC: vnc://34.42.16.156:5900"
echo "4. Access mitmproxy: http://34.42.16.156:8081"
echo ""

