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

// Residential proxy configuration (optional - set all three to enable)
const PROXY_SERVER = process.env.PROXY_SERVER; // e.g., "brd.superproxy.io:33335"
const PROXY_USERNAME = process.env.PROXY_USERNAME; // e.g., "brd-customer-XXX-zone-residential"
const PROXY_PASSWORD = process.env.PROXY_PASSWORD;
const PROXY_ENABLED = Boolean(PROXY_SERVER && PROXY_USERNAME && PROXY_PASSWORD);

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

let automationState = {
    isRunning: false,
    mode: 'idle',
    data: {},
    scriptId: null,
    prompt: null,
    error: null,
    networkCaptures: [],
    capturedResponses: {},
    pendingRequests: {},
    continueResolver: null
};

function matchesUrlPattern(url, pattern) {
    if (!pattern) return true;
    return new RegExp(pattern).test(url);
}

function matchesBodyPattern(body, pattern) {
    if (!pattern) return true;
    if (!body) return false;
    return new RegExp(pattern).test(body);
}

function tryParseJson(text) {
    if (typeof text !== 'string') return text;
    try {
        return JSON.parse(text);
    } catch {
        return text;
    }
}

function checkNetworkCapture(url, requestBody, responseBody) {
    if (!automationState.isRunning || automationState.networkCaptures.length === 0) {
        return;
    }

    for (const capture of automationState.networkCaptures) {
        const { urlPattern, bodyPattern, key, transform } = capture;
        
        if (!matchesUrlPattern(url, urlPattern)) continue;
        if (!matchesBodyPattern(requestBody, bodyPattern)) continue;
        
        try {
            let data = tryParseJson(responseBody);
            
            if (transform && typeof transform === 'function') {
                data = transform(data);
            }
            
            automationState.capturedResponses[key] = data;
            automationState.data[key] = data;
            
            console.log(`[NetworkCapture] Captured: ${key}`);
            broadcast('AUTOMATION_DATA_UPDATED', { key, value: data, data: automationState.data });
            broadcast('NETWORK_CAPTURED', { key, url, timestamp: Date.now() });
        } catch (err) {
            console.error(`[NetworkCapture] Error for ${key}:`, err.message);
        }
    }
}

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

