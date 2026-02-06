/**
 * Headless E2E Test — Tests the full pipeline without interactive input.
 *
 * Runs: test server + Playwright HAR recording + login flow + HAR analysis
 */

import { chromium } from 'playwright';
import { createServer, type Server } from 'node:http';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { analyzeHar, prepareHarSummaryForLLM } from './har-analyzer.js';
import type { RecordingSession } from './types.js';

const PORT = 3860;
const BASE = `http://localhost:${PORT}`;

const MESSAGES_DATA = {
  messages: [
    { id: 1, from: 'Alice', text: 'Meeting tomorrow at 10am', timestamp: '2025-01-15T10:30:00Z', read: true },
    { id: 2, from: 'Bob', text: 'Can you review my PR?', timestamp: '2025-01-15T11:00:00Z', read: false },
    { id: 3, from: 'Carol', text: 'Deployment complete!', timestamp: '2025-01-15T12:00:00Z', read: false },
  ],
  pagination: { page: 1, total: 3, hasMore: false },
};

function startServer(): Promise<Server> {
  return new Promise((resolve) => {
    const srv = createServer((req, res) => {
      const p = new URL(req.url || '/', BASE).pathname;

      if (p === '/' || p === '/login') {
        res.writeHead(200, { 'Content-Type': 'text/html' });
        res.end(`<html><body>
          <h1>TestApp Login</h1>
          <input type="text" id="username" placeholder="Username">
          <input type="password" id="password" placeholder="Password">
          <button id="loginBtn">Login</button>
          <script>
            document.getElementById('loginBtn').onclick = async () => {
              const r = await fetch('/api/login', {method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({username:document.getElementById('username').value, password:document.getElementById('password').value})});
              const d = await r.json();
              if (d.success) window.location.href = '/dashboard';
            };
          </script>
        </body></html>`);
        return;
      }

      if (p === '/api/login' && req.method === 'POST') {
        let body = '';
        req.on('data', (c: Buffer) => body += c.toString());
        req.on('end', () => {
          res.writeHead(200, {
            'Content-Type': 'application/json',
            'Set-Cookie': 'session=test-token-123; Path=/',
          });
          res.end(JSON.stringify({ success: true, user: { name: 'Test User', id: 100 } }));
        });
        return;
      }

      if (p === '/dashboard') {
        res.writeHead(200, { 'Content-Type': 'text/html' });
        res.end(`<html><body><h1>Dashboard</h1><a id="messagesLink" href="/messages">Messages</a>
          <script>fetch('/api/stats').then(r=>r.json());</script></body></html>`);
        return;
      }

      if (p === '/messages') {
        res.writeHead(200, { 'Content-Type': 'text/html' });
        res.end(`<html><body><h1>Messages</h1><div id="list"></div>
          <script>
            Promise.all([fetch('/api/threads').then(r=>r.json()), fetch('/api/messages').then(r=>r.json())])
              .then(([t,m]) => { document.getElementById('list').innerHTML = m.messages.map(x=>'<div>'+x.from+': '+x.text+'</div>').join(''); });
          </script></body></html>`);
        return;
      }

      if (p === '/api/stats') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ totalMessages: 3, unreadMessages: 2, totalContacts: 3 }));
        return;
      }

      if (p === '/api/threads') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          threads: [
            { id: 't1', participant: { id: 1, name: 'Alice' }, lastMessage: { text: 'Meeting tomorrow' }, unreadCount: 0 },
            { id: 't2', participant: { id: 2, name: 'Bob' }, lastMessage: { text: 'PR review?' }, unreadCount: 1 },
            { id: 't3', participant: { id: 3, name: 'Carol' }, lastMessage: { text: 'Deploy done' }, unreadCount: 1 },
          ],
          pagination: { page: 1, total: 3 },
        }));
        return;
      }

      if (p === '/api/messages') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify(MESSAGES_DATA));
        return;
      }

      if (p === '/api/users') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ users: [{ id: 1, name: 'Alice' }, { id: 2, name: 'Bob' }, { id: 3, name: 'Carol' }] }));
        return;
      }

      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end('{"error":"not found"}');
    });
    srv.listen(PORT, () => resolve(srv));
  });
}

