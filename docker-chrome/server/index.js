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

        if (currentSession.context) {
            await currentSession.context.close().catch(() => {});
        }

        if (browser) {
            await browser.close().catch(() => {});
        }

        currentSession = {
            id: sessionId,
            userDataDir: null,
            context: null,
            page: null,
            createdAt: Date.now()
        };

        browser = null;
        cdpConnected = false;

        const userDataDir = await restartChromeWithNewUserDataDir(sessionId);
        currentSession.userDataDir = userDataDir;

        broadcast('SESSION_RESET', { sessionId, timestamp: Date.now() });

        setTimeout(connectToBrowser, 250);

        res.json({
            success: true,
            sessionId,
            userDataDir,
            message: 'Session reset. Chrome restarted with a new user-data-dir and persistent scripts cleared.'
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

    const { width, height } = req.body;

    const w = Math.max(320, Math.min(1920, Number(width) || 375));
    const h = Math.max(480, Math.min(1080, Number(height) || 667));

    try {
        await currentSession.page.setViewportSize({ width: w, height: h });
        broadcast('VIEWPORT_CHANGED', { width: w, height: h });
        res.json({ success: true, width: w, height: h });
    } catch (e) {
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
        await execAsync(`pkill -f "--remote-debugging-port=${CDP_PORT}"`);
    } catch (e) {
        if (e && e.code !== 1) throw e;
    }

    const chromeBin = await resolveChromeBin();
    const env = { ...process.env, DISPLAY: process.env.DISPLAY || ':0' };

    const args = [
        '--no-sandbox',
        '--disable-dev-shm-usage',
        `--remote-debugging-port=${CDP_PORT}`,
        '--remote-debugging-address=0.0.0.0',
        '--load-extension=/opt/extension',
        '--no-first-run',
        '--no-default-browser-check',
        `--user-data-dir=${userDataDir}`,
        '--window-position=0,0',
        '--window-size=1920,1080',
        'about:blank'
    ];

    const proc = spawn(chromeBin, args, { detached: true, stdio: 'ignore', env });
    proc.unref();

    return userDataDir;
}

async function connectToBrowser() {
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
        await applyPendingPersistentScripts();
        setupNetworkCapture(currentSession.page, currentSession.id);
    } catch (err) {
        cdpConnected = false;
        setTimeout(connectToBrowser, 2000);
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
    } catch (err) {
        cdpConnected = false;
    }
}

server.listen(PORT, () => {
    connectToBrowser();
});
