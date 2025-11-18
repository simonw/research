#!/bin/bash

set -euo pipefail

# ---------------------------------------------------------------
# Waydroid Setup Script for GCP Ubuntu 22.04 LTS
# Run this AFTER:
#   gcloud compute instances create waydroid-mitm \
#     --image-family=ubuntu-2204-lts \
#     --image-project=ubuntu-os-cloud \
#     --machine-type=n2-standard-4 \
#     --zone=us-central1-a \
#     --boot-disk-size=50GB \
#     --project=corsali-development
# ---------------------------------------------------------------

log() { echo "[$(date -Iseconds)] $*" >&2; }

log "=== Starting Waydroid setup ==="

# 1. Ensure we're on a generic HWE kernel (not -gcp)
log "1. Installing/updating HWE generic kernel..."

sudo apt update
sudo apt install -y --no-install-recommends \
    linux-generic-hwe-22.04 \
    linux-headers-generic-hwe-22.04

# 2. Remove GCP kernel if present (safe only if not running it)
if [[ "$(uname -r)" == *"-gcp" ]]; then
    log "⚠️  Currently running GCP kernel. Setting GRUB to boot generic next time..."
    
    # Make GRUB menu visible once
    sudo sed -i 's/^GRUB_TIMEOUT=.*/GRUB_TIMEOUT=10/' /etc/default/grub
    sudo update-grub
    
    # Use grub-reboot for one-time boot into generic
    GENERIC_ENTRY=$(grep -P "^menuentry.*6\.8\..*-generic'" /boot/grub/grub.cfg | head -1 | sed -E "s/.*'([^']+)'.*/\1/")
    if [[ -n "$GENERIC_ENTRY" ]]; then
        log "Found generic entry: $GENERIC_ENTRY"
        sudo grub-reboot "$GENERIC_ENTRY"
    else
        log "⚠️  Could not auto-detect generic entry — please manually select at reboot"
    fi
    
    log "Rebooting to generic kernel (required before proceeding)..."
    echo "After reboot, re-run this script to continue."
    sudo reboot
    exit 0
fi

# 3. Verify we're now on generic kernel
if ! uname -r | grep -q "generic"; then
    log "❌ Not on a generic kernel. Reboot manually into -generic and re-run."
    exit 1
fi

log "✅ Running on kernel: $(uname -r)"

# 4. Load and persist binder (ashmem not needed on modern kernels)
log "2. Loading and enabling binder modules..."

sudo modprobe binder_linux devices="binder,hwbinder,vndbinder"

# Verify devices exist (ashmem not required on kernel 6.8+)
for dev in binder hwbinder vndbinder; do
    if [[ ! -e "/dev/$dev" ]]; then
        log "❌ /dev/$dev missing — kernel likely lacks CONFIG_ANDROID_*"
        exit 1
    fi
done

# Persist across reboots
echo "binder_linux" | sudo tee /etc/modules-load.d/binder.conf >/dev/null

# 5. Install Waydroid
log "3. Installing Waydroid..."

sudo apt install -y curl ca-certificates
curl -s https://repo.waydro.id | sudo bash
sudo apt update
sudo apt install -y waydroid

# 6. Initialize (downloads ~1.5GB image)
log "4. Initializing Waydroid (this may take 5–10 mins)..."

sudo waydroid init

# 7. Start Waydroid services
log "5. Starting Waydroid session & container..."

sudo systemctl enable --now waydroid-container
waydroid session start &>/dev/null &
sleep 3

# Optional: Show status
log "6. Verifying status..."

if waydroid status | grep -q "Session:\s*RUNNING"; then
    log "✅ Waydroid is RUNNING"
else
    log "⚠️  Waydroid not fully up — try: 'waydroid show-full-ui' or check 'waydroid log'"
fi

log "=== Setup complete! ==="
log "Next steps:"
log "  - Run 'waydroid show-full-ui' (requires X11/Wayland forwarding or local display)"
log "  - Or use ADB: 'adb connect 127.0.0.1:5555'"
