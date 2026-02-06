#!/usr/bin/env npx tsx
/**
 * End-to-end test for Playwright Automator.
 *
 * Tests the full pipeline:
 * 1. Start test server
 * 2. Launch Playwright with HAR recording
 * 3. Automate login + navigation (simulating user actions)
 * 4. Stop recording and analyze HAR
 * 5. Generate a script using Gemini (if API key provided)
 * 6. Verify all output files exist and are valid
 *
 * Usage:
 *   npx tsx src/e2e-test.ts                    # Test recording only
 *   GEMINI_API_KEY=xxx npx tsx src/e2e-test.ts # Full test with script generation
 */

import { chromium } from 'playwright';
import { createServer, type Server } from 'node:http';
import { mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { analyzeHar, prepareHarSummaryForLLM } from './har-analyzer.js';
import { generateScript } from './gemini.js';
import type { RecordingSession } from './types.js';

const TEST_PORT = 3848;
const TEST_URL = `http://localhost:${TEST_PORT}`;
const TEST_DIR = join(process.cwd(), 'runs', `e2e-test-${Date.now()}`);

// ── Test Server (inline, same as test-server.ts) ──

const AUTH_TOKEN = 'test-session-token-12345';

const users = [
  { id: 1, name: 'Alice Johnson', avatar: 'A' },
  { id: 2, name: 'Bob Smith', avatar: 'B' },
  { id: 3, name: 'Carol Williams', avatar: 'C' },
];

const messages = [
  { id: 1, from: users[0], to: 'you', text: 'Hey! Meeting tomorrow at 10am.', timestamp: '2025-01-15T10:30:00Z', read: true },
  { id: 2, from: users[0], to: 'you', text: 'Bring the presentation slides', timestamp: '2025-01-15T10:31:00Z', read: true },
  { id: 3, from: users[1], to: 'you', text: 'Can you review my PR?', timestamp: '2025-01-15T11:00:00Z', read: false },
  { id: 4, from: users[2], to: 'you', text: 'Deployment to staging complete!', timestamp: '2025-01-15T12:00:00Z', read: false },
  { id: 5, from: users[2], to: 'you', text: 'Let me know if any issues', timestamp: '2025-01-15T12:01:00Z', read: false },
];

const threads = [
  { id: 't1', participant: users[0], lastMessage: messages[1], unreadCount: 0, messageCount: 2 },
  { id: 't2', participant: users[1], lastMessage: messages[2], unreadCount: 1, messageCount: 1 },
  { id: 't3', participant: users[2], lastMessage: messages[4], unreadCount: 2, messageCount: 2 },
];

function startTestServer(): Promise<Server> {
  return new Promise((resolve) => {
    const server = createServer(async (req, res) => {
      const url = new URL(req.url || '/', `http://localhost:${TEST_PORT}`);
      const path = url.pathname;
      const method = req.method || 'GET';

      // Parse cookies
      const cookieHeader = req.headers.cookie || '';
      const cookies: Record<string, string> = {};
      for (const pair of cookieHeader.split(';')) {
        const [name, ...rest] = pair.trim().split('=');
        if (name) cookies[name] = rest.join('=');
      }
      const isAuth = cookies['session'] === AUTH_TOKEN;

      if (method === 'OPTIONS') {
        res.writeHead(204, { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Methods': 'GET, POST', 'Access-Control-Allow-Headers': 'Content-Type' });
        res.end();
        return;
      }

      // Login page
      if ((path === '/' || path === '/login') && method === 'GET') {
        if (isAuth) { res.writeHead(302, { Location: '/dashboard' }); res.end(); return; }
        res.writeHead(200, { 'Content-Type': 'text/html' });
        res.end(`<html><body>
          <h1>TestApp Login</h1>
          <form id="loginForm">
            <input type="text" name="username" id="username" placeholder="Username">
            <input type="password" name="password" id="password" placeholder="Password">
            <button type="submit" id="loginBtn">Log In</button>
          </form>
          <script>
            document.getElementById('loginForm').addEventListener('submit', async (e) => {
              e.preventDefault();
              const res = await fetch('/api/login', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ username: document.getElementById('username').value, password: document.getElementById('password').value }),
              });
              const data = await res.json();
              if (data.success) window.location.href = '/dashboard';
            });
          </script>
        </body></html>`);
        return;
      }

      // Login API
      if (path === '/api/login' && method === 'POST') {
        let body = '';
        req.on('data', (c: Buffer) => body += c.toString());
        req.on('end', () => {
          try {
            const data = JSON.parse(body);
            if (data.username === 'testuser' && data.password === 'testpass') {
              res.writeHead(200, { 'Content-Type': 'application/json', 'Set-Cookie': `session=${AUTH_TOKEN}; Path=/` });
              res.end(JSON.stringify({ success: true, user: { name: 'Test User', id: 100 } }));
            } else {
              res.writeHead(401, { 'Content-Type': 'application/json' });
              res.end(JSON.stringify({ success: false, error: 'Bad credentials' }));
            }
          } catch { res.writeHead(400, { 'Content-Type': 'application/json' }); res.end(JSON.stringify({ error: 'bad json' })); }
        });
        return;
      }

      // Auth check for other routes
      if (!isAuth) {
        if (path.startsWith('/api/')) { res.writeHead(401, { 'Content-Type': 'application/json' }); res.end(JSON.stringify({ error: 'Unauthorized' })); }
        else { res.writeHead(302, { Location: '/login' }); res.end(); }
        return;
      }

      // Dashboard
      if (path === '/dashboard') {
        res.writeHead(200, { 'Content-Type': 'text/html' });
        res.end(`<html><body><h1>Dashboard</h1><a href="/messages" id="messagesLink">Messages</a>
          <script>
            fetch('/api/stats').then(r=>r.json()).then(d=>{ document.title='Dashboard - '+d.totalMessages+' messages'; });
          </script>
        </body></html>`);
        return;
      }

      // Messages page
      if (path === '/messages') {
        res.writeHead(200, { 'Content-Type': 'text/html' });
        res.end(`<html><body><h1>Messages</h1><div id="threads">Loading...</div>
          <script>
            fetch('/api/threads').then(r=>r.json()).then(data=>{
              document.getElementById('threads').innerHTML = data.threads.map(t=>
                '<div class="thread" data-id="'+t.id+'"><b>'+t.participant.name+'</b>: '+t.lastMessage.text+'</div>'
              ).join('');
            });
            fetch('/api/messages').then(r=>r.json()).then(data=>{
              console.log('All messages loaded:', data.messages.length);
            });
          </script>
        </body></html>`);
        return;
      }

      // API: stats
      if (path === '/api/stats') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ totalMessages: messages.length, unreadMessages: messages.filter(m => !m.read).length, totalContacts: users.length }));
        return;
      }

      // API: threads
      if (path === '/api/threads') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ threads, pagination: { page: 1, total: threads.length } }));
        return;
      }

      // API: thread messages
      const threadMatch = path.match(/^\/api\/threads\/(t\d+)\/messages$/);
      if (threadMatch) {
        const t = threads.find(t => t.id === threadMatch[1]);
        if (!t) { res.writeHead(404, { 'Content-Type': 'application/json' }); res.end(JSON.stringify({ error: 'not found' })); return; }
        const threadMsgs = messages.filter(m => m.from.id === t.participant.id);
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ threadId: t.id, participant: t.participant, messages: threadMsgs }));
        return;
      }

      // API: all messages
      if (path === '/api/messages') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ messages, pagination: { page: 1, total: messages.length } }));
        return;
      }

      // API: users
      if (path === '/api/users') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ users }));
        return;
      }

      // API: me
      if (path === '/api/me') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ user: { id: 100, name: 'Test User' } }));
        return;
      }

      res.writeHead(404);
      res.end('Not found');
    });

    server.listen(TEST_PORT, () => resolve(server));
  });
}