async function main() {
  console.log('=== Playwright Automator E2E Test ===\n');

  const testDir = join(process.cwd(), 'runs', `e2e-test-${Date.now()}`);
  mkdirSync(testDir, { recursive: true });
  const harPath = join(testDir, 'recording.har');

  let passed = 0;
  let failed = 0;
  const check = (ok: boolean, msg: string) => {
    if (ok) { console.log(`  PASS: ${msg}`); passed++; }
    else { console.log(`  FAIL: ${msg}`); failed++; }
  };

  // 1. Start server
  console.log('1. Starting test server...');
  const server = await startServer();
  console.log(`   Listening on ${BASE}\n`);

  try {
    // 2. Launch browser
    console.log('2. Recording browser session...');
    const browser = await chromium.launch({
      headless: true,
      args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu', '--single-process'],
    });
    const context = await browser.newContext({
      recordHar: { path: harPath, mode: 'full' },
      viewport: { width: 1280, height: 800 },
    });
    const page = await context.newPage();
    check(true, 'Browser launched with HAR recording');

    // 3. Login flow
    await page.goto(BASE, { waitUntil: 'domcontentloaded', timeout: 10000 });
    check(true, 'Login page loaded');

    await page.fill('#username', 'testuser', { timeout: 5000 });
    await page.fill('#password', 'testpass', { timeout: 5000 });
    await page.click('#loginBtn');
    check(true, 'Login form submitted');

    await page.waitForURL('**/dashboard', { timeout: 10000 });
    check(page.url().includes('/dashboard'), `Dashboard reached: ${page.url()}`);

    // 4. Navigate to messages
    await page.click('#messagesLink');
    await page.waitForURL('**/messages', { timeout: 10000 });
    await page.waitForTimeout(1500);
    check(page.url().includes('/messages'), `Messages page: ${page.url()}`);

    // 5. Capture cookies
    const cookies = await context.cookies();
    const cookieMap: Record<string, string> = {};
    for (const c of cookies) cookieMap[c.name] = c.value;
    check(cookieMap['session'] === 'test-token-123', 'Session cookie captured');

    // 6. Close browser (flush HAR)
    await context.close();
    await browser.close();
    check(true, 'Browser closed');

    // 7. Analyze HAR
    console.log('\n3. Analyzing HAR recording...');
    check(existsSync(harPath), 'HAR file exists');

    const harData = JSON.parse(readFileSync(harPath, 'utf-8'));
    check(harData.log.entries.length > 0, `HAR has ${harData.log.entries.length} entries`);

    const analysis = analyzeHar(harData, BASE);
    check(analysis.apiRequests.length > 0, `Extracted ${analysis.apiRequests.length} API requests`);

    const endpoints = analysis.apiRequests.map(r => `${r.method} ${r.path}`);
    check(endpoints.some(e => e.includes('/api/login')), 'POST /api/login captured');
    check(endpoints.some(e => e.includes('/api/stats')), 'GET /api/stats captured');
    check(endpoints.some(e => e.includes('/api/threads')), 'GET /api/threads captured');
    check(endpoints.some(e => e.includes('/api/messages')), 'GET /api/messages captured');

    // Check response bodies are captured
    const messagesReq = analysis.apiRequests.find(r => r.path.includes('/api/messages'));
    if (messagesReq?.responseBody) {
      try {
        const body = JSON.parse(messagesReq.responseBody);
        check(Array.isArray(body.messages), `Messages response parsed: ${body.messages?.length} messages`);
      } catch {
        check(false, 'Messages response is valid JSON');
      }
    } else {
      check(false, 'Messages response body captured');
    }

    // 8. Test LLM summary
    console.log('\n4. Testing LLM summary...');
    const summary = prepareHarSummaryForLLM(analysis.apiRequests);
    check(summary.length > 50, `LLM summary: ${summary.length} chars`);
    check(summary.includes('/api/'), 'Summary includes API paths');

    // 9. Save session data
    console.log('\n5. Saving session data...');
    const session: RecordingSession = {
      id: `e2e-${Date.now()}`,
      url: BASE,
      description: 'Get all messages from the test app',
      startTime: new Date().toISOString(),
      endTime: new Date().toISOString(),
      actions: [
        { type: 'navigate', timestamp: new Date().toISOString(), url: BASE },
        { type: 'type', timestamp: new Date().toISOString(), url: BASE, selector: '#username', value: 'testuser' },
        { type: 'type', timestamp: new Date().toISOString(), url: BASE, selector: '#password', value: '***' },
        { type: 'click', timestamp: new Date().toISOString(), url: BASE, selector: '#loginBtn', tagName: 'button', text: 'Login' },
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

    writeFileSync(join(testDir, 'session.json'), JSON.stringify(session, null, 2));
    writeFileSync(join(testDir, 'auth.json'), JSON.stringify({ authHeaders: analysis.authHeaders, cookies: cookieMap, authMethod: analysis.authMethod }, null, 2));
    writeFileSync(join(testDir, 'actions.json'), JSON.stringify(session.actions, null, 2));

    check(existsSync(join(testDir, 'session.json')), 'session.json saved');
    check(existsSync(join(testDir, 'auth.json')), 'auth.json saved');

    // 10. Test Gemini (if key available)
    const geminiKey = process.env.GEMINI_API_KEY;
    if (geminiKey) {
      console.log('\n6. Testing Gemini script generation...');
      try {
        const { generateScript } = await import('./gemini.js');
        const result = await generateScript(session, geminiKey);
        check(result.script.length > 100, `Script generated: ${result.script.length} chars`);
        check(result.script.includes('playwright') || result.script.includes('chromium'), 'Script imports playwright');
        writeFileSync(join(testDir, 'automation.ts'), result.script);
        writeFileSync(join(testDir, 'generation-info.json'), JSON.stringify({
          strategy: result.strategy,
          apiEndpoints: result.apiEndpoints,
        }, null, 2));
        check(existsSync(join(testDir, 'automation.ts')), 'automation.ts saved');
        console.log(`   Strategy: ${result.strategy}`);
        console.log(`   Endpoints: ${result.apiEndpoints.join(', ') || 'none detected'}`);
      } catch (err: any) {
        console.log(`   Gemini error: ${err.message}`);
        check(false, 'Gemini script generation');
      }
    } else {
      console.log('\n6. Skipping Gemini (no GEMINI_API_KEY)');
    }

  } finally {
    server.close();
  }

  console.log(`\n${'='.repeat(40)}`);
  console.log(`RESULTS: ${passed} passed, ${failed} failed`);
  console.log(`Output: ${testDir}`);
  console.log('='.repeat(40));

  process.exit(failed > 0 ? 1 : 0);
}

main().catch(e => { console.error('Fatal:', e); process.exit(1); });
