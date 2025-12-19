const express = require('express');
const { chromium } = require('playwright');
const http = require('http');
const WebSocket = require('ws');
const cors = require('cors');
const httpProxy = require('http-proxy');
const { exec, spawn } = require('child_process');
const { promisify } = require('util');
const fs = require('fs/promises');
const path = require('path');

const execAsync = promisify(exec);

const app = express();
const server = http.createServer(app);
const wss = new WebSocket.Server({ noServer: true });
const proxy = httpProxy.createProxyServer({
    target: 'http://127.0.0.1:3000',
    ws: true,
    changeOrigin: true
});

app.use(cors());
app.use(express.json());

const PORT = process.env.PORT || 8080;
const CDP_PORT = process.env.CDP_PORT || 9222;

// Fixed mobile viewport - matches SELKIES_MANUAL_WIDTH/HEIGHT in deploy.sh
// deviceScaleFactor: 1 because Xvfb display is the physical pixels
// Chrome will render at 430x932 CSS pixels filling the display exactly
const FIXED_VIEWPORT = {
    width: 430,
    height: 932,
    deviceScaleFactor: 1,
    mobile: true
};

// iPhone 15 Pro Max user agent
const MOBILE_USER_AGENT = 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1';

let browser = null;
let cdpConnected = false;

let currentSession = {
    id: null,
    userDataDir: null,
    context: null,
    page: null,
    createdAt: null
};

let persistentScripts = [];
let appliedScriptCount = 0;

const clients = new Set();

const responseBodyCache = new Map(); // requestId -> { body, base64Encoded }
const MAX_CACHE_SIZE = 100;

wss.on('connection', (ws) => {
    clients.add(ws);
    ws.on('close', () => clients.delete(ws));
});

function broadcast(type, payload) {
    const msg = JSON.stringify({ type, payload });
    clients.forEach(client => {
        if (client.readyState === WebSocket.OPEN) {
            client.send(msg);
        }
    });
}

proxy.on('error', (err, req, res) => {
    if (res && typeof res.writeHead === 'function') {
        res.writeHead(502, { 'Content-Type': 'text/plain' });
        res.end('Upstream unavailable');
        return;
    }
    if (req && req.socket) {
        req.socket.destroy();
    }
});

app.get('/healthz', (req, res) => {
    res.json({ ok: true });
});

app.get('/api/status', (req, res) => {
    res.json({
        cdpConnected,
        cdpPort: Number(CDP_PORT),
        wsPath: '/ws',
        persistentScriptCount: persistentScripts.length
    });
});

app.get('/api/session', (req, res) => {
    res.json({
        sessionId: currentSession.id,
        createdAt: currentSession.createdAt,
        hasPage: !!currentSession.page
    });
});

