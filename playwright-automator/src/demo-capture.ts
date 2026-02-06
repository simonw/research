#!/usr/bin/env npx tsx
/**
 * Demo script — Runs the full pipeline and captures screenshots for documentation.
 *
 * 1. Starts the test server
 * 2. Opens browser, records login + navigation with screenshots
 * 3. Closes browser, analyzes HAR
 * 4. Shows the analysis summary
 * 5. Saves all screenshots to screenshots-for-pr/
 */

import { chromium } from 'playwright';
import { createServer, type Server } from 'node:http';
import { readFileSync, writeFileSync, existsSync, mkdirSync, copyFileSync } from 'node:fs';
import { join } from 'node:path';
import { analyzeHar, prepareHarSummaryForLLM } from './har-analyzer.js';
import type { RecordingSession } from './types.js';

const PORT = 3870;
const BASE = `http://localhost:${PORT}`;
const SCREENSHOT_DIR = join(process.cwd(), 'screenshots-for-pr');
const RUN_DIR = join(process.cwd(), 'runs', `demo-${Date.now()}`);

mkdirSync(SCREENSHOT_DIR, { recursive: true });
mkdirSync(RUN_DIR, { recursive: true });

const MESSAGES_DATA = {
  messages: [
    { id: 1, from: { id: 1, name: 'Alice Johnson', avatar: '👩' }, to: 'you', text: 'Hey! Are you coming to the meeting tomorrow?', timestamp: '2025-01-15T10:30:00Z', read: true },
    { id: 2, from: { id: 1, name: 'Alice Johnson', avatar: '👩' }, to: 'you', text: 'I have the presentation slides ready', timestamp: '2025-01-15T10:31:00Z', read: true },
    { id: 3, from: { id: 2, name: 'Bob Smith', avatar: '👨' }, to: 'you', text: 'Can you review my PR? It\'s the auth refactor one.', timestamp: '2025-01-15T11:00:00Z', read: false },
    { id: 4, from: { id: 3, name: 'Carol Williams', avatar: '👩‍💼' }, to: 'you', text: 'The deployment to staging is complete!', timestamp: '2025-01-15T12:00:00Z', read: false },
    { id: 5, from: { id: 3, name: 'Carol Williams', avatar: '👩‍💼' }, to: 'you', text: 'Let me know if you see any issues', timestamp: '2025-01-15T12:01:00Z', read: false },
    { id: 6, from: { id: 4, name: 'Dave Brown', avatar: '🧑‍💻' }, to: 'you', text: 'Quick question about the API design', timestamp: '2025-01-15T14:00:00Z', read: true },
    { id: 7, from: { id: 4, name: 'Dave Brown', avatar: '🧑‍💻' }, to: 'you', text: 'Should we use REST or GraphQL for the new service?', timestamp: '2025-01-15T14:02:00Z', read: true },
  ],
  pagination: { page: 1, total: 7, hasMore: false },
};

const THREADS_DATA = {
  threads: [
    { id: 't1', participant: { id: 1, name: 'Alice Johnson' }, lastMessage: { text: 'I have the presentation slides ready' }, unreadCount: 0, messageCount: 2 },
    { id: 't2', participant: { id: 2, name: 'Bob Smith' }, lastMessage: { text: 'Can you review my PR?' }, unreadCount: 1, messageCount: 1 },
    { id: 't3', participant: { id: 3, name: 'Carol Williams' }, lastMessage: { text: 'Let me know if you see any issues' }, unreadCount: 2, messageCount: 2 },
    { id: 't4', participant: { id: 4, name: 'Dave Brown' }, lastMessage: { text: 'Should we use REST or GraphQL?' }, unreadCount: 0, messageCount: 2 },
  ],
  pagination: { page: 1, total: 4 },
};

