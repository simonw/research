#!/usr/bin/env bash
# Provision a GCE VM, deploy the Android MITM MVP, and expose noVNC/mitmproxy ports.
set -euo pipefail

PROJECT="${PROJECT:-corsali-development}"
ZONE="${ZONE:-us-central1-a}"
INSTANCE_NAME="${INSTANCE_NAME:-android-mitm-mvp}"  # change if multiple instances needed
MACHINE_TYPE="${MACHINE_TYPE:-n2-standard-4}"        # More CPU/RAM for smoother emulator
BOOT_DISK_SIZE="${BOOT_DISK_SIZE:-100GB}"
NETWORK_TAG="${NETWORK_TAG:-android-mitm-mvp}"
FIREWALL_RULE="${FIREWALL_RULE:-android-mitm-mvp-ports}"
IMAGE_FAMILY="${IMAGE_FAMILY:-debian-12}"
IMAGE_PROJECT="${IMAGE_PROJECT:-debian-cloud}"
MIN_CPU_PLATFORM="${MIN_CPU_PLATFORM:-Intel Cascade Lake}"
SSH_KEY_FILE="${SSH_KEY_FILE:-$HOME/.ssh/android-mitm-mvp}"
SSH_USER="${SSH_USER:-$(whoami)}"
APP_PACKAGE="${APP_PACKAGE:-com.android.chrome}"
EMULATOR_ADDITIONAL_ARGS="${EMULATOR_ADDITIONAL_ARGS:--cores 4 -memory 8192 -no-snapshot -no-boot-anim -noaudio -gpu swiftshader_indirect}"
EMULATOR_DATA_PARTITION="${EMULATOR_DATA_PARTITION:-2048m}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_BASENAME="$(basename "${REPO_ROOT}")"
ARCHIVE="$(mktemp -t android-mitm-mvp.XXXXXX)"
ARCHIVE="${ARCHIVE}.tar.gz"
SSH_METADATA_FILE="$(mktemp -t android-mitm-mvp-ssh.XXXXXX)"

echo "=== Building project archive ==="
tar -czf "${ARCHIVE}" -C "$(dirname "${REPO_ROOT}")" "${REPO_BASENAME}"

trap 'rm -f "${ARCHIVE}" "${SSH_METADATA_FILE}"' EXIT

if [[ ! -f "${SSH_KEY_FILE}" ]]; then
  echo "=== Generating SSH key at ${SSH_KEY_FILE} ==="
  ssh-keygen -t rsa -f "${SSH_KEY_FILE}" -N "" -C "${SSH_USER}@android-mitm-mvp"
fi

printf '%s:%s\n' "${SSH_USER}" "$(cat "${SSH_KEY_FILE}.pub")" > "${SSH_METADATA_FILE}"

echo "=== Setting gcloud project to ${PROJECT} ==="
gcloud config set project "${PROJECT}" >/dev/null

echo "=== Ensuring firewall rule ${FIREWALL_RULE} exists ==="
if ! gcloud compute firewall-rules describe "${FIREWALL_RULE}" --project "${PROJECT}" >/dev/null 2>&1; then
  gcloud compute firewall-rules create "${FIREWALL_RULE}" \
    --project "${PROJECT}" \
    --allow tcp:6080,tcp:8081 \
    --target-tags "${NETWORK_TAG}" \
    --description "Expose noVNC (6080) and mitmproxy (8081) for Android MITM MVP"
fi

echo "=== Creating VM ${INSTANCE_NAME} (${MACHINE_TYPE}) in ${ZONE} ==="
if ! gcloud compute instances describe "${INSTANCE_NAME}" --zone "${ZONE}" --project "${PROJECT}" >/dev/null 2>&1; then
  gcloud compute instances create "${INSTANCE_NAME}" \
    --project "${PROJECT}" \
    --zone "${ZONE}" \
    --machine-type "${MACHINE_TYPE}" \
    --min-cpu-platform "${MIN_CPU_PLATFORM}" \
    --maintenance-policy TERMINATE \
    --enable-nested-virtualization \
    --boot-disk-type pd-ssd \
    --boot-disk-size "${BOOT_DISK_SIZE}" \
    --image-family "${IMAGE_FAMILY}" \
    --image-project "${IMAGE_PROJECT}" \
    --tags "${NETWORK_TAG}" \
    --metadata enable-oslogin=false \
    --metadata-from-file ssh-keys="${SSH_METADATA_FILE}"
else
  echo "Instance ${INSTANCE_NAME} already exists; skipping creation"
  gcloud compute instances add-metadata "${INSTANCE_NAME}" \
    --project "${PROJECT}" \
    --zone "${ZONE}" \
    --metadata enable-oslogin=false \
    --metadata-from-file ssh-keys="${SSH_METADATA_FILE}"
fi

echo "=== Waiting for SSH availability ==="
gcloud compute ssh "${INSTANCE_NAME}" \
  --zone "${ZONE}" \
  --project "${PROJECT}" \
  --ssh-key-file "${SSH_KEY_FILE}" \
  --ssh-flag='-o IdentitiesOnly=yes' \
  --command "echo Instance reachable" >/dev/null

echo "=== Uploading project archive ==="
gcloud compute scp "${ARCHIVE}" "${INSTANCE_NAME}:/tmp/android-mitm-mvp.tar.gz" \
  --zone "${ZONE}" --project "${PROJECT}" \
  --ssh-key-file "${SSH_KEY_FILE}" \
  --scp-flag='-o IdentitiesOnly=yes' \
  --quiet

echo "=== Installing dependencies, building image, and launching container ==="
read -r -d '' REMOTE_COMMAND <<EOF || true
set -euxo pipefail
sudo apt-get update
sudo apt-get install -y docker.io tar
sudo systemctl enable docker
sudo systemctl start docker
TMPDIR=\$(mktemp -d)
tar -xzf /tmp/android-mitm-mvp.tar.gz -C "\$TMPDIR"
cd "\$TMPDIR"/${REPO_BASENAME}
sudo docker build -t android-mitm-mvp .
sudo docker rm -f android-mitm-mvp || true
sudo docker run -d \\
  --name android-mitm-mvp \\
  --privileged \\
  --device /dev/kvm \\
  -e WEB_VNC=true \\
  -e APP_PACKAGE="${APP_PACKAGE}" \\
  -e EMULATOR_ADDITIONAL_ARGS="${EMULATOR_ADDITIONAL_ARGS}" \\
  -e EMULATOR_DATA_PARTITION="${EMULATOR_DATA_PARTITION}" \\
  -p 0.0.0.0:6080:6080 \\
  -p 0.0.0.0:8081:8081 \\
  android-mitm-mvp
rm -rf "\$TMPDIR" /tmp/android-mitm-mvp.tar.gz
EOF

gcloud compute ssh "${INSTANCE_NAME}" \
  --zone "${ZONE}" \
  --project "${PROJECT}" \
  --ssh-key-file "${SSH_KEY_FILE}" \
  --ssh-flag='-o IdentitiesOnly=yes' \
  --command "${REMOTE_COMMAND}"

echo "=== Deployment complete ==="
echo "Access noVNC at:   http://$(gcloud compute instances describe "${INSTANCE_NAME}" --zone "${ZONE}" --project "${PROJECT}" --format='get(networkInterfaces[0].accessConfigs[0].natIP)'):6080"
echo "Access mitmproxy at: http://$(gcloud compute instances describe "${INSTANCE_NAME}" --zone "${ZONE}" --project "${PROJECT}" --format='get(networkInterfaces[0].accessConfigs[0].natIP)'):8081"