// ── Main Test ──

async function runTest() {
  console.log('╔══════════════════════════════════════════════════╗');
  console.log('║     Playwright Automator — E2E Test             ║');
  console.log('╚══════════════════════════════════════════════════╝\n');

  const geminiKey = process.env.GEMINI_API_KEY;
  let passed = 0;
  let failed = 0;

  function assert(condition: boolean, msg: string) {
    if (condition) {
      console.log(`  ✅ ${msg}`);
      passed++;
    } else {
      console.log(`  ❌ ${msg}`);
      failed++;
    }
  }

  // Setup
  mkdirSync(TEST_DIR, { recursive: true });
  const harPath = join(TEST_DIR, 'recording.har');
  const screenshotsDir = join(TEST_DIR, 'screenshots');
  mkdirSync(screenshotsDir, { recursive: true });

  // 1. Start test server
  console.log('1. Starting test server...');
  const server = await startTestServer();
  console.log(`   Server running at ${TEST_URL}\n`);

  let browser, context, page;

  try {
    // 2. Launch browser with HAR recording
    console.log('2. Launching browser with HAR recording...');
    browser = await chromium.launch({ headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu', '--single-process'] });
    context = await browser.newContext({
      recordHar: { path: harPath, mode: 'full' },
      viewport: { width: 1280, height: 800 },
    });
    page = await context.newPage();
    assert(true, 'Browser launched with HAR recording');

    // 3. Navigate to login page
    console.log('\n3. Testing login flow...');
    await page.goto(TEST_URL, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(500);

    // Take screenshot (with timeout protection)
    try {
      await page.screenshot({ path: join(screenshotsDir, '01-login.png'), timeout: 10000 });
      assert(existsSync(join(screenshotsDir, '01-login.png')), 'Login screenshot captured');
    } catch {
      console.log('  ⚠️  Screenshot timed out, continuing...');
    }

    // 4. Login
    await page.fill('#username', 'testuser');
    await page.fill('#password', 'testpass');
    await page.click('#loginBtn');
    await page.waitForURL('**/dashboard', { timeout: 10000 });
    await page.waitForTimeout(1000);

    const dashboardUrl = page.url();
    assert(dashboardUrl.includes('/dashboard'), `Navigated to dashboard: ${dashboardUrl}`);
    try {
      await page.screenshot({ path: join(screenshotsDir, '02-dashboard.png'), timeout: 10000 });
    } catch { /* ignore */ }

    // 5. Navigate to messages
    console.log('\n4. Testing messages page...');
    await page.click('#messagesLink');
    await page.waitForURL('**/messages', { timeout: 10000 });
    await page.waitForTimeout(2000);

    const messagesUrl = page.url();
    assert(messagesUrl.includes('/messages'), `Navigated to messages: ${messagesUrl}`);
    try {
      await page.screenshot({ path: join(screenshotsDir, '03-messages.png'), timeout: 10000 });
    } catch { /* ignore */ }

    // 6. Get cookies
    console.log('\n5. Checking captured data...');
    const browserCookies = await context.cookies();
    const cookieMap: Record<string, string> = {};
    for (const c of browserCookies) cookieMap[c.name] = c.value;
    assert(cookieMap['session'] === AUTH_TOKEN, 'Session cookie captured');

    // 7. Close browser to flush HAR
    await context.close();
    await browser.close();
    browser = null;

    // 8. Verify HAR file
    console.log('\n6. Analyzing HAR file...');
    assert(existsSync(harPath), 'HAR file exists');

    const harContent = readFileSync(harPath, 'utf-8');
    const harData = JSON.parse(harContent);
    const entryCount = harData.log?.entries?.length ?? 0;
    assert(entryCount > 0, `HAR has ${entryCount} entries`);

    // 9. Analyze HAR
    const analysis = analyzeHar(harData, TEST_URL);
    assert(analysis.apiRequests.length > 0, `Found ${analysis.apiRequests.length} API requests`);
    assert(analysis.targetDomain === 'localhost', `Target domain: ${analysis.targetDomain}`);

    // Check specific API endpoints were captured
    const endpoints = analysis.apiRequests.map(r => `${r.method} ${r.path}`);
    const hasLogin = endpoints.some(e => e.includes('/api/login'));
    const hasThreads = endpoints.some(e => e.includes('/api/threads'));
    const hasMessages = endpoints.some(e => e.includes('/api/messages'));
    const hasStats = endpoints.some(e => e.includes('/api/stats'));

    assert(hasLogin, 'Captured POST /api/login');
    assert(hasThreads, 'Captured GET /api/threads');
    assert(hasMessages, 'Captured GET /api/messages');
    assert(hasStats, 'Captured GET /api/stats');

    // 10. Check response bodies captured
    const threadsReq = analysis.apiRequests.find(r => r.path.includes('/api/threads') && !r.path.includes('/messages'));
    const hasResponseBody = threadsReq?.responseBody != null;
    assert(hasResponseBody, 'Response body captured for /api/threads');

    if (threadsReq?.responseBody) {
      try {
        const data = JSON.parse(threadsReq.responseBody);
        assert(Array.isArray(data.threads), `Threads data parsed: ${data.threads?.length} threads`);
      } catch {
        assert(false, 'Threads response is valid JSON');
      }
    }

    // 11. Prepare LLM summary
    console.log('\n7. Testing LLM summary preparation...');
    const llmSummary = prepareHarSummaryForLLM(analysis.apiRequests);
    assert(llmSummary.length > 100, `LLM summary generated (${llmSummary.length} chars)`);
    assert(llmSummary.includes('/api/'), 'Summary includes API paths');

    // 12. Save session data
    console.log('\n8. Saving session data...');
    const session: RecordingSession = {
      id: `e2e-test-${Date.now()}`,
      url: TEST_URL,
      description: 'Get all messages from the test app',
      startTime: new Date().toISOString(),
      endTime: new Date().toISOString(),
      actions: [
        { type: 'navigate', timestamp: new Date().toISOString(), url: TEST_URL, description: 'Navigate to login' },
        { type: 'type', timestamp: new Date().toISOString(), url: TEST_URL, selector: '#username', value: 'testuser' },
        { type: 'type', timestamp: new Date().toISOString(), url: TEST_URL, selector: '#password', value: '***' },
        { type: 'click', timestamp: new Date().toISOString(), url: TEST_URL, selector: '#loginBtn', tagName: 'button', text: 'Log In' },
        { type: 'navigate', timestamp: new Date().toISOString(), url: `${TEST_URL}/dashboard`, description: 'Navigate to dashboard' },
        { type: 'click', timestamp: new Date().toISOString(), url: `${TEST_URL}/dashboard`, selector: '#messagesLink', tagName: 'a', text: 'Messages' },
        { type: 'navigate', timestamp: new Date().toISOString(), url: `${TEST_URL}/messages`, description: 'Navigate to messages' },
      ],
      apiRequests: analysis.apiRequests,
      cookies: cookieMap,
      authHeaders: analysis.authHeaders,
      authMethod: analysis.authMethod,
      targetDomain: analysis.targetDomain,
      harFilePath: harPath,
      screenshotsDir,
    };

    writeFileSync(join(TEST_DIR, 'session.json'), JSON.stringify(session, null, 2));
    writeFileSync(join(TEST_DIR, 'auth.json'), JSON.stringify({
      authHeaders: analysis.authHeaders,
      cookies: cookieMap,
      authMethod: analysis.authMethod,
    }, null, 2));
    writeFileSync(join(TEST_DIR, 'actions.json'), JSON.stringify(session.actions, null, 2));

    assert(existsSync(join(TEST_DIR, 'session.json')), 'session.json saved');
    assert(existsSync(join(TEST_DIR, 'auth.json')), 'auth.json saved');
    assert(existsSync(join(TEST_DIR, 'actions.json')), 'actions.json saved');

    // 13. Test Gemini script generation (if API key provided)
    if (geminiKey) {
      console.log('\n9. Testing Gemini script generation...');
      try {
        const result = await generateScript(session, geminiKey);
        assert(result.script.length > 100, `Script generated (${result.script.length} chars)`);
        assert(result.script.includes('chromium') || result.script.includes('playwright'), 'Script imports playwright');
        assert(result.strategy !== '', `Strategy: ${result.strategy}`);

        writeFileSync(join(TEST_DIR, 'automation.ts'), result.script);
        writeFileSync(join(TEST_DIR, 'generation-info.json'), JSON.stringify({
          strategy: result.strategy,
          apiEndpoints: result.apiEndpoints,
          explanation: result.explanation,
        }, null, 2));

        assert(existsSync(join(TEST_DIR, 'automation.ts')), 'automation.ts saved');
        console.log(`\n   Generated script strategy: ${result.strategy}`);
        if (result.apiEndpoints.length > 0) {
          console.log(`   API endpoints targeted: ${result.apiEndpoints.join(', ')}`);
        }
      } catch (err: any) {
        console.log(`  ⚠️  Gemini generation failed: ${err.message}`);
        assert(false, 'Gemini script generation');
      }
    } else {
      console.log('\n9. Skipping Gemini test (no GEMINI_API_KEY set)');
    }

  } catch (err: any) {
    console.error(`\n💥 Test error: ${err.message}`);
    console.error(err.stack);
    failed++;
  } finally {
    // Cleanup
    if (browser) {
      try { await context!.close(); } catch {}
      try { await browser.close(); } catch {}
    }
    server.close();
  }

  // Summary
  console.log('\n' + '═'.repeat(50));
  console.log(`TEST RESULTS: ${passed} passed, ${failed} failed`);
  console.log(`Output directory: ${TEST_DIR}`);
  console.log('═'.repeat(50) + '\n');

  if (failed > 0) {
    process.exit(1);
  }
}

runTest().catch((err) => {
  console.error('Fatal:', err);
  process.exit(1);
});
