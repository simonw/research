#!/bin/bash
# E2E test for Android MITM MVP

VM_IP="34.42.16.156"

# Check noVNC
curl -f http://${VM_IP}:6080 || echo "noVNC not accessible"

# Check mitmproxy
curl -f http://${VM_IP}:8081 || echo "mitmproxy not accessible"

# Further tests would require password for mitmproxy and VNC interaction

echo "Manual verification required for traffic capture."
