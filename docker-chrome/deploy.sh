#!/bin/bash
set -e

if [ -f .env ]; then
    source .env
fi

PROJECT_ID="corsali-development"
REGION="us-central1"
SERVICE_NAME="docker-chrome"
IMAGE_NAME="gcr.io/${PROJECT_ID}/${SERVICE_NAME}"

echo "🚀 Building and deploying to ${PROJECT_ID}..."

echo "📦 Building container..."
gcloud builds submit --tag "${IMAGE_NAME}" . --project "${PROJECT_ID}"

CHROME_ARGS="--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1 --remote-allow-origins=* --no-first-run --start-fullscreen --start-maximized --block-new-web-contents --disable-infobars --disable-extensions --disable-dev-tools --allow-running-insecure-content"

if [ -n "${PROXY_SERVER}" ]; then
    CHROME_ARGS="${CHROME_ARGS} --proxy-server=${PROXY_SERVER}"
    echo "🔒 Proxy enabled: ${PROXY_SERVER}"
else
    echo "🌐 Proxy disabled (set PROXY_SERVER in .env to enable)"
fi

CHROME_ARGS="${CHROME_ARGS} about:blank"

ENV_VARS="CHROME_CLI=${CHROME_ARGS}"
ENV_VARS="${ENV_VARS},CDP_PORT=9222"
ENV_VARS="${ENV_VARS},NO_DECOR=true"
ENV_VARS="${ENV_VARS},HARDEN_DESKTOP=true"
ENV_VARS="${ENV_VARS},DISABLE_MOUSE_BUTTONS=true"
ENV_VARS="${ENV_VARS},HARDEN_KEYBINDS=true"
ENV_VARS="${ENV_VARS},SELKIES_UI_SHOW_SIDEBAR=false"
ENV_VARS="${ENV_VARS},HARDEN_OPENBOX=true"
ENV_VARS="${ENV_VARS},SELKIES_MANUAL_WIDTH=430"
ENV_VARS="${ENV_VARS},SELKIES_MANUAL_HEIGHT=932"

if [ -n "${PROXY_SERVER}" ]; then
    ENV_VARS="${ENV_VARS},PROXY_SERVER=${PROXY_SERVER}"
    ENV_VARS="${ENV_VARS},PROXY_USERNAME=${PROXY_USERNAME}"
    ENV_VARS="${ENV_VARS},PROXY_PASSWORD=${PROXY_PASSWORD}"
fi

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
  --set-env-vars="${ENV_VARS}"

echo "✅ Deployment complete!"
gcloud run services describe "${SERVICE_NAME}" --platform managed --region "${REGION}" --project "${PROJECT_ID}" --format 'value(status.url)'
