#!/bin/bash
set -e

PROJECT_ID="corsali-development"
REGION="us-central1"
SERVICE_NAME="docker-chrome"
IMAGE_NAME="gcr.io/${PROJECT_ID}/${SERVICE_NAME}"

echo "🚀 Building and deploying to ${PROJECT_ID}..."

# 1. Build and push image
echo "📦 Building container..."
gcloud builds submit --tag "${IMAGE_NAME}" . --project "${PROJECT_ID}"

# 2. Deploy to Cloud Run
echo "🚀 Deploying to Cloud Run..."
gcloud run deploy "${SERVICE_NAME}" \
  --image "${IMAGE_NAME}" \
  --platform managed \
  --region "${REGION}" \
  --project "${PROJECT_ID}" \
  --allow-unauthenticated \
  --port 8080 \
  --cpu 2 \
  --memory 2Gi \
  --session-affinity \
  --set-env-vars="CHROME_CLI=--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1 --remote-allow-origins=* --no-first-run --start-fullscreen --start-maximized --block-new-web-contents --disable-infobars --disable-extensions --disable-dev-tools about:blank" \
  --set-env-vars="CDP_PORT=9222" \
  --set-env-vars="NO_DECOR=true" \
  --set-env-vars="HARDEN_DESKTOP=true" \
  --set-env-vars="DISABLE_MOUSE_BUTTONS=true" \
  --set-env-vars="HARDEN_KEYBINDS=true" \
  --set-env-vars="SELKIES_UI_SHOW_SIDEBAR=false" \
  --set-env-vars="HARDEN_OPENBOX=true" \
  --set-env-vars="SELKIES_MANUAL_WIDTH=430" \
  --set-env-vars="SELKIES_MANUAL_HEIGHT=932"

echo "✅ Deployment complete!"
gcloud run services describe "${SERVICE_NAME}" --platform managed --region "${REGION}" --project "${PROJECT_ID}" --format 'value(status.url)'
