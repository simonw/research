const express = require('express');
const { chromium } = require('playwright');
const http = require('http');
const WebSocket = require('ws');
const cors = require('cors');
const httpProxy = require('http-proxy');

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
let context = null;
let page = null;
let cdpConnected = false;

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

app.post('/api/navigate', async (req, res) => {
    if (!page) return res.status(503).json({ error: 'Browser not connected' });
    const { url } = req.body;
    try {
        await page.goto(url);
        res.json({ success: true });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

app.post('/api/inject', async (req, res) => {
    if (!page) return res.status(503).json({ error: 'Browser not connected' });
    const { code } = req.body;
    try {
        const result = await page.evaluate(code);
        res.json({ result });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

async function applyPendingPersistentScripts() {
    if (!context) return;
    const start = appliedScriptCount;
    const end = persistentScripts.length;
    if (start >= end) return;

    for (let i = start; i < end; i++) {
        const code = persistentScripts[i];
        await context.addInitScript(code);
        if (page) {
            await page.addInitScript(code);
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

async function connectToBrowser() {
    try {
        browser = await chromium.connectOverCDP(`http://127.0.0.1:${CDP_PORT}`);
        const contexts = browser.contexts();
        context = contexts.length > 0 ? contexts[0] : await browser.newContext();
        const pages = context.pages();
        page = pages.length > 0 ? pages[0] : await context.newPage();
        cdpConnected = true;
        appliedScriptCount = 0;
        await applyPendingPersistentScripts();
        setupNetworkCapture(page);
    } catch (err) {
        cdpConnected = false;
        setTimeout(connectToBrowser, 2000);
    }
}

async function setupNetworkCapture(page) {
    try {
        const client = await page.context().newCDPSession(page);
        await client.send('Network.enable');

        client.on('Network.requestWillBeSent', (event) => {
            broadcast('NETWORK_REQUEST', {
                requestId: event.requestId,
                url: event.request.url,
                method: event.request.method,
                timestamp: Date.now()
            });
        });

        client.on('Network.responseReceived', (event) => {
            broadcast('NETWORK_RESPONSE', {
                requestId: event.requestId,
                status: event.response.status,
                mimeType: event.response.mimeType,
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
