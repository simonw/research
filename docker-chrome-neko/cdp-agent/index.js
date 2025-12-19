/**
 * CDP Agent for docker-chrome-neko
 * 
 * Provides:
 * - Network traffic capture and broadcasting via WebSocket
 * - JavaScript injection (one-time and persistent)
 * - Viewport sync for responsive resize
 * - Navigation control
 * - Session management
 * 
 * Adapted from docker-chrome/server/index.js
 */

const express = require('express');
const { chromium } = require('puppeteer-core');
const http = require('http');
const WebSocket = require('ws');
const cors = require('cors');

const app = express();
const server = http.createServer(app);
const wss = new WebSocket.Server({ noServer: true });

app.use(cors());
app.use(express.json());

const PORT = process.env.CDP_AGENT_PORT || 3001;
const CDP_PORT = process.env.CDP_PORT || 9222;
const CDP_HOST = process.env.CDP_HOST || '127.0.0.1';

let browser = null;
let cdpConnected = false;

let currentSession = {
    id: null,
    context: null,
    page: null,
    cdpClient: null,
    createdAt: null,
    viewport: { width: 1280, height: 720 }
};

let persistentScripts = [];
let appliedScriptCount = 0;

const clients = new Set();
const responseBodyCache = new Map();
const MAX_CACHE_SIZE = 100;

// WebSocket connection handling
wss.on('connection', (ws) => {
    clients.add(ws);
    console.log(`WebSocket client connected. Total clients: ${clients.size}`);
    
    // Send current status on connect
    ws.send(JSON.stringify({
        type: 'STATUS',
        payload: {
            cdpConnected,
            viewport: currentSession.viewport,
            persistentScriptCount: persistentScripts.length
        }
    }));
    
    ws.on('close', () => {
        clients.delete(ws);
        console.log(`WebSocket client disconnected. Total clients: ${clients.size}`);
    });
});

function broadcast(type, payload) {
    const msg = JSON.stringify({ type, payload });
    clients.forEach(client => {
        if (client.readyState === WebSocket.OPEN) {
            client.send(msg);
        }
    });
}

// Health check
app.get('/healthz', (req, res) => {
    res.json({ ok: true, cdpConnected });
});

// Status endpoint
app.get('/api/status', (req, res) => {
    res.json({
        cdpConnected,
        cdpPort: Number(CDP_PORT),
        cdpHost: CDP_HOST,
        wsPath: '/ws',
        viewport: currentSession.viewport,
        persistentScriptCount: persistentScripts.length,
        sessionId: currentSession.id
    });
});

// Session info
app.get('/api/session', (req, res) => {
    res.json({
        sessionId: currentSession.id,
        createdAt: currentSession.createdAt,
        hasPage: !!currentSession.page,
        viewport: currentSession.viewport
    });
});

