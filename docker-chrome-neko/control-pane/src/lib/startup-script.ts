/**
 * Generates the VM startup script that:
 * 1. Installs Docker and Node.js
 * 2. Pulls and runs neko-chromium container with CDP enabled
 * 3. Installs and starts CDP agent for network capture/JS injection
 * 4. Starts Cloudflare Tunnels for neko and CDP agent
 * 5. Schedules auto-shutdown after TTL
 * 6. Reports progress via /status.json for frontend polling
 */
export function generateStartupScript(ttlMinutes: number): string {
  // CDP Agent code embedded in startup script
  const cdpAgentCode = `
const express = require('express');
const puppeteer = require('puppeteer-core');
const http = require('http');
const WebSocket = require('ws');
const cors = require('cors');

const app = express();
const server = http.createServer(app);
const wss = new WebSocket.Server({ noServer: true });

app.use(cors());
app.use(express.json());

const PORT = 3001;
const CDP_PORT = 9222;

let browser = null;
let cdpConnected = false;
let currentSession = { id: null, context: null, page: null, cdpClient: null, createdAt: null, viewport: { width: 1280, height: 720 } };
let persistentScripts = [];
let appliedScriptCount = 0;
const clients = new Set();
const responseBodyCache = new Map();

wss.on('connection', (ws) => {
  clients.add(ws);
  ws.send(JSON.stringify({ type: 'STATUS', payload: { cdpConnected, viewport: currentSession.viewport } }));
  ws.on('close', () => clients.delete(ws));
});

function broadcast(type, payload) {
  const msg = JSON.stringify({ type, payload });
  clients.forEach(c => c.readyState === WebSocket.OPEN && c.send(msg));
}

app.get('/healthz', (req, res) => res.json({ ok: true, cdpConnected }));
app.get('/api/status', (req, res) => res.json({ cdpConnected, cdpPort: CDP_PORT, viewport: currentSession.viewport, persistentScriptCount: persistentScripts.length }));

app.post('/api/navigate', async (req, res) => {
  if (!currentSession.page) return res.status(503).json({ error: 'No active session' });
  try { await currentSession.page.goto(req.body.url, { waitUntil: 'domcontentloaded' }); res.json({ success: true }); }
  catch (e) { res.status(500).json({ error: e.message }); }
});

app.post('/api/inject', async (req, res) => {
  if (!currentSession.page) return res.status(503).json({ error: 'No active session' });
  try { const result = await currentSession.page.evaluate(req.body.code); res.json({ success: true, result }); }
  catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/viewport', (req, res) => res.json(currentSession.viewport));
app.post('/api/viewport', async (req, res) => {
  if (!currentSession.page) return res.status(503).json({ error: 'No active session' });
  const w = Math.max(320, Math.min(3840, Number(req.body.width) || 1280));
  const h = Math.max(240, Math.min(2160, Number(req.body.height) || 720));
  try {
    await currentSession.page.setViewport({ width: w, height: h, deviceScaleFactor: 1 });
    currentSession.viewport = { width: w, height: h };
    broadcast('VIEWPORT_CHANGED', currentSession.viewport);
    res.json({ success: true, ...currentSession.viewport });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/inject/persist', (req, res) => res.json({ scripts: persistentScripts.map((c,i) => ({id:i, code:c.slice(0,100)})), count: persistentScripts.length }));
app.post('/api/inject/persist', async (req, res) => {
  if (!req.body.code) return res.status(400).json({ error: 'code required' });
  persistentScripts.push(req.body.code);
  if (currentSession.context) {
    for (let i = appliedScriptCount; i < persistentScripts.length; i++) {
      await currentSession.context.addInitScript(persistentScripts[i]);
    }
    appliedScriptCount = persistentScripts.length;
  }
  res.json({ success: true, count: persistentScripts.length });
});
app.delete('/api/inject/persist', (req, res) => { persistentScripts = []; appliedScriptCount = 0; res.json({ success: true }); });

app.get('/api/network/:requestId/body', async (req, res) => {
  const cached = responseBodyCache.get(req.params.requestId);
  if (cached) return res.json(cached);
  if (currentSession.cdpClient) {
    try {
      const result = await currentSession.cdpClient.send('Network.getResponseBody', { requestId: req.params.requestId });
      responseBodyCache.set(req.params.requestId, result);
      return res.json(result);
    } catch (e) { return res.status(404).json({ error: 'Not available' }); }
  }
  res.status(404).json({ error: 'Not found' });
});

server.on('upgrade', (req, socket, head) => {
  if (req.url?.startsWith('/ws')) { wss.handleUpgrade(req, socket, head, ws => wss.emit('connection', ws, req)); }
  else socket.destroy();
});

async function setupNetworkCapture(page) {
  const client = await page.target().createCDPSession();
  await client.send('Network.enable');
  client.on('Network.requestWillBeSent', e => broadcast('NETWORK_REQUEST', { requestId: e.requestId, url: e.request.url, method: e.request.method, type: e.type, timestamp: Date.now() }));
  client.on('Network.responseReceived', e => broadcast('NETWORK_RESPONSE', { requestId: e.requestId, url: e.response.url, status: e.response.status, mimeType: e.response.mimeType, timestamp: Date.now() }));
  client.on('Network.loadingFinished', async e => { try { const r = await client.send('Network.getResponseBody', { requestId: e.requestId }); responseBodyCache.set(e.requestId, r); } catch {} });
  currentSession.cdpClient = client;
}

async function connectToBrowser() {
  try {
    browser = await puppeteer.connect({ browserURL: 'http://127.0.0.1:9222' });
    const contexts = browser.browserContexts();
    currentSession.context = contexts[0] || await browser.createIncognitoBrowserContext();
    const pages = await currentSession.context.pages();
    currentSession.page = pages[0] || await currentSession.context.newPage();
    currentSession.id = 'session_' + Date.now();
    currentSession.createdAt = Date.now();
    cdpConnected = true;
    await currentSession.page.setViewport({ width: 1280, height: 720, deviceScaleFactor: 1 });
    await setupNetworkCapture(currentSession.page);
    console.log('CDP connected to browser');
    broadcast('CDP_CONNECTED', { sessionId: currentSession.id });
  } catch (err) {
    cdpConnected = false;
    console.log('CDP connect failed, retrying...', err.message);
    setTimeout(connectToBrowser, 3000);
  }
}

server.listen(PORT, '0.0.0.0', () => {
  console.log('CDP Agent on port', PORT);
  setTimeout(connectToBrowser, 5000);
});
`;

  return `#!/bin/bash
set -e

exec > >(tee /var/log/startup-script.log) 2>&1
echo "Starting neko-chrome + CDP setup at $(date)"
START_TIME=$(date +%s)

mkdir -p /var/www/html
update_status() {
  local stage="$1" step="$2" total="$3" msg="$4" tunnel="\${5:-}" cdp_tunnel="\${6:-}"
  local pct=$((step * 100 / total)) ts=$(date +%s000) elapsed=$(($(date +%s) - START_TIME))
  cat > /var/www/html/status.json <<EOF
{"stage":"$stage","step":$step,"totalSteps":$total,"message":"$msg","percent":$pct,"timestamp":$ts,"tunnelUrl":"$tunnel","cdpAgentUrl":"$cdp_tunnel","elapsedSeconds":$elapsed}
EOF
  echo "[PROGRESS] Step $step/$total ($pct%) [\${elapsed}s]: $msg"
}

TOTAL_STEPS=12

cd /var/www/html
nohup python3 -m http.server 8080 --bind 0.0.0.0 > /var/log/status-server.log 2>&1 &
cd /
ufw allow 8080/tcp || true

# Step 1: System update
update_status "updating" 1 $TOTAL_STEPS "Updating system..."
apt-get update

# Step 2: Install Docker
update_status "docker" 2 $TOTAL_STEPS "Installing Docker..."
apt-get install -y ca-certificates curl gnupg lsb-release
mkdir -p /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
apt-get update
apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin
systemctl start docker && systemctl enable docker

# Step 3: Install Node.js
update_status "nodejs" 3 $TOTAL_STEPS "Installing Node.js..."
curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
apt-get install -y nodejs

# Step 4: Download cloudflared in background
update_status "deps" 4 $TOTAL_STEPS "Downloading dependencies..."
curl -L --output /usr/local/bin/cloudflared https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64 &
CF_PID=$!

# Step 5: Pull neko-chromium image  
update_status "pull" 5 $TOTAL_STEPS "Pulling neko-chromium image..."
docker pull ghcr.io/m1k1o/neko/chromium:latest

wait $CF_PID && chmod +x /usr/local/bin/cloudflared

# Step 6: Start neko container with CDP enabled
update_status "neko" 6 $TOTAL_STEPS "Starting neko container..."
docker run -d --name neko-chrome \\
  --shm-size=2gb \\
  -p 8081:8080 \\
  -p 52000-52100:52000-52100/udp \\
  -p 9222:9222 \\
  -e NEKO_MEMBER_MULTIUSER_USER_PASSWORD=neko \\
  -e NEKO_MEMBER_MULTIUSER_ADMIN_PASSWORD=admin \\
  -e NEKO_WEBRTC_EPR=52000-52100 \\
  -e NEKO_WEBRTC_ICELITE=true \\
  -e NEKO_SESSION_IMPLICIT_HOSTING=true \\
  -e NEKO_CAPTURE_AUDIO_ENABLED=false \\
  -e NEKO_DESKTOP_SCREEN=1280x720@30 \\
  ghcr.io/m1k1o/neko/chromium:latest

# Step 7: Wait for neko
update_status "boot" 7 $TOTAL_STEPS "Waiting for neko to start..."
for i in {1..60}; do
  curl -s http://localhost:8081 > /dev/null 2>&1 && break
  sleep 2
done

# Step 8: Setup CDP Agent
update_status "cdp" 8 $TOTAL_STEPS "Setting up CDP agent..."
mkdir -p /opt/cdp-agent
cd /opt/cdp-agent

cat > package.json <<'PKGEOF'
{"name":"cdp-agent","dependencies":{"cors":"^2.8.5","express":"^4.18.2","puppeteer-core":"^21.6.1","ws":"^8.14.2"}}
PKGEOF

cat > index.js <<'JSEOF'
${cdpAgentCode}
JSEOF

npm install --production

# Step 9: Start CDP Agent
update_status "cdp_start" 9 $TOTAL_STEPS "Starting CDP agent..."
nohup node index.js > /var/log/cdp-agent.log 2>&1 &
sleep 3

# Step 10: Start neko tunnel
update_status "tunnel1" 10 $TOTAL_STEPS "Starting neko tunnel..."
nohup /usr/local/bin/cloudflared tunnel --url http://localhost:8081 > /var/log/cloudflared-neko.log 2>&1 &
sleep 5

TUNNEL_URL=""
for i in {1..20}; do
  TUNNEL_URL=$(grep -o 'https://[^[:space:]]*\\.trycloudflare\\.com' /var/log/cloudflared-neko.log 2>/dev/null | head -1 || echo "")
  [ -n "$TUNNEL_URL" ] && break
  sleep 2
done
[ -z "$TUNNEL_URL" ] && TUNNEL_URL="http://$(curl -s http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip -H 'Metadata-Flavor: Google'):8081"

# Step 11: Start CDP agent tunnel
update_status "tunnel2" 11 $TOTAL_STEPS "Starting CDP agent tunnel..." "$TUNNEL_URL"
nohup /usr/local/bin/cloudflared tunnel --url http://localhost:3001 > /var/log/cloudflared-cdp.log 2>&1 &
sleep 5

CDP_TUNNEL_URL=""
for i in {1..20}; do
  CDP_TUNNEL_URL=$(grep -o 'https://[^[:space:]]*\\.trycloudflare\\.com' /var/log/cloudflared-cdp.log 2>/dev/null | head -1 || echo "")
  [ -n "$CDP_TUNNEL_URL" ] && break
  sleep 2
done
[ -z "$CDP_TUNNEL_URL" ] && CDP_TUNNEL_URL="http://$(curl -s http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip -H 'Metadata-Flavor: Google'):3001"

echo "$TUNNEL_URL" > /var/www/html/tunnel_url.txt
echo "$CDP_TUNNEL_URL" > /var/www/html/cdp_tunnel_url.txt

ufw allow 8081/tcp || true
ufw allow 3001/tcp || true
ufw allow 52000:52100/udp || true

# Step 12: Ready
TOTAL_TIME=$(($(date +%s) - START_TIME))
update_status "ready" 12 $TOTAL_STEPS "Ready! (took \${TOTAL_TIME}s)" "$TUNNEL_URL" "$CDP_TUNNEL_URL"

# Auto-delete after TTL
INSTANCE_NAME=$(curl -s "http://metadata.google.internal/computeMetadata/v1/instance/name" -H "Metadata-Flavor: Google")
ZONE=$(curl -s "http://metadata.google.internal/computeMetadata/v1/instance/zone" -H "Metadata-Flavor: Google" | awk -F'/' '{print $NF}')
PROJECT=$(curl -s "http://metadata.google.internal/computeMetadata/v1/project/project-id" -H "Metadata-Flavor: Google")

(sleep $((${ttlMinutes} * 60)) && gcloud compute instances delete "$INSTANCE_NAME" --zone="$ZONE" --project="$PROJECT" --quiet) &

echo "=== Setup complete ==="
echo "Neko: $TUNNEL_URL"
echo "CDP Agent: $CDP_TUNNEL_URL"
docker ps
`;
}