app.post('/api/session/kill', async (req, res) => {
    try {
        const sessionId = `session_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

        persistentScripts = [];
        appliedScriptCount = 0;
        responseBodyCache.clear();

        if (browser) {
            try {
                await browser.close();
            } catch (e) {
                console.log('Browser close failed (may already be closed):', e.message);
            }
        }

        browser = null;
        cdpConnected = false;
        currentSession = {
            id: sessionId,
            userDataDir: null,
            context: null,
            page: null,
            cdpClient: null,
            createdAt: Date.now()
        };

        console.log(`Killing session and restarting Chrome with fresh profile: ${sessionId}`);
        const userDataDir = await restartChromeWithNewUserDataDir(sessionId);
        currentSession.userDataDir = userDataDir;

        broadcast('SESSION_KILLED', { sessionId, timestamp: Date.now() });

        setTimeout(connectToBrowser, 1500);

        res.json({
            success: true,
            sessionId,
            userDataDir,
            message: 'Session killed. Chrome restarting with fresh user data directory.'
        });
    } catch (e) {
        console.error('Session kill failed:', e.message);
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

app.post('/api/network/clear', async (req, res) => {
    try {
        // Clear server-side response body cache
        responseBodyCache.clear();
        
        // Clear browser cache if CDP client is available
        if (currentSession.cdpClient) {
            try {
                await currentSession.cdpClient.send('Network.clearBrowserCache');
            } catch (e) {
                console.log('Failed to clear browser cache via CDP:', e.message);
            }
        }
        
        // Broadcast network clear event to all connected clients
        broadcast('NETWORK_CLEARED', { timestamp: Date.now() });
        
        res.json({ success: true, message: 'Network cache cleared' });
    } catch (e) {
        res.status(500).json({ error: e.message });
    }
});

// ==================== AUTOMATION API ====================

// Get automation status
app.get('/api/automation/status', (req, res) => {
    res.json({
        isRunning: automationState.isRunning,
        mode: automationState.mode,
        scriptId: automationState.scriptId,
        prompt: automationState.prompt,
        error: automationState.error,
        dataKeys: Object.keys(automationState.data)
    });
});

// Get automation data
app.get('/api/automation/data', (req, res) => {
    res.json(automationState.data);
});

// Update automation data (merge)
app.post('/api/automation/data', (req, res) => {
    const { key, value } = req.body;
    if (key) {
        automationState.data[key] = value;
        broadcast('AUTOMATION_DATA_UPDATED', { key, value, data: automationState.data });
    } else if (typeof req.body === 'object') {
        automationState.data = { ...automationState.data, ...req.body };
        broadcast('AUTOMATION_DATA_UPDATED', { data: automationState.data });
    }
    res.json({ success: true, data: automationState.data });
});

// Clear automation data
app.delete('/api/automation/data', (req, res) => {
    automationState.data = {};
    broadcast('AUTOMATION_DATA_UPDATED', { data: {} });
    res.json({ success: true });
});

// Helper: Set automation mode and broadcast
function setAutomationMode(mode, extra = {}) {
    automationState.mode = mode;
    broadcast('AUTOMATION_MODE_CHANGED', { mode, ...extra });
}

// Helper: Broadcast cursor position
function broadcastCursor(x, y, action = 'move') {
    broadcast('AUTOMATION_CURSOR', { x, y, action, timestamp: Date.now() });
}

// Helper: Get element center position
async function getElementCenter(selector) {
    if (!currentSession.page) throw new Error('No active page');
    
    const box = await currentSession.page.evaluate((sel) => {
        const el = document.querySelector(sel);
        if (!el) return null;
        const rect = el.getBoundingClientRect();
        return {
            x: rect.left + rect.width / 2,
            y: rect.top + rect.height / 2,
            width: rect.width,
            height: rect.height
        };
    }, selector);
    
    if (!box) throw new Error(`Element not found: ${selector}`);
    return box;
}

// Helper: Animate cursor to position
async function animateCursorTo(x, y, duration = 300) {
    const steps = 10;
    const startX = automationState.lastCursorX || 0;
    const startY = automationState.lastCursorY || 0;
    
    for (let i = 1; i <= steps; i++) {
        const progress = i / steps;
        const currentX = startX + (x - startX) * progress;
        const currentY = startY + (y - startY) * progress;
        broadcastCursor(currentX, currentY, 'move');
        await new Promise(r => setTimeout(r, duration / steps));
    }
    
    automationState.lastCursorX = x;
    automationState.lastCursorY = y;
}

// Create playwright wrapper API for scripts
function createPlaywrightAPI() {
    const page = currentSession.page;
    if (!page) throw new Error('No active page');
    
    return {
        // Navigate with cursor animation
        goto: async (url, options = {}) => {
            setAutomationMode('automation', { action: 'navigating', url });
            await page.goto(url, options);
            return true;
        },
        
        // Click with cursor animation
        click: async (selector, options = {}) => {
            const box = await getElementCenter(selector);
            await animateCursorTo(box.x, box.y);
            broadcastCursor(box.x, box.y, 'click');
            await new Promise(r => setTimeout(r, 150)); // Visual feedback delay
            await page.click(selector, options);
            return true;
        },
        
        // Type with cursor animation
        type: async (selector, text, options = {}) => {
            const box = await getElementCenter(selector);
            await animateCursorTo(box.x, box.y);
            broadcastCursor(box.x, box.y, 'click');
            await page.click(selector);
            await page.type(selector, text, { delay: options.delay || 50 });
            return true;
        },
        
        // Fill (faster than type, no keystroke animation)
        fill: async (selector, text) => {
            const box = await getElementCenter(selector);
            await animateCursorTo(box.x, box.y);
            broadcastCursor(box.x, box.y, 'click');
            await page.fill(selector, text);
            return true;
        },
        
        // Wait for selector
        waitFor: async (selector, options = {}) => {
            await page.waitForSelector(selector, { timeout: options.timeout || 30000 });
            return true;
        },
        
        // Wait for navigation
        waitForNavigation: async (options = {}) => {
            await page.waitForNavigation({ timeout: options.timeout || 30000, ...options });
            return true;
        },
        
        // Wait for URL pattern
        waitForURL: async (urlPattern, options = {}) => {
            await page.waitForURL(urlPattern, { timeout: options.timeout || 30000 });
            return true;
        },
        
        // Sleep
        sleep: async (ms) => {
            await new Promise(r => setTimeout(r, ms));
            return true;
        },
        
        // Scrape text from DOM
        scrapeText: async (selector, key) => {
            const text = await page.evaluate((sel) => {
                const el = document.querySelector(sel);
                return el ? el.textContent?.trim() : null;
            }, selector);
            if (key && text !== null) {
                automationState.data[key] = text;
                broadcast('AUTOMATION_DATA_UPDATED', { key, value: text, data: automationState.data });
            }
            return text;
        },
        
        // Scrape multiple elements
        scrapeAll: async (selector, key) => {
            const texts = await page.evaluate((sel) => {
                const els = document.querySelectorAll(sel);
                return Array.from(els).map(el => el.textContent?.trim());
            }, selector);
            if (key) {
                automationState.data[key] = texts;
                broadcast('AUTOMATION_DATA_UPDATED', { key, value: texts, data: automationState.data });
            }
            return texts;
        },
        
        // Scrape attribute
        scrapeAttribute: async (selector, attribute, key) => {
            const value = await page.evaluate((sel, attr) => {
                const el = document.querySelector(sel);
                return el ? el.getAttribute(attr) : null;
            }, selector, attribute);
            if (key && value !== null) {
                automationState.data[key] = value;
                broadcast('AUTOMATION_DATA_UPDATED', { key, value, data: automationState.data });
            }
            return value;
        },
        
        // Evaluate custom code
        evaluate: async (code) => {
            return await page.evaluate(code);
        },
        
        captureNetwork: (options) => {
            const { urlPattern, bodyPattern, key } = options;
            if (!key) throw new Error('captureNetwork requires a key');
            automationState.networkCaptures.push({ urlPattern, bodyPattern, key });
            return true;
        },
        
        waitForNetworkCapture: async (key, options = {}) => {
            const timeout = options.timeout || 30000;
            const pollInterval = options.pollInterval || 500;
            const startTime = Date.now();
            
            return new Promise((resolve, reject) => {
                const checkCapture = () => {
                    const captured = automationState.capturedResponses[key];
                    if (captured !== undefined) {
                        resolve(captured);
                        return;
                    }
                    
                    if (Date.now() - startTime > timeout) {
                        reject(new Error(`Timeout waiting for network capture: ${key}`));
                        return;
                    }
                    
                    setTimeout(checkCapture, pollInterval);
                };
                checkCapture();
            });
        },
        
        getCapturedResponse: (key) => {
            return automationState.capturedResponses[key];
        },
        
        clearNetworkCaptures: () => {
            automationState.networkCaptures = [];
            automationState.capturedResponses = {};
        },
        
        // Switch to user-input mode
        // condition: async function that returns true when automation should resume
        // pollInterval: how often to check condition (default 500ms)
        promptUser: async (message, condition, pollInterval = 500) => {
            automationState.prompt = message;
            setAutomationMode('user-input', { prompt: message });
            
            // Poll the condition until it returns true
            return new Promise((resolve) => {
                const checkCondition = async () => {
                    try {
                        const result = await condition();
                        if (result === true) {
                            automationState.prompt = null;
                            setAutomationMode('automation');
                            resolve('condition-met');
                        } else {
                            setTimeout(checkCondition, pollInterval);
                        }
                    } catch (err) {
                        // Condition errored, keep polling
                        setTimeout(checkCondition, pollInterval);
                    }
                };
                checkCondition();
            });
        },
        
        // Get current URL
        url: () => page.url(),
        
        // Check if element exists
        exists: async (selector) => {
            const el = await page.$(selector);
            return el !== null;
        },
        
        // Access persistent data
        get data() {
            return automationState.data;
        },
        
        // Set data directly
        setData: (key, value) => {
            automationState.data[key] = value;
            broadcast('AUTOMATION_DATA_UPDATED', { key, value, data: automationState.data });
        }
    };
}

// Start automation script
app.post('/api/automation/start', async (req, res) => {
    if (!currentSession.page) {
        return res.status(503).json({ error: 'No active browser session' });
    }
    
    if (automationState.isRunning) {
        return res.status(400).json({ error: 'Automation already running' });
    }
    
    const { script } = req.body;
    if (!script || typeof script !== 'string') {
        return res.status(400).json({ error: 'Script is required' });
    }
    
    const scriptId = `script_${Date.now()}`;
    
    automationState = {
        isRunning: true,
        mode: 'automation',
        data: {},
        scriptId,
        prompt: null,
        error: null,
        networkCaptures: [],
        capturedResponses: {},
        pendingRequests: {},
        continueResolver: null,
        lastCursorX: 0,
        lastCursorY: 0
    };
    
    broadcast('AUTOMATION_MODE_CHANGED', { mode: 'automation', scriptId });
    
    // Run script in background
    (async () => {
        try {
            const playwright = createPlaywrightAPI();
            
            // Create async function from script and execute it
            const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
            const scriptFn = new AsyncFunction('playwright', script);
            
            const result = await scriptFn(playwright);
            
            // Script completed successfully
            automationState.isRunning = false;
            automationState.mode = 'idle';
            
            broadcast('AUTOMATION_COMPLETE', {
                scriptId,
                success: true,
                data: automationState.data,
                result
            });
            
            console.log(`Automation ${scriptId} completed successfully`);
        } catch (error) {
            automationState.isRunning = false;
            automationState.mode = 'idle';
            automationState.error = error.message;
            
            broadcast('AUTOMATION_COMPLETE', {
                scriptId,
                success: false,
                error: error.message,
                data: automationState.data
            });
            
            console.error(`Automation ${scriptId} failed:`, error.message);
        }
    })();
    
    res.json({ success: true, scriptId });
});

// Continue automation after user-input mode
app.post('/api/automation/continue', (req, res) => {
    if (automationState.mode !== 'user-input') {
        return res.status(400).json({ error: 'Not in user-input mode' });
    }
    
    if (automationState.continueResolver) {
        automationState.continueResolver();
        automationState.continueResolver = null;
    }
    
    automationState.prompt = null;
    setAutomationMode('automation');
    
    res.json({ success: true });
});

// Stop automation
app.post('/api/automation/stop', (req, res) => {
    // Always reset state to avoid stuck automation
    const wasRunning = automationState.isRunning;
    
    automationState.isRunning = false;
    automationState.mode = 'idle';
    automationState.scriptId = null;
    automationState.prompt = null;
    automationState.error = wasRunning ? 'Stopped by user' : null;
    
    if (automationState.continueResolver) {
        automationState.continueResolver('stopped');
        automationState.continueResolver = null;
    }
    
    broadcast('AUTOMATION_MODE_CHANGED', { mode: 'idle', stopped: true });
    broadcast('AUTOMATION_COMPLETE', { success: false, error: 'Stopped by user', data: automationState.data });
    
    res.json({ success: true, data: automationState.data });
});

// ==================== END AUTOMATION API ====================

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
        
        // Set up proxy if configured
        if (PROXY_ENABLED) {
            await setupProxy(currentSession.page);
        }
        
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
            automationState.pendingRequests[event.requestId] = {
                url: event.request.url,
                method: event.request.method,
                postData: event.request.postData || ''
            };
            
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
            delete automationState.pendingRequests[event.requestId];
            broadcast('NETWORK_FAILED', {
                sessionId,
                requestId: event.requestId,
                errorText: event.errorText,
                timestamp: Date.now()
            });
        });

        client.on('Network.loadingFinished', async (event) => {
            const requestInfo = automationState.pendingRequests[event.requestId];
            delete automationState.pendingRequests[event.requestId];
            
            try {
                const result = await client.send('Network.getResponseBody', { requestId: event.requestId });
                responseBodyCache.set(event.requestId, result);
                if (responseBodyCache.size > MAX_CACHE_SIZE) {
                    const firstKey = responseBodyCache.keys().next().value;
                    responseBodyCache.delete(firstKey);
                }
                
                if (requestInfo && automationState.networkCaptures.length > 0) {
                    const responseBody = result.base64Encoded 
                        ? Buffer.from(result.body, 'base64').toString('utf-8')
                        : result.body;
                    checkNetworkCapture(requestInfo.url, requestInfo.postData, responseBody);
                }
            } catch (e) {
                // Body may not be available for all requests
            }
        });
    } catch (err) {
        cdpConnected = false;
    }
}

async function setupProxy(page) {
    if (!PROXY_ENABLED) return;
    
    try {
        const client = currentSession.cdpClient || await page.context().newCDPSession(page);
        
        // Enable Fetch with auth request handling
        await client.send('Fetch.enable', {
            handleAuthRequests: true,
            patterns: [{ urlPattern: '*' }]
        });
        
        // Handle auth challenges from proxy
        client.on('Fetch.authRequired', async (event) => {
            const { requestId, authChallenge } = event;
            
            if (authChallenge.source === 'Proxy') {
                await client.send('Fetch.continueWithAuth', {
                    requestId,
                    authChallengeResponse: {
                        response: 'ProvideCredentials',
                        username: PROXY_USERNAME,
                        password: PROXY_PASSWORD
                    }
                });
            } else {
                // Not a proxy auth challenge, cancel
                await client.send('Fetch.continueWithAuth', {
                    requestId,
                    authChallengeResponse: { response: 'CancelAuth' }
                });
            }
        });
        
        // Continue all requests (they'll be routed through proxy via Chrome args)
        client.on('Fetch.requestPaused', async (event) => {
            await client.send('Fetch.continueRequest', { requestId: event.requestId });
        });
        
        if (!currentSession.cdpClient) {
            currentSession.cdpClient = client;
        }
        
        console.log(`Proxy authentication handler configured for ${PROXY_SERVER}`);
    } catch (err) {
        console.error('Failed to setup proxy authentication:', err.message);
    }
}

server.listen(PORT, () => {
    if (PROXY_ENABLED) {
        console.log(`Proxy enabled: ${PROXY_SERVER} (user: ${PROXY_USERNAME.substring(0, 20)}...)`);
    } else {
        console.log('Proxy: disabled (set PROXY_SERVER, PROXY_USERNAME, PROXY_PASSWORD to enable)');
    }
    connectToBrowser();
});