// Reset session (clear persistent scripts, reconnect)
app.post('/api/session/reset', async (req, res) => {
    try {
        persistentScripts = [];
        appliedScriptCount = 0;
        
        if (browser) {
            await browser.close().catch(() => {});
        }
        
        browser = null;
        cdpConnected = false;
        
        currentSession = {
            id: `session_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
            context: null,
            page: null,
            cdpClient: null,
            createdAt: Date.now(),
            viewport: { width: 1280, height: 720 }
        };
        
        broadcast('SESSION_RESET', { sessionId: currentSession.id, timestamp: Date.now() });
        
        // Reconnect to browser
        setTimeout(connectToBrowser, 500);
        
        res.json({
            success: true,
            sessionId: currentSession.id,
            message: 'Session reset. Persistent scripts cleared.'
        });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

// Navigate to URL
app.post('/api/navigate', async (req, res) => {
    if (!currentSession.page) {
        return res.status(503).json({ error: 'No active session' });
    }
    
    const { url } = req.body;
    if (!url) {
        return res.status(400).json({ error: 'url is required' });
    }
    
    try {
        await currentSession.page.goto(url, { waitUntil: 'domcontentloaded' });
        res.json({ success: true, url });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

// One-time JavaScript injection
app.post('/api/inject', async (req, res) => {
    if (!currentSession.page) {
        return res.status(503).json({ error: 'No active session' });
    }
    
    const { code } = req.body;
    if (!code) {
        return res.status(400).json({ error: 'code is required' });
    }
    
    try {
        const result = await currentSession.page.evaluate(code);
        res.json({ success: true, result });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

// Viewport management
app.get('/api/viewport', (req, res) => {
    res.json(currentSession.viewport);
});

app.post('/api/viewport', async (req, res) => {
    if (!currentSession.page) {
        return res.status(503).json({ error: 'No active session' });
    }
    
    const { width, height } = req.body;
    const w = Math.max(320, Math.min(3840, Number(width) || 1280));
    const h = Math.max(240, Math.min(2160, Number(height) || 720));
    
    try {
        await currentSession.page.setViewport({
            width: w,
            height: h,
            deviceScaleFactor: 1
        });
        
        currentSession.viewport = { width: w, height: h };
        broadcast('VIEWPORT_CHANGED', currentSession.viewport);
        
        res.json({ success: true, ...currentSession.viewport });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

// Persistent scripts
app.get('/api/inject/persist', (req, res) => {
    res.json({
        scripts: persistentScripts.map((code, index) => ({ id: index, code: code.slice(0, 100) + '...' })),
        count: persistentScripts.length
    });
});

app.post('/api/inject/persist', async (req, res) => {
    const { code } = req.body;
    if (typeof code !== 'string' || code.length === 0) {
        return res.status(400).json({ error: 'code must be a non-empty string' });
    }
    if (code.length > 50000) {
        return res.status(400).json({ error: 'code too large (max 50KB)' });
    }
    
    persistentScripts.push(code);
    
    try {
        await applyPendingPersistentScripts();
        res.json({ success: true, count: persistentScripts.length });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

app.delete('/api/inject/persist', (req, res) => {
    persistentScripts = [];
    appliedScriptCount = 0;
    res.json({ success: true, count: 0 });
});

// Paste text
app.post('/api/paste', async (req, res) => {
    if (!currentSession.page) {
        return res.status(503).json({ error: 'No active session' });
    }
    
    const { text, selector } = req.body;
    if (typeof text !== 'string') {
        return res.status(400).json({ error: 'text must be a string' });
    }
    
    try {
        if (selector) {
            await currentSession.page.click(selector);
        }
        
        await currentSession.page.keyboard.type(text);
        res.json({ success: true, length: text.length });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

// Get response body for a network request
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
            
            // Limit cache size
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

// WebSocket upgrade handling
server.on('upgrade', (req, socket, head) => {
    if (req.url && req.url.startsWith('/ws')) {
        wss.handleUpgrade(req, socket, head, (ws) => {
            wss.emit('connection', ws, req);
        });
    } else {
        socket.destroy();
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
    console.log(`Applied ${end - start} persistent scripts`);
}

async function applyStealthScripts() {
    if (!currentSession.page) return;
    
    const stealthScript = `
        // Remove automation indicators
        Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
        Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3, 4, 5] });
        Object.defineProperty(navigator, 'languages', { get: () => ['en-US', 'en'] });
        window.chrome = { runtime: {} };
        
        // Disable right-click context menu
        document.addEventListener('contextmenu', e => e.preventDefault(), true);
        
        // Block keyboard shortcuts that could break kiosk mode
        document.addEventListener('keydown', e => {
            // Block Ctrl+T (new tab), Ctrl+N (new window)
            if (e.ctrlKey && ['t', 'n'].includes(e.key.toLowerCase())) {
                e.preventDefault();
                e.stopPropagation();
            }
            // Block F12 (dev tools)
            if (e.key === 'F12') {
                e.preventDefault();
                e.stopPropagation();
            }
        }, true);
    `;
    
    try {
        const client = await currentSession.page.target().createCDPSession();
        await client.send('Page.addScriptToEvaluateOnNewDocument', { source: stealthScript });
        await client.send('Runtime.evaluate', { expression: stealthScript });
        currentSession.cdpClient = client;
        console.log('Stealth scripts applied');
    } catch (err) {
        console.error('Failed to apply stealth scripts:', err.message);
    }
}

async function setupNetworkCapture(page) {
    try {
        const client = await page.target().createCDPSession();
        await client.send('Network.enable');
        
        client.on('Network.requestWillBeSent', (event) => {
            broadcast('NETWORK_REQUEST', {
                requestId: event.requestId,
                url: event.request.url,
                method: event.request.method,
                type: event.type || 'Other',
                timestamp: Date.now()
            });
        });
        
        client.on('Network.responseReceived', (event) => {
            broadcast('NETWORK_RESPONSE', {
                requestId: event.requestId,
                url: event.response.url,
                status: event.response.status,
                mimeType: event.response.mimeType,
                type: event.type,
                timestamp: Date.now()
            });
        });
        
        client.on('Network.loadingFailed', (event) => {
            broadcast('NETWORK_FAILED', {
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
                // Body not available for all requests
            }
        });
        
        console.log('Network capture enabled');
        currentSession.cdpClient = client;
    } catch (err) {
        console.error('Failed to setup network capture:', err.message);
    }
}

async function connectToBrowser() {
    const browserURL = `http://${CDP_HOST}:${CDP_PORT}`;
    console.log(`Attempting to connect to browser at ${browserURL}...`);
    
    try {
        browser = await chromium.connect({ wsEndpoint: null, browserURL });
        
        const contexts = browser.contexts();
        currentSession.context = contexts.length > 0 ? contexts[0] : await browser.newContext();
        
        const pages = await currentSession.context.pages();
        currentSession.page = pages.length > 0 ? pages[0] : await currentSession.context.newPage();
        
        currentSession.id = currentSession.id || `session_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
        currentSession.createdAt = currentSession.createdAt || Date.now();
        
        cdpConnected = true;
        appliedScriptCount = 0;
        
        // Set initial viewport
        await currentSession.page.setViewport({
            width: currentSession.viewport.width,
            height: currentSession.viewport.height,
            deviceScaleFactor: 1
        });
        
        await applyStealthScripts();
        await applyPendingPersistentScripts();
        await setupNetworkCapture(currentSession.page);
        
        console.log(`Connected to browser. Session: ${currentSession.id}`);
        broadcast('CDP_CONNECTED', { sessionId: currentSession.id });
        
    } catch (err) {
        cdpConnected = false;
        console.log(`Failed to connect to browser: ${err.message}. Retrying in 3s...`);
        setTimeout(connectToBrowser, 3000);
    }
}

// Start server
server.listen(PORT, '0.0.0.0', () => {
    console.log(`CDP Agent listening on port ${PORT}`);
    console.log(`WebSocket available at ws://0.0.0.0:${PORT}/ws`);
    
    // Wait for Chromium to start before connecting
    setTimeout(connectToBrowser, 5000);
});
