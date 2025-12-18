const express = require('express');
const { chromium } = require('playwright');
const http = require('http');
const WebSocket = require('ws');
const cors = require('cors');

const app = express();
const server = http.createServer(app);
const wss = new WebSocket.Server({ server });

app.use(cors());
app.use(express.json());

const PORT = process.env.PORT || 8080;
const CDP_PORT = process.env.CDP_PORT || 9222;

let browser = null;
let context = null;
let page = null;

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

async function connectToBrowser() {
    try {
        console.log(`Connecting to Chrome on port ${CDP_PORT}...`);
        browser = await chromium.connectOverCDP(`http://localhost:${CDP_PORT}`);
        const contexts = browser.contexts();
        context = contexts.length > 0 ? contexts[0] : await browser.newContext();
        
        const pages = context.pages();
        page = pages.length > 0 ? pages[0] : await context.newPage();

        console.log('Connected to Chrome!');
        setupNetworkCapture(page);
    } catch (err) {
        console.error('Failed to connect to browser:', err);
        setTimeout(connectToBrowser, 2000);
    }
}

async function setupNetworkCapture(page) {
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

    console.log('Network capture enabled');
}

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

server.listen(PORT, () => {
    console.log(`Control server listening on port ${PORT}`);
    connectToBrowser();
});
