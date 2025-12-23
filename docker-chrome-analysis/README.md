# Docker Chrome Architecture Analysis

## Overview
A serverless-ready, remotely controllable Chromium instance with full network capture and automation capabilities.

## Architecture Components

### 1. Control Panel (Next.js Frontend)
- **Location**: `control-pane/`
- **Framework**: Next.js 15 with App Router
- **Styling**: Tailwind CSS
- **State Management**: React hooks + WebSocket
- **Components**:
  - `BrowserFrame`: WebRTC stream viewer with automation overlay
  - `NetworkPanel`: Real-time network request inspector
  - `ControlPanel`: Navigation and script execution controls
  - `DataPanel`: Automation data display

### 2. Bridge Server (Node.js)
- **Location**: `server/index.js`
- **Technology**: Express + WebSocket + Playwright
- **Key Features**:
  - CDP connection management
  - Network event capture via CDP Network domain
  - Playwright automation API
  - Session management
  - Proxy authentication support

### 3. Container (Docker)
- **Base Image**: linuxserver/chromium:latest
- **Services** (via supervisord):
  - Xvfb: Virtual display server
  - Selkies: WebRTC streaming server
  - Chrome: Headless Chromium with CDP
  - Bridge Server: Node.js API server

## Data Flow Architecture

### Network Event Capture
1. **CDP Network Domain**: `Network.enable()` enables network monitoring
2. **Event Listeners**: 
   - `Network.requestWillBeSent`: Captures outgoing requests
   - `Network.responseReceived`: Captures response headers
   - `Network.loadingFinished`: Captures response bodies
3. **Storage**: LRU cache with 1000 request limit
4. **WebSocket Broadcast**: Real-time updates to frontend
5. **Frontend Display**: NetworkPanel with filtering and details view

### Playwright Script Execution
1. **API Wrapper**: `createPlaywrightAPI()` provides high-level automation functions
2. **Async Execution**: Scripts run as AsyncFunction with playwright context
3. **Visual Feedback**: Cursor animation and overlay during execution
4. **Data Collection**: `playwright.data` object for scraped data
5. **Network Capture**: `captureNetwork()` for API response monitoring
6. **User Interaction**: `promptUser()` for manual intervention

### WebRTC Streaming Architecture
1. **Selkies GStreamer**: WebRTC-based streaming (low latency)
2. **Display Server**: Xvfb provides virtual display
3. **Browser Integration**: Chrome renders to virtual display
4. **Client**: BrowserFrame component displays WebRTC stream
5. **Resolution**: Fixed 430x932 (iPhone SE) viewport

### Session Management
1. **Profile Creation**: Fresh Chrome user data directory per session
2. **CDP Connection**: Playwright connects via `chromium.connectOverCDP()`
3. **Context Management**: Single browser context with multiple pages
4. **Script Injection**: Stealth scripts applied via `Page.addScriptToEvaluateOnNewDocument`
5. **Viewport Control**: Fixed mobile viewport with device emulation

## Key APIs

### Bridge Server Endpoints
- `GET /api/status`: CDP connection status
- `POST /api/navigate`: Navigate to URL
- `POST /api/inject`: Execute JavaScript
- `POST /api/viewport`: Set viewport dimensions
- `POST /api/paste`: Insert text via CDP Input domain
- `GET/POST /api/network/*`: Network request details
- `POST /api/automation/start`: Execute Playwright script
- `POST /api/session/kill`: Restart browser with fresh profile

### Playwright API Wrapper
- `goto(url)`: Navigate with visual feedback
- `click(selector)`: Click with cursor animation
- `type(selector, text)`: Type with keystroke simulation
- `fill(selector, text)`: Fast form filling
- `scrapeText/Attribute/All`: Data extraction
- `captureNetwork()`: API response monitoring
- `promptUser()`: Manual intervention workflow
- `waitForNetworkCapture()`: Wait for specific API responses

### WebSocket Events
- `NETWORK_REQUEST/RESPONSE/FAILED`: Network activity
- `AUTOMATION_MODE_CHANGED`: Script execution state
- `AUTOMATION_CURSOR`: Visual cursor position
- `AUTOMATION_DATA_UPDATED`: Scraped data updates
- `SESSION_KILLED`: Browser restart notifications

## Deployment Options

### Cloud Run (Primary)
- Single port (8080) for WebRTC + API
- Stateless with session affinity
- Residential proxy support
- Auto-scaling with 2 CPU, 2GB RAM

### VM Deployment (Alternative)
- Persistent VM with static IP
- Direct SSH access for debugging
- Firewall management
- Idempotent deployment script

## Security Features

### Stealth Mode
- `navigator.webdriver` override
- Plugin array spoofing
- User agent randomization
- Context menu blocking
- Keyboard shortcut blocking

### Browser Lockdown
- No URL bar or tabs (`--start-fullscreen`)
- No right-click context menu
- Blocked new windows/popups
- Disabled dev tools
- Window decorations removed

### Network Security
- HTTPS traffic decryption via CDP
- Proxy authentication support
- Certificate trust store management

## Performance Optimizations

### Network Capture
- LRU cache with eviction
- Base64 encoding for binary content
- Selective body capture
- Real-time filtering

### Streaming
- WebRTC over VNC (better latency)
- Fixed resolution (no dynamic scaling)
- Hardware acceleration disabled
- Minimal UI overhead

### Automation
- Cursor animation for visual feedback
- Async execution with error handling
- Data persistence across script runs
- Network capture integration

## Development Workflow

1. **Local Development**: `docker build && docker run` with port mapping
2. **Cloud Deployment**: `./deploy.sh` for Cloud Run
3. **VM Deployment**: `./deploy-vm.sh` for persistent testing
4. **Debugging**: SSH access to VM containers
5. **Monitoring**: WebSocket connection status + CDP health checks

## Integration Points

- **CDP Protocol**: Direct Chrome DevTools Protocol usage
- **Playwright**: Browser automation framework
- **Selkies**: WebRTC streaming technology
- **WebSocket**: Real-time frontend updates
- **Express**: REST API server
- **Supervisor**: Multi-service orchestration
