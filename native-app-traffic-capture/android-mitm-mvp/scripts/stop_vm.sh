#!/usr/bin/env bash
# Tear down the GCE VM and optional firewall rule created by start_vm.sh.
set -euo pipefail

PROJECT="${PROJECT:-corsali-development}"
ZONE="${ZONE:-us-central1-a}"
INSTANCE_NAME="${INSTANCE_NAME:-android-mitm-mvp}"
FIREWALL_RULE="${FIREWALL_RULE:-android-mitm-mvp-ports}"
DELETE_FIREWALL="${DELETE_FIREWALL:-false}"

echo "=== Setting gcloud project to ${PROJECT} ==="
gcloud config set project "${PROJECT}" >/dev/null

if gcloud compute instances describe "${INSTANCE_NAME}" --zone "${ZONE}" --project "${PROJECT}" >/dev/null 2>&1; then
  echo "=== Deleting instance ${INSTANCE_NAME} ==="
  gcloud compute instances delete "${INSTANCE_NAME}" \
    --zone "${ZONE}" \
    --project "${PROJECT}" \
    --quiet
else
  echo "Instance ${INSTANCE_NAME} not found; nothing to delete"
fi

if [[ "${DELETE_FIREWALL}" == "true" ]]; then
  if gcloud compute firewall-rules describe "${FIREWALL_RULE}" --project "${PROJECT}" >/dev/null 2>&1; then
    echo "=== Deleting firewall rule ${FIREWALL_RULE} ==="
    gcloud compute firewall-rules delete "${FIREWALL_RULE}" --project "${PROJECT}" --quiet
  else
    echo "Firewall rule ${FIREWALL_RULE} not found; nothing to delete"
  fi
else
  echo "Skipping firewall deletion (set DELETE_FIREWALL=true to remove)"
fi