app.post('/api/session/reset', async (req, res) => {
    try {
        const sessionId = `session_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

        persistentScripts = [];
        appliedScriptCount = 0;

        // Don't restart Chrome - just clear cookies and navigate to blank
        // This is much more reliable than trying to respawn Chrome
        if (currentSession.page && currentSession.cdpClient) {
            try {
                await currentSession.cdpClient.send('Network.clearBrowserCookies');
                await currentSession.cdpClient.send('Network.clearBrowserCache');
                await currentSession.page.goto('about:blank');
                await applyFixedViewport();
                await applyStealthScripts();
                
                currentSession.id = sessionId;
                currentSession.createdAt = Date.now();
                
                broadcast('SESSION_RESET', { sessionId, timestamp: Date.now() });
                
                return res.json({
                    success: true,
                    sessionId,
                    message: 'Session reset. Cookies/cache cleared, navigated to blank page.'
                });
            } catch (e) {
                console.log('Soft reset failed, will attempt reconnect:', e.message);
            }
        }

        // If we don't have a connection, try to connect to existing Chrome
        currentSession = {
            id: sessionId,
            userDataDir: null,
            context: null,
            page: null,
            cdpClient: null,
            createdAt: Date.now()
        };

        browser = null;
        cdpConnected = false;

        broadcast('SESSION_RESET', { sessionId, timestamp: Date.now() });

        // Try to connect to the existing Chrome (started by linuxserver)
        setTimeout(connectToBrowser, 500);

        res.json({
            success: true,
            sessionId,
            message: 'Session reset. Reconnecting to browser...'
        });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

app.post('/api/navigate', async (req, res) => {
    if (!currentSession.page) return res.status(503).json({ error: 'No active session' });
    const { url } = req.body;
    try {
        await currentSession.page.goto(url);
        res.json({ success: true });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

app.post('/api/inject', async (req, res) => {
    if (!currentSession.page) return res.status(503).json({ error: 'No active session' });
    const { code } = req.body;
    try {
        const result = await currentSession.page.evaluate(code);
        res.json({ result });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

app.post('/api/viewport', async (req, res) => {
    if (!currentSession.page) return res.status(503).json({ error: 'No active session' });

    const { width, height, devicePixelRatio, mobile } = req.body;

    const w = Math.max(320, Math.min(1920, Number(width) || 375));
    const h = Math.max(480, Math.min(1080, Number(height) || 667));
    const dpr = Math.max(1, Math.min(3, Number(devicePixelRatio) || 1));
    // Auto-detect mobile if not specified: width < 768 is mobile
    const isMobile = typeof mobile === 'boolean' ? mobile : w < 768;

    try {
        // Use CDP Emulation.setDeviceMetricsOverride for full viewport control
        // This sets: window.innerWidth/Height, screen.width/height, DPR, and mobile mode
        const client = currentSession.cdpClient || await currentSession.page.context().newCDPSession(currentSession.page);
        
        await client.send('Emulation.setDeviceMetricsOverride', {
            width: w,
            height: h,
            deviceScaleFactor: dpr,
            mobile: isMobile,
            screenOrientation: w > h 
                ? { type: 'landscapePrimary', angle: 90 }
                : { type: 'portraitPrimary', angle: 0 }
        });
        
        // Cache CDP client for reuse
        if (!currentSession.cdpClient) {
            currentSession.cdpClient = client;
        }
        
        broadcast('VIEWPORT_CHANGED', { width: w, height: h, devicePixelRatio: dpr, mobile: isMobile });
        res.json({ success: true, width: w, height: h, devicePixelRatio: dpr, mobile: isMobile });
    } catch (e) {
        console.error('Viewport change failed:', e.message);
        res.status(500).json({ error: e.message });
    }
});


app.get('/api/viewport', async (req, res) => {
    if (!currentSession.page) return res.status(503).json({ error: 'No active session' });

    try {
        const viewport = currentSession.page.viewportSize();
        res.json(viewport || { width: 375, height: 667 });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

async function applyPendingPersistentScripts() {
    if (!currentSession.context) return;
    const start = appliedScriptCount;
    const end = persistentScripts.length;
    if (start >= end) return;

    for (let i = start; i < end; i++) {
        const code = persistentScripts[i];
        await currentSession.context.addInitScript(code);
        if (currentSession.page) {
            await currentSession.page.addInitScript(code);
        }
    }

    appliedScriptCount = end;
}

app.get('/api/inject/persist', (req, res) => {
    res.json({ scripts: persistentScripts, count: persistentScripts.length });
});

app.post('/api/inject/persist', async (req, res) => {
    const { code } = req.body;
    if (typeof code !== 'string' || code.length === 0) {
        return res.status(400).json({ error: 'code must be a non-empty string' });
    }
    if (code.length > 20000) {
        return res.status(400).json({ error: 'code too large' });
    }

    persistentScripts = [...persistentScripts, code];

    try {
        await applyPendingPersistentScripts();
        res.json({ ok: true, count: persistentScripts.length });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

app.delete('/api/inject/persist', (req, res) => {
    persistentScripts = [];
    appliedScriptCount = 0;
    res.json({ ok: true, count: 0 });
});

app.post('/api/paste', async (req, res) => {
    if (!currentSession.page) return res.status(503).json({ error: 'No active session' });
    const { text, selector } = req.body;

    if (typeof text !== 'string') {
        return res.status(400).json({ error: 'text must be a string' });
    }

    try {
        if (selector) {
            await currentSession.page.click(selector);
        }

        const client = currentSession.cdpClient || await currentSession.page.context().newCDPSession(currentSession.page);
        await client.send('Input.insertText', { text });

        res.json({ success: true, length: text.length });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

app.get('/api/network/:requestId/body', async (req, res) => {
    const { requestId } = req.params;
    const cached = responseBodyCache.get(requestId);
    if (cached) {
        return res.json(cached);
    }

    if (currentSession.cdpClient) {
        try {
            const result = await currentSession.cdpClient.send('Network.getResponseBody', { requestId });
            responseBodyCache.set(requestId, result);
            if (responseBodyCache.size > MAX_CACHE_SIZE) {
                const firstKey = responseBodyCache.keys().next().value;
                responseBodyCache.delete(firstKey);
            }
            return res.json(result);
        } catch (e) {
            return res.status(404).json({ error: 'Response body not available', details: e.message });
        }
    }

    res.status(404).json({ error: 'Response body not found' });
});

app.use((req, res, next) => {
    if (req.url.startsWith('/api') || req.url === '/healthz' || req.url.startsWith('/ws')) {
        next();
        return;
    }
    proxy.web(req, res);
});

server.on('upgrade', (req, socket, head) => {
    if (req.url && req.url.startsWith('/ws')) {
        wss.handleUpgrade(req, socket, head, (ws) => {
            wss.emit('connection', ws, req);
        });
        return;
    }
    proxy.ws(req, socket, head);
});

async function resolveChromeBin() {
    const candidates = [];
    if (process.env.CHROME_BIN) candidates.push(process.env.CHROME_BIN);
    candidates.push('google-chrome-stable', 'google-chrome', 'chromium-browser', 'chromium');

    for (const candidate of candidates) {
        try {
            await execAsync(`command -v ${candidate}`);
            return candidate;
        } catch (e) {
            continue;
        }
    }

    return candidates[0];
}

async function restartChromeWithNewUserDataDir(sessionId) {
    const profileRoot = process.env.CHROME_PROFILE_ROOT || '/tmp/chrome-profiles';
    const userDataDir = path.join(profileRoot, sessionId);
    await fs.mkdir(userDataDir, { recursive: true });

    try {
        await execAsync(`pkill -f "remote-debugging-port=${CDP_PORT}"`);
    } catch (e) {
        // pkill returns 1 if no processes matched, which is fine
        // Only throw on actual errors
    }

    const chromeBin = await resolveChromeBin();
    const env = { ...process.env, DISPLAY: process.env.DISPLAY || ':0' };

    const args = [
        '--no-sandbox',
        '--disable-dev-shm-usage',
        `--remote-debugging-port=${CDP_PORT}`,
        '--remote-debugging-address=127.0.0.1',
        '--no-first-run',
        '--no-default-browser-check',
        `--user-data-dir=${userDataDir}`,
        '--start-fullscreen',
        '--start-maximized',
        '--disable-infobars',
        '--disable-session-crashed-bubble',
        '--disable-restore-session-state',
        '--noerrdialogs',
        '--disable-translate',
        '--disable-extensions',
        '--disable-component-extensions-with-background-pages',
        '--disable-background-networking',
        '--disable-sync',
        '--disable-default-apps',
        '--disable-client-side-phishing-detection',
        '--disable-dev-tools',
        '--block-new-web-contents',
        `--user-agent=${MOBILE_USER_AGENT}`,
        'about:blank'
    ];

    const proc = spawn(chromeBin, args, { detached: true, stdio: 'ignore', env });
    proc.unref();
    
    console.log(`Spawned Chrome with PID ${proc.pid}, bin: ${chromeBin}`);
    console.log(`Chrome args: ${args.join(' ')}`);

    return userDataDir;
}

async function connectToBrowser() {
    console.log(`Attempting to connect to CDP at http://127.0.0.1:${CDP_PORT}...`);
    try {
        browser = await chromium.connectOverCDP(`http://127.0.0.1:${CDP_PORT}`);

        const contexts = browser.contexts();
        currentSession.context = contexts.length > 0 ? contexts[0] : await browser.newContext();

        const pages = currentSession.context.pages();
        currentSession.page = pages.length > 0 ? pages[0] : await currentSession.context.newPage();

        currentSession.id = currentSession.id || `session_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
        currentSession.createdAt = currentSession.createdAt || Date.now();

        cdpConnected = true;
        appliedScriptCount = 0;

        await applyStealthScripts();
        await applyPendingPersistentScripts();
        await setupNetworkCapture(currentSession.page, currentSession.id);
        
        // Apply fixed mobile viewport to match Selkies resolution
        await applyFixedViewport();
        
        // Reload the page to ensure lockdown scripts are active
        // (addScriptToEvaluateOnNewDocument only works on NEW document loads)
        try {
            await currentSession.page.reload({ waitUntil: 'domcontentloaded' });
            console.log('Page reloaded to apply lockdown scripts');
        } catch (reloadErr) {
            console.log('Page reload skipped:', reloadErr.message);
        }
    } catch (err) {
        console.log(`CDP connection failed: ${err.message}. Retrying in 2s...`);
        cdpConnected = false;
        setTimeout(connectToBrowser, 2000);
    }
}

async function applyStealthScripts() {
    if (!currentSession.page) return;

    const lockdownScript = `
        // Stealth: Remove automation flags
        Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
        Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3, 4, 5] });
        Object.defineProperty(navigator, 'languages', { get: () => ['en-US', 'en'] });
        window.chrome = { runtime: {} };
        
        // Lockdown: Disable right-click context menu
        document.addEventListener('contextmenu', e => e.preventDefault(), true);
        
        // Lockdown: Block certain keyboard shortcuts
        document.addEventListener('keydown', e => {
            // Block Ctrl+T (new tab), Ctrl+N (new window), Ctrl+Shift+N (incognito)
            if (e.ctrlKey && ['t', 'n'].includes(e.key.toLowerCase())) {
                e.preventDefault();
                e.stopPropagation();
            }
            // Block F12 (dev tools)
            if (e.key === 'F12') {
                e.preventDefault();
                e.stopPropagation();
            }
            // Block Ctrl+Shift+I, Ctrl+Shift+J, Ctrl+U (view source)
            if (e.ctrlKey && e.shiftKey && ['i', 'j'].includes(e.key.toLowerCase())) {
                e.preventDefault();
                e.stopPropagation();
            }
            if (e.ctrlKey && e.key.toLowerCase() === 'u') {
                e.preventDefault();
                e.stopPropagation();
            }
        }, true);
    `;

    try {
        const client = await currentSession.page.context().newCDPSession(currentSession.page);
        
        // Add script for all future documents
        await client.send('Page.addScriptToEvaluateOnNewDocument', {
            source: lockdownScript
        });
        
        // Also run on the current page immediately
        await client.send('Runtime.evaluate', {
            expression: lockdownScript,
            awaitPromise: false
        });
        
        currentSession.cdpClient = client;
        console.log('Stealth and lockdown scripts applied successfully');
    } catch (err) {
        console.error('Failed to apply stealth scripts:', err.message);
    }
}

async function applyFixedViewport() {
    if (!currentSession.page) return;
    
    try {
        const client = currentSession.cdpClient || await currentSession.page.context().newCDPSession(currentSession.page);
        
        await client.send('Emulation.setDeviceMetricsOverride', {
            width: FIXED_VIEWPORT.width,
            height: FIXED_VIEWPORT.height,
            deviceScaleFactor: FIXED_VIEWPORT.deviceScaleFactor,
            mobile: FIXED_VIEWPORT.mobile,
            screenOrientation: { type: 'portraitPrimary', angle: 0 }
        });
        
        await client.send('Emulation.setUserAgentOverride', {
            userAgent: MOBILE_USER_AGENT,
            platform: 'iPhone',
            userAgentMetadata: {
                brands: [{ brand: 'Safari', version: '17' }],
                fullVersionList: [{ brand: 'Safari', version: '17.0' }],
                platform: 'iOS',
                platformVersion: '17.0',
                architecture: '',
                model: 'iPhone',
                mobile: true
            }
        });
        
        if (!currentSession.cdpClient) {
            currentSession.cdpClient = client;
        }
        
        console.log(`Fixed viewport applied: ${FIXED_VIEWPORT.width}x${FIXED_VIEWPORT.height} @${FIXED_VIEWPORT.deviceScaleFactor}x mobile=${FIXED_VIEWPORT.mobile}`);
        console.log(`Mobile user agent applied: iPhone/Safari`);
    } catch (err) {
        console.error('Failed to apply fixed viewport:', err.message);
    }
}

async function setupNetworkCapture(page, sessionId = null) {
    try {
        const client = await page.context().newCDPSession(page);
        await client.send('Network.enable');

        client.on('Network.requestWillBeSent', (event) => {
            broadcast('NETWORK_REQUEST', {
                sessionId,
                requestId: event.requestId,
                url: event.request.url,
                method: event.request.method,
                type: event.type || 'Other',
                timestamp: Date.now()
            });
        });

        client.on('Network.responseReceived', (event) => {
            broadcast('NETWORK_RESPONSE', {
                sessionId,
                requestId: event.requestId,
                status: event.response.status,
                mimeType: event.response.mimeType,
                type: event.type,
                timestamp: Date.now()
            });
        });

        client.on('Network.loadingFailed', (event) => {
            broadcast('NETWORK_FAILED', {
                sessionId,
                requestId: event.requestId,
                errorText: event.errorText,
                timestamp: Date.now()
            });
        });

        client.on('Network.loadingFinished', async (event) => {
            try {
                const result = await client.send('Network.getResponseBody', { requestId: event.requestId });
                responseBodyCache.set(event.requestId, result);
                if (responseBodyCache.size > MAX_CACHE_SIZE) {
                    const firstKey = responseBodyCache.keys().next().value;
                    responseBodyCache.delete(firstKey);
                }
            } catch (e) {
                // Body may not be available for all requests (e.g., 204, redirects)
            }
        });
    } catch (err) {
        cdpConnected = false;
    }
}

server.listen(PORT, () => {
    connectToBrowser();
});