function startServer(): Promise<Server> {
  return new Promise((resolve) => {
    const srv = createServer((req, res) => {
      const url = new URL(req.url || '/', BASE);
      const p = url.pathname;
      const method = req.method || 'GET';

      const cookieHeader = req.headers.cookie || '';
      const cookies: Record<string, string> = {};
      for (const pair of cookieHeader.split(';')) {
        const [name, ...rest] = pair.trim().split('=');
        if (name) cookies[name] = rest.join('=');
      }
      const isAuth = cookies['session'] === 'demo-session-token';

      if (method === 'OPTIONS') {
        res.writeHead(204, { 'Access-Control-Allow-Origin': '*' });
        res.end();
        return;
      }

      // Login page
      if ((p === '/' || p === '/login') && !isAuth) {
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
        res.end(`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>TestApp - Login</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #f0f2f5; display: flex; justify-content: center; align-items: center; min-height: 100vh; }
    .login-card { background: white; border-radius: 12px; padding: 40px; width: 380px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
    h1 { text-align: center; color: #1a1a2e; margin-bottom: 8px; font-size: 28px; }
    .subtitle { text-align: center; color: #666; margin-bottom: 24px; font-size: 14px; }
    .form-group { margin-bottom: 16px; }
    label { display: block; margin-bottom: 4px; font-size: 14px; color: #333; font-weight: 500; }
    input { width: 100%; padding: 12px; border: 1px solid #ddd; border-radius: 8px; font-size: 16px; transition: border-color 0.2s; }
    input:focus { outline: none; border-color: #4361ee; box-shadow: 0 0 0 3px rgba(67,97,238,0.1); }
    button { width: 100%; padding: 14px; background: #4361ee; color: white; border: none; border-radius: 8px; font-size: 16px; cursor: pointer; font-weight: 600; transition: background 0.2s; }
    button:hover { background: #3651d4; }
    .hint { text-align: center; color: #999; font-size: 13px; margin-top: 16px; }
    .error { color: #e63946; text-align: center; font-size: 14px; margin-top: 8px; display: none; }
  </style>
</head>
<body>
  <div class="login-card">
    <h1>TestApp</h1>
    <p class="subtitle">Sign in to access your messages</p>
    <form id="loginForm">
      <div class="form-group">
        <label for="username">Username</label>
        <input type="text" id="username" name="username" placeholder="Enter your username" required>
      </div>
      <div class="form-group">
        <label for="password">Password</label>
        <input type="password" id="password" name="password" placeholder="Enter your password" required>
      </div>
      <button type="submit" id="loginBtn">Sign In</button>
    </form>
    <div id="error" class="error"></div>
    <p class="hint">Demo: testuser / testpass</p>
  </div>
  <script>
    document.getElementById('loginForm').addEventListener('submit', async (e) => {
      e.preventDefault();
      const username = document.getElementById('username').value;
      const password = document.getElementById('password').value;
      const errEl = document.getElementById('error');
      errEl.style.display = 'none';
      try {
        const res = await fetch('/api/login', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ username, password }),
        });
        const data = await res.json();
        if (data.success) {
          window.location.href = '/dashboard';
        } else {
          errEl.textContent = data.error || 'Login failed';
          errEl.style.display = 'block';
        }
      } catch (err) {
        errEl.textContent = 'Connection error';
        errEl.style.display = 'block';
      }
    });
  </script>
</body>
</html>`);
        return;
      }

      // Login API
      if (p === '/api/login' && method === 'POST') {
        let body = '';
        req.on('data', (c: Buffer) => body += c.toString());
        req.on('end', () => {
          try {
            const data = JSON.parse(body);
            if (data.username === 'testuser' && data.password === 'testpass') {
              res.writeHead(200, { 'Content-Type': 'application/json', 'Set-Cookie': 'session=demo-session-token; Path=/' });
              res.end(JSON.stringify({ success: true, user: { name: 'Test User', id: 100 } }));
            } else {
              res.writeHead(401, { 'Content-Type': 'application/json' });
              res.end(JSON.stringify({ success: false, error: 'Invalid credentials' }));
            }
          } catch { res.writeHead(400); res.end('{"error":"bad json"}'); }
        });
        return;
      }

      if (!isAuth && p.startsWith('/api/')) {
        res.writeHead(401, { 'Content-Type': 'application/json' });
        res.end('{"error":"Unauthorized"}');
        return;
      }
      if (!isAuth) {
        res.writeHead(302, { Location: '/login' });
        res.end();
        return;
      }

      // Dashboard
      if (p === '/' || p === '/dashboard') {
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
        res.end(`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>TestApp - Dashboard</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #f0f2f5; }
    nav { background: white; padding: 12px 24px; display: flex; justify-content: space-between; align-items: center; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }
    nav h1 { font-size: 20px; color: #1a1a2e; }
    nav a { color: #4361ee; text-decoration: none; margin-left: 20px; font-weight: 500; }
    nav a:hover { text-decoration: underline; }
    .container { max-width: 900px; margin: 24px auto; padding: 0 20px; }
    .stats { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; margin-bottom: 24px; }
    .stat-card { background: white; border-radius: 12px; padding: 24px; text-align: center; box-shadow: 0 1px 3px rgba(0,0,0,0.08); }
    .stat-card h3 { color: #666; font-size: 13px; text-transform: uppercase; letter-spacing: 1px; }
    .stat-card .value { font-size: 36px; font-weight: 700; color: #1a1a2e; margin: 8px 0; }
    .stat-card .label { font-size: 13px; color: #999; }
    .section { background: white; border-radius: 12px; padding: 24px; box-shadow: 0 1px 3px rgba(0,0,0,0.08); }
    .section h2 { font-size: 18px; color: #1a1a2e; margin-bottom: 16px; }
    .thread { display: flex; align-items: center; padding: 12px; border-radius: 8px; cursor: pointer; transition: background 0.2s; }
    .thread:hover { background: #f8f9fa; }
    .thread-avatar { width: 44px; height: 44px; border-radius: 50%; background: #e8eaf6; display: flex; align-items: center; justify-content: center; font-size: 20px; margin-right: 12px; flex-shrink: 0; }
    .thread-content { flex: 1; min-width: 0; }
    .thread-name { font-weight: 600; color: #1a1a2e; font-size: 15px; }
    .thread-preview { color: #666; font-size: 14px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .unread-badge { background: #4361ee; color: white; padding: 2px 8px; border-radius: 12px; font-size: 12px; font-weight: 600; }
  </style>
</head>
<body>
  <nav>
    <h1>TestApp</h1>
    <div>
      <a href="/dashboard">Dashboard</a>
      <a href="/messages" id="messagesLink">Messages</a>
      <a href="/api/logout">Logout</a>
    </div>
  </nav>
  <div class="container">
    <div class="stats">
      <div class="stat-card"><h3>Messages</h3><div class="value" id="msgCount">-</div><div class="label">total</div></div>
      <div class="stat-card"><h3>Unread</h3><div class="value" id="unreadCount">-</div><div class="label">new messages</div></div>
      <div class="stat-card"><h3>Contacts</h3><div class="value" id="contactCount">-</div><div class="label">people</div></div>
    </div>
    <div class="section">
      <h2>Recent Conversations</h2>
      <div id="threads">Loading...</div>
    </div>
  </div>
  <script>
    async function loadDashboard() {
      const [statsRes, threadsRes] = await Promise.all([
        fetch('/api/stats'), fetch('/api/threads')
      ]);
      const stats = await statsRes.json();
      const threads = await threadsRes.json();
      document.getElementById('msgCount').textContent = stats.totalMessages;
      document.getElementById('unreadCount').textContent = stats.unreadMessages;
      document.getElementById('contactCount').textContent = stats.totalContacts;
      document.getElementById('threads').innerHTML = threads.threads.map(t =>
        '<div class="thread" onclick="location.href=\\'/messages\\'">' +
        '<div class="thread-avatar">' + (t.participant.name[0]) + '</div>' +
        '<div class="thread-content"><div class="thread-name">' + t.participant.name + '</div>' +
        '<div class="thread-preview">' + t.lastMessage.text + '</div></div>' +
        (t.unreadCount > 0 ? '<span class="unread-badge">' + t.unreadCount + '</span>' : '') +
        '</div>'
      ).join('');
    }
    loadDashboard();
  </script>
</body>
</html>`);
        return;
      }

      // Messages page
      if (p === '/messages') {
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
        res.end(`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>TestApp - Messages</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #f0f2f5; }
    nav { background: white; padding: 12px 24px; display: flex; justify-content: space-between; align-items: center; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }
    nav h1 { font-size: 20px; color: #1a1a2e; }
    nav a { color: #4361ee; text-decoration: none; margin-left: 20px; font-weight: 500; }
    .container { max-width: 900px; margin: 24px auto; padding: 0 20px; }
    .page-header { margin-bottom: 20px; }
    .page-header h2 { font-size: 22px; color: #1a1a2e; }
    .page-header p { color: #666; font-size: 14px; margin-top: 4px; }
    .messages-list { background: white; border-radius: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08); overflow: hidden; }
    .message { padding: 16px 20px; border-bottom: 1px solid #f0f0f0; display: flex; align-items: flex-start; }
    .message:last-child { border-bottom: none; }
    .message.unread { background: #f8f9ff; }
    .msg-avatar { width: 40px; height: 40px; border-radius: 50%; background: #e8eaf6; display: flex; align-items: center; justify-content: center; font-weight: 600; color: #4361ee; margin-right: 12px; flex-shrink: 0; }
    .msg-content { flex: 1; }
    .msg-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px; }
    .msg-sender { font-weight: 600; color: #1a1a2e; font-size: 15px; }
    .msg-time { color: #999; font-size: 13px; }
    .msg-text { color: #444; font-size: 14px; line-height: 1.4; }
    .unread-dot { width: 8px; height: 8px; border-radius: 50%; background: #4361ee; margin-left: 8px; flex-shrink: 0; align-self: center; }
  </style>
</head>
<body>
  <nav>
    <h1>TestApp</h1>
    <div>
      <a href="/dashboard">Dashboard</a>
      <a href="/messages">Messages</a>
      <a href="/api/logout">Logout</a>
    </div>
  </nav>
  <div class="container">
    <div class="page-header">
      <h2>All Messages</h2>
      <p id="msgSummary">Loading messages...</p>
    </div>
    <div class="messages-list" id="messagesList">Loading...</div>
  </div>
  <script>
    async function loadMessages() {
      const res = await fetch('/api/messages');
      const data = await res.json();
      document.getElementById('msgSummary').textContent = data.messages.length + ' messages, ' + data.messages.filter(m => !m.read).length + ' unread';
      document.getElementById('messagesList').innerHTML = data.messages.map(m =>
        '<div class="message' + (m.read ? '' : ' unread') + '">' +
        '<div class="msg-avatar">' + m.from.name[0] + '</div>' +
        '<div class="msg-content">' +
        '<div class="msg-header"><span class="msg-sender">' + m.from.name + '</span>' +
        '<span class="msg-time">' + new Date(m.timestamp).toLocaleString() + '</span></div>' +
        '<div class="msg-text">' + m.text + '</div>' +
        '</div>' +
        (m.read ? '' : '<div class="unread-dot"></div>') +
        '</div>'
      ).join('');
    }
    loadMessages();
  </script>
</body>
</html>`);
        return;
      }

      // API endpoints
      if (p === '/api/stats') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ totalMessages: 7, unreadMessages: 3, totalContacts: 4 }));
        return;
      }
      if (p === '/api/threads') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify(THREADS_DATA));
        return;
      }
      if (p === '/api/messages') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify(MESSAGES_DATA));
        return;
      }
      if (p === '/api/users') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ users: [{ id: 1, name: 'Alice Johnson' }, { id: 2, name: 'Bob Smith' }, { id: 3, name: 'Carol Williams' }, { id: 4, name: 'Dave Brown' }] }));
        return;
      }
      if (p === '/api/me') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ user: { id: 100, name: 'Test User', email: 'test@testapp.com' } }));
        return;
      }

      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end('{"error":"not found"}');
    });
    srv.listen(PORT, () => resolve(srv));
  });
}

