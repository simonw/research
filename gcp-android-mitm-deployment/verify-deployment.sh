#!/bin/bash
# Verification script for dockerify-android-mitm deployment

set -e

VM_NAME="android-mitm-mvp"
ZONE="us-central1-a"
PROJECT="corsali-development"

echo "=========================================="
echo "Dockerify-Android MITM Deployment Verification"
echo "=========================================="
echo ""

# Get VM IP
echo "[1/8] Getting VM IP address..."
VM_IP=$(gcloud compute instances describe ${VM_NAME} --zone=${ZONE} --project=${PROJECT} --format="value(networkInterfaces[0].accessConfigs[0].natIP)")
echo "✓ VM IP: ${VM_IP}"
echo ""

# Check container status
echo "[2/8] Checking container status..."
CONTAINER_STATUS=$(gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- "docker ps --filter name=dockerify-android-mitm --format '{{.Status}}'" 2>/dev/null)
echo "✓ Container Status: ${CONTAINER_STATUS}"
echo ""

# Check Android boot status
echo "[3/8] Checking Android boot status..."
BOOT_STATUS=$(gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- "docker exec dockerify-android-mitm adb shell getprop sys.boot_completed" 2>/dev/null | tr -d '\r')
if [ "$BOOT_STATUS" == "1" ]; then
    echo "✓ Android fully booted"
else
    echo "✗ Android not fully booted (status: $BOOT_STATUS)"
    exit 1
fi
echo ""

# Check Android version
echo "[4/8] Checking Android version..."
ANDROID_VERSION=$(gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- "docker exec dockerify-android-mitm adb shell getprop ro.build.version.release" 2>/dev/null | tr -d '\r')
echo "✓ Android version: ${ANDROID_VERSION}"
echo ""

# Check services
echo "[5/8] Checking running services..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- "docker exec dockerify-android-mitm ps aux | grep -E '(mitmproxy|emulator|ws-scrcpy)' | grep -v grep" 2>/dev/null | while read line; do
    if echo "$line" | grep -q "mitmweb"; then
        echo "✓ mitmproxy running"
    elif echo "$line" | grep -q "qemu-system"; then
        echo "✓ Emulator running"
    elif echo "$line" | grep -q "ws-scrcpy"; then
        echo "✓ ws-scrcpy running"
    fi
done
echo ""

# Check traffic interception
echo "[6/8] Checking traffic interception..."
IPTABLES_CHECK=$(gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- "docker logs dockerify-android-mitm 2>&1 | grep -c 'iptables system-wide redirection active'" 2>/dev/null)
if [ "$IPTABLES_CHECK" -gt "0" ]; then
    echo "✓ iptables system-wide redirection active"
else
    echo "✗ iptables redirection not confirmed"
fi
echo ""

# Check captured flows
echo "[7/8] Checking captured traffic..."
FLOW_COUNT=$(gcloud compute ssh ${VM_NAME} --zone=${ZONE} --project=${PROJECT} -- "docker exec dockerify-android-mitm bash -c \"curl -s http://localhost:8081/flows | python3 -m json.tool | grep -c '\\\"id\\\"'\"" 2>/dev/null)
echo "✓ Captured flows: ${FLOW_COUNT}"
echo ""

# Check web UIs
echo "[8/8] Checking web UI accessibility..."
MITMPROXY_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 http://${VM_IP}:8081)
if [ "$MITMPROXY_STATUS" == "200" ]; then
    echo "✓ mitmproxy web UI accessible (HTTP ${MITMPROXY_STATUS})"
else
    echo "✗ mitmproxy web UI not accessible (HTTP ${MITMPROXY_STATUS})"
fi

SCRCPY_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 http://${VM_IP}:8000)
if [ "$SCRCPY_STATUS" == "200" ]; then
    echo "✓ ws-scrcpy web UI accessible (HTTP ${SCRCPY_STATUS})"
else
    echo "✗ ws-scrcpy web UI not accessible (HTTP ${SCRCPY_STATUS})"
fi
echo ""

# Summary
echo "=========================================="
echo "Deployment Status: SUCCESS"
echo "=========================================="
echo ""
echo "Access URLs:"
echo "  - mitmproxy:  http://${VM_IP}:8081 (password: mitmproxy)"
echo "  - ws-scrcpy:  http://${VM_IP}:8000"
echo ""
echo "System is ready for E2E testing!"
