#!/bin/bash
set -e

VM_NAME="android-mitm-mvp"
ZONE="us-central1-a"
PROJECT="corsali-development"

echo "=========================================="
echo "Deploying Dockerify-Android MITM to GCP"
echo "=========================================="

# Step 1: Stop existing container
echo "[1/5] Stopping existing budtmo container..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- \
    "docker stop android-mitm-mvp 2>/dev/null || true"

# Step 2: Transfer files to VM
echo "[2/5] Transferring files to VM..."
gcloud compute scp --recurse --zone=${ZONE} --project=${PROJECT} \
    ../dockerify-android-mitm ${VM_NAME}:~/

# Step 3: Build Docker image on VM
echo "[3/5] Building Docker image on VM (this may take 10-15 minutes)..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- \
    "cd ~/dockerify-android-mitm && (docker compose build || docker-compose build)"

# Step 4: Start new container
echo "[4/5] Starting dockerify-android-mitm container..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- \
    "cd ~/dockerify-android-mitm && (docker compose down --remove-orphans 2>/dev/null || docker-compose down --remove-orphans 2>/dev/null || true) && (docker compose up -d || docker-compose up -d)"

# Step 5: Wait and show logs
echo "[5/5] Waiting for services to start..."
sleep 30

echo ""
echo "Tailing logs (Ctrl+C to exit)..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- \
    "docker logs -f dockerify-android-mitm"