async function main() {
  console.log('=== Playwright Automator Demo ===\n');

  const server = await startServer();
  console.log(`Test server running on ${BASE}\n`);

  const harPath = join(RUN_DIR, 'recording.har');

  try {
    // Launch browser with HAR recording
    console.log('Step 1: Launching browser with HAR recording...');
    const browser = await chromium.launch({
      headless: true,
      args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu', '--single-process'],
    });
    const context = await browser.newContext({
      recordHar: { path: harPath, mode: 'full' },
      viewport: { width: 1280, height: 800 },
    });
    const page = await context.newPage();

    // Screenshot 1: Login page
    console.log('Step 2: Navigating to login page...');
    await page.goto(BASE, { waitUntil: 'networkidle', timeout: 15000 });
    await page.waitForTimeout(500);
    await page.screenshot({ path: join(SCREENSHOT_DIR, '01-login-page.png'), timeout: 10000 });
    console.log('  -> Screenshot: 01-login-page.png');

    // Screenshot 2: Filling login form
    console.log('Step 3: Filling login form...');
    await page.fill('#username', 'testuser', { timeout: 5000 });
    await page.fill('#password', 'testpass', { timeout: 5000 });
    await page.screenshot({ path: join(SCREENSHOT_DIR, '02-login-filled.png'), timeout: 10000 });
    console.log('  -> Screenshot: 02-login-filled.png');

    // Login
    console.log('Step 4: Logging in...');
    await page.click('#loginBtn');
    await page.waitForURL('**/dashboard', { timeout: 10000 });
    await page.waitForTimeout(1500);

    // Screenshot 3: Dashboard
    await page.screenshot({ path: join(SCREENSHOT_DIR, '03-dashboard.png'), timeout: 10000 });
    console.log('  -> Screenshot: 03-dashboard.png');

    // Navigate to messages
    console.log('Step 5: Navigating to messages...');
    await page.click('#messagesLink');
    await page.waitForURL('**/messages', { timeout: 10000 });
    await page.waitForTimeout(1500);

    // Screenshot 4: Messages page
    await page.screenshot({ path: join(SCREENSHOT_DIR, '04-messages-page.png'), timeout: 10000 });
    console.log('  -> Screenshot: 04-messages-page.png');

    // Capture cookies
    const cookies = await context.cookies();
    const cookieMap: Record<string, string> = {};
    for (const c of cookies) cookieMap[c.name] = c.value;

    // Close browser to flush HAR
    console.log('\nStep 6: Closing browser and analyzing HAR...');
    await context.close();
    await browser.close();

    // Analyze HAR
    const harData = JSON.parse(readFileSync(harPath, 'utf-8'));
    const analysis = analyzeHar(harData, BASE);

    // Build session
    const session: RecordingSession = {
      id: `demo-${Date.now()}`,
      url: BASE,
      description: 'Get all messages from the test app',
      startTime: new Date().toISOString(),
      endTime: new Date().toISOString(),
      actions: [
        { type: 'navigate', timestamp: new Date().toISOString(), url: BASE },
        { type: 'type', timestamp: new Date().toISOString(), url: BASE, selector: '#username', value: 'testuser' },
        { type: 'type', timestamp: new Date().toISOString(), url: BASE, selector: '#password', value: '***' },
        { type: 'click', timestamp: new Date().toISOString(), url: BASE, selector: '#loginBtn', tagName: 'button', text: 'Sign In' },
        { type: 'navigate', timestamp: new Date().toISOString(), url: `${BASE}/dashboard` },
        { type: 'click', timestamp: new Date().toISOString(), url: `${BASE}/dashboard`, selector: '#messagesLink', tagName: 'a', text: 'Messages' },
        { type: 'navigate', timestamp: new Date().toISOString(), url: `${BASE}/messages` },
      ],
      apiRequests: analysis.apiRequests,
      cookies: cookieMap,
      authHeaders: analysis.authHeaders,
      authMethod: analysis.authMethod,
      targetDomain: 'localhost',
      harFilePath: harPath,
    };

    // Save session files
    writeFileSync(join(RUN_DIR, 'session.json'), JSON.stringify(session, null, 2));
    writeFileSync(join(RUN_DIR, 'auth.json'), JSON.stringify({
      authHeaders: analysis.authHeaders,
      cookies: cookieMap,
      authMethod: analysis.authMethod,
    }, null, 2));

    // Print analysis
    console.log('\n=== HAR Analysis Results ===');
    console.log(`Total HAR entries: ${harData.log.entries.length}`);
    console.log(`API requests extracted: ${analysis.apiRequests.length}`);
    console.log(`Auth method: ${analysis.authMethod}`);
    console.log(`Cookies captured: ${Object.keys(cookieMap).length}`);
    console.log(`\nAPI Endpoints Captured:`);
    for (const req of analysis.apiRequests) {
      const bodyLen = req.responseBody ? req.responseBody.length : 0;
      console.log(`  ${req.method} ${req.path} -> ${req.status} (${bodyLen} bytes response)`);
    }

    // Prepare LLM summary
    const llmSummary = prepareHarSummaryForLLM(analysis.apiRequests);
    writeFileSync(join(RUN_DIR, 'llm-summary.txt'), llmSummary);

    console.log(`\nLLM summary prepared: ${llmSummary.length} chars`);
    console.log(`\nAll files saved to: ${RUN_DIR}`);
    console.log(`Screenshots saved to: ${SCREENSHOT_DIR}`);
    console.log('\n=== Demo Complete ===');

  } finally {
    server.close();
  }
}

main().catch(e => { console.error('Fatal:', e); process.exit(1); });
