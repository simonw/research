# Docker Chrome Neko - E2E Testing Notes

## Date: 2025-12-19

## Objective
Test the end-to-end flow of the docker-chrome-neko project:
1. Start control-pane on port 3000
2. Deploy VMs to GCP
3. Verify HTTPS via Cloudflare tunnels
4. Access VMs in control pane
5. Test responsiveness
6. Test network traffic capture

---

## Architecture Understanding

### Components
1. **Control Pane** (Next.js on port 3000)
   - Main UI for managing browser sessions
   - Modes: Local (Docker) and GCP
   - Embeds neko WebRTC stream in iframe
   - Displays network traffic via WebSocket

2. **GCP VM Provisioning**
   - Creates Ubuntu 22.04 VMs with startup script
   - Installs Docker, Node.js, cloudflared
   - Pulls `ghcr.io/m1k1o/neko/chromium:latest`
   - Runs CDP agent on port 3001
   - Creates Cloudflare tunnels for HTTPS

3. **CDP Agent** (Node.js on port 3001)
   - Connects to Chrome via DevTools Protocol
   - Captures network traffic
   - Broadcasts events via WebSocket
   - Provides REST API for browser control

### Data Flow
```
User -> Control Pane -> GCP API -> VM Creation
                     -> VM starts startup script
                     -> Docker runs neko-chromium
                     -> Cloudflare tunnels created
                     -> URLs returned to control pane
                     -> iframe loads neko stream
                     -> WebSocket connects to CDP agent
                     -> Network traffic displayed
```

---

## Testing Log

### Test 1: Starting Control Pane
- Time: 00:25 AM
- Status: PASS
- Notes: Control pane was already running on port 3000. Verified with `curl http://localhost:3000` returning 200.

### Test 2: VM Deployment
- Time: 00:30 AM
- Status: PASS
- Notes: 
  - Found 2 existing sessions: neko-chrome-915b2b8a (IP: 34.61.34.206) and neko-chrome-9529ae97 (IP: 34.72.246.106)
  - Both at 100% deployment progress
  - VMs created successfully with 113 second boot time
  - Sessions properly listed via `/api/sessions` endpoint

### Test 3: Cloudflare HTTPS
- Time: 00:32 AM
- Status: PARTIAL PASS
- Notes:
  - Tunnel URLs generated: 
    - Neko: `https://admission-bibliographic-societies-handled.trycloudflare.com`
    - CDP Agent: `https://consolidation-diff-vendor-hanging.trycloudflare.com`
  - HTTP/HTTPS requests work correctly through tunnels
  - **ISSUE**: WebRTC streaming does NOT work through Cloudflare tunnels (UDP not supported)

### Test 4: VM Access in Iframe
- Time: 00:35 AM
- Status: PARTIAL PASS
- Notes:
  - Neko login screen loads correctly in iframe via Cloudflare tunnel
  - Login works (username: test, password: neko)
  - **ISSUE**: WebRTC peer connection fails through tunnel
  - WebRTC WORKS when using direct IP access (http://34.61.34.206:8081)

### Test 5: UI Responsiveness
- Time: 00:40 AM
- Status: PASS
- Notes:
  - Viewport indicator shows correct dimensions (1312x600)
  - Mode toggle (Local/GCP) works correctly
  - Session chips display progress percentage
  - Session selection updates iframe URL
  - Kill session button works

### Test 6: Network Traffic Capture
- Time: 00:45 AM
- Status: FAIL
- Notes:
  - CDP Agent is running and accessible via tunnel
  - WebSocket connection to CDP agent works
  - **ISSUE**: CDP agent cannot connect to Chrome (`cdpConnected: false`)
  - Chrome DevTools Protocol not enabled in neko-chromium container
  - Verified: `curl http://127.0.0.1:9222/json/version` inside container returns nothing

---

## Issues Found

### Issue 1: WebRTC via Cloudflare Tunnel (CRITICAL)
- **Problem**: WebRTC streaming fails through Cloudflare tunnels
- **Cause**: Cloudflare tunnels only support HTTP/HTTPS, not UDP which WebRTC requires
- **Symptoms**: 
  - Login works, video stream shows loading spinner then "Reconnecting..."
  - ICE connection state transitions: checking -> disconnected -> failed
- **Resolution Options**:
  1. Use direct IP access (requires opening firewall)
  2. Set up TURN server for WebRTC relay
  3. Use different streaming solution (VNC over WebSocket)

### Issue 2: Missing GCP Firewall Rules (FIXED)
- **Problem**: WebRTC UDP ports not open in GCP firewall
- **Fix Applied**: Created firewall rules:
  - `neko-webrtc-udp`: Allow UDP 52000-52100 for target tag `neko-chrome`
  - `neko-cdp-agent`: Allow TCP 3001 for target tag `neko-chrome`
- **Result**: WebRTC works with direct IP after fix

### Issue 3: Chrome DevTools Protocol Not Enabled (CRITICAL)
- **Problem**: CDP agent cannot connect to Chrome on port 9222
- **Cause**: neko-chromium image doesn't enable `--remote-debugging-port=9222` by default
- **Symptoms**: 
  - CDP status shows `cdpConnected: false`
  - No response from `http://127.0.0.1:9222/json/version` inside container
- **Resolution Options**:
  1. Add Chrome flags via neko environment variables (need to research)
  2. Build custom neko image with CDP enabled
  3. Use different browser image that has CDP pre-configured

---

## Configuration Used
- GCP Project: corsali-development
- GCP Zone: us-central1-a
- Machine Type: e2-standard-4
- Disk Size: 100GB
- TTL: 60 minutes
- Service Account: redroid-vm-manager@corsali-development.iam.gserviceaccount.com

## Firewall Rules Created
```bash
gcloud compute firewall-rules create neko-webrtc-udp \
  --allow udp:52000-52100 \
  --target-tags neko-chrome \
  --description "Allow UDP for neko WebRTC"

gcloud compute firewall-rules create neko-cdp-agent \
  --allow tcp:3001 \
  --target-tags neko-chrome \
  --description "Allow TCP for neko CDP agent"
```

---

## Summary

| Feature | Status | Notes |
|---------|--------|-------|
| Control Pane UI | PASS | Responsive, mode switching works |
| VM Deployment | PASS | VMs deploy in ~113 seconds |
| Cloudflare Tunnels | PASS | HTTP/HTTPS works |
| WebRTC via Tunnel | FAIL | UDP not supported |
| WebRTC via Direct IP | PASS | Works after firewall fix |
| CDP Agent | PARTIAL | Running but not connected |
| Network Capture | FAIL | Blocked by CDP issue |
| Session Management | PASS | Start/stop/list works |
