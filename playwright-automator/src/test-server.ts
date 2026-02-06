/**
 * Test Server — A sample web application for testing the Playwright Automator.
 *
 * Provides:
 * - A login page with a mock auth flow
 * - A dashboard with messages (simulating DMs)
 * - API endpoints that return JSON data
 * - Cookie-based auth
 *
 * Run: npx tsx src/test-server.ts
 * Then point the automator at http://localhost:3847
 */

import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';

const PORT = 3847;
const AUTH_TOKEN = 'test-session-token-12345';

// Mock data
const users = [
  { id: 1, name: 'Alice Johnson', avatar: '👩' },
  { id: 2, name: 'Bob Smith', avatar: '👨' },
  { id: 3, name: 'Carol Williams', avatar: '👩‍💼' },
  { id: 4, name: 'Dave Brown', avatar: '🧑‍💻' },
];

const messages = [
  { id: 1, from: users[0], to: 'you', text: 'Hey! Are you coming to the meeting tomorrow?', timestamp: '2025-01-15T10:30:00Z', read: true },
  { id: 2, from: users[0], to: 'you', text: 'I have the presentation slides ready', timestamp: '2025-01-15T10:31:00Z', read: true },
  { id: 3, from: users[1], to: 'you', text: 'Can you review my PR? It is the auth refactor one.', timestamp: '2025-01-15T11:00:00Z', read: false },
  { id: 4, from: users[2], to: 'you', text: 'The deployment to staging is complete!', timestamp: '2025-01-15T12:00:00Z', read: false },
  { id: 5, from: users[2], to: 'you', text: 'Let me know if you see any issues', timestamp: '2025-01-15T12:01:00Z', read: false },
  { id: 6, from: users[3], to: 'you', text: 'Quick question about the API design', timestamp: '2025-01-15T14:00:00Z', read: true },
  { id: 7, from: users[3], to: 'you', text: 'Should we use REST or GraphQL for the new service?', timestamp: '2025-01-15T14:02:00Z', read: true },
  { id: 8, from: users[0], to: 'you', text: 'Meeting moved to 3pm', timestamp: '2025-01-15T15:00:00Z', read: false },
  { id: 9, from: users[1], to: 'you', text: 'Thanks for the review!', timestamp: '2025-01-16T09:00:00Z', read: true },
  { id: 10, from: users[3], to: 'you', text: 'Let us go with REST, simpler for our use case', timestamp: '2025-01-16T10:00:00Z', read: false },
];

const threads = [
  { id: 't1', participant: users[0], lastMessage: messages[7], unreadCount: 1, messageCount: 3 },
  { id: 't2', participant: users[1], lastMessage: messages[8], unreadCount: 0, messageCount: 2 },
  { id: 't3', participant: users[2], lastMessage: messages[4], unreadCount: 2, messageCount: 2 },
  { id: 't4', participant: users[3], lastMessage: messages[9], unreadCount: 1, messageCount: 3 },
];

function parseCookies(req: IncomingMessage): Record<string, string> {
  const cookies: Record<string, string> = {};
  const header = req.headers.cookie;
  if (header) {
    for (const pair of header.split(';')) {
      const [name, ...rest] = pair.trim().split('=');
      if (name) cookies[name] = rest.join('=');
    }
  }
  return cookies;
}

function isAuthenticated(req: IncomingMessage): boolean {
  const cookies = parseCookies(req);
  return cookies['session'] === AUTH_TOKEN;
}

function getBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve) => {
    let body = '';
    req.on('data', (chunk: Buffer) => { body += chunk.toString(); });
    req.on('end', () => resolve(body));
  });
}

function sendJson(res: ServerResponse, data: any, status = 200) {
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Access-Control-Allow-Origin': '*',
  });
  res.end(JSON.stringify(data));
}

function sendHtml(res: ServerResponse, html: string, status = 200, extraHeaders: Record<string, string> = {}) {
  res.writeHead(status, {
    'Content-Type': 'text/html; charset=utf-8',
    ...extraHeaders,
  });
  res.end(html);
}

// HTML Templates
const loginPageHtml = `<!DOCTYPE html>
<html>
<head><title>TestApp - Login</title>
<style>
  body { font-family: system-ui; max-width: 400px; margin: 100px auto; padding: 20px; }
  h1 { text-align: center; }
  form { display: flex; flex-direction: column; gap: 12px; }
  input { padding: 10px; border: 1px solid #ccc; border-radius: 4px; font-size: 16px; }
  button { padding: 12px; background: #0066ff; color: white; border: none; border-radius: 4px; font-size: 16px; cursor: pointer; }
  button:hover { background: #0052cc; }
  .error { color: red; text-align: center; }
  .hint { color: #666; text-align: center; font-size: 14px; margin-top: 20px; }
</style>
</head>
<body>
  <h1>TestApp</h1>
  <form id="loginForm" action="/api/login" method="POST">
    <input type="text" name="username" id="username" placeholder="Username" required>
    <input type="password" name="password" id="password" placeholder="Password" required>
    <button type="submit">Log In</button>
  </form>
  <div id="error" class="error"></div>
  <div class="hint">Use: testuser / testpass</div>
  <script>
    document.getElementById('loginForm').addEventListener('submit', async (e) => {
      e.preventDefault();
      const username = document.getElementById('username').value;
      const password = document.getElementById('password').value;
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
          document.getElementById('error').textContent = data.error || 'Login failed';
        }
      } catch (err) {
        document.getElementById('error').textContent = 'Connection error';
      }
    });
  </script>
</body>
</html>`;

const dashboardHtml = `<!DOCTYPE html>
<html>
<head><title>TestApp - Dashboard</title>
<style>
  body { font-family: system-ui; max-width: 800px; margin: 0 auto; padding: 20px; }
  nav { display: flex; justify-content: space-between; align-items: center; padding: 10px 0; border-bottom: 1px solid #eee; margin-bottom: 20px; }
  nav a { color: #0066ff; text-decoration: none; margin-left: 16px; }
  .stats { display: flex; gap: 20px; margin-bottom: 30px; }
  .stat-card { flex: 1; padding: 20px; background: #f5f5f5; border-radius: 8px; text-align: center; }
  .stat-card h3 { margin: 0; color: #666; font-size: 14px; }
  .stat-card .value { font-size: 32px; font-weight: bold; margin: 8px 0; }
  h1 { margin: 0; }
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
  <div class="stats">
    <div class="stat-card"><h3>Messages</h3><div class="value" id="msgCount">-</div></div>
    <div class="stat-card"><h3>Unread</h3><div class="value" id="unreadCount">-</div></div>
    <div class="stat-card"><h3>Contacts</h3><div class="value" id="contactCount">-</div></div>
  </div>
  <h2>Recent Messages</h2>
  <div id="recentMessages">Loading...</div>
  <script>
    async function loadDashboard() {
      const [threadsRes, statsRes] = await Promise.all([
        fetch('/api/threads'),
        fetch('/api/stats'),
      ]);
      const threadsData = await threadsRes.json();
      const statsData = await statsRes.json();
      document.getElementById('msgCount').textContent = statsData.totalMessages;
      document.getElementById('unreadCount').textContent = statsData.unreadMessages;
      document.getElementById('contactCount').textContent = statsData.totalContacts;
      const html = threadsData.threads.map(t =>
        '<div style="padding:12px;border-bottom:1px solid #eee;cursor:pointer" onclick="location.href=\\'/messages/'+t.id+'\\'">' +
        '<strong>'+t.participant.avatar+' '+t.participant.name+'</strong>' +
        (t.unreadCount > 0 ? ' <span style="background:#0066ff;color:white;padding:2px 8px;border-radius:10px;font-size:12px">'+t.unreadCount+'</span>' : '') +
        '<p style="margin:4px 0;color:#666">'+t.lastMessage.text+'</p>' +
        '</div>'
      ).join('');
      document.getElementById('recentMessages').innerHTML = html;
    }
    loadDashboard();
  </script>
</body>
</html>`;

const messagesHtml = `<!DOCTYPE html>
<html>
<head><title>TestApp - Messages</title>
<style>
  body { font-family: system-ui; max-width: 800px; margin: 0 auto; padding: 20px; }
  nav { display: flex; justify-content: space-between; align-items: center; padding: 10px 0; border-bottom: 1px solid #eee; margin-bottom: 20px; }
  nav a { color: #0066ff; text-decoration: none; margin-left: 16px; }
  h1 { margin: 0; }
  .thread { padding: 16px; border: 1px solid #eee; border-radius: 8px; margin-bottom: 12px; cursor: pointer; }
  .thread:hover { background: #f5f5f5; }
  .thread-header { display: flex; justify-content: space-between; align-items: center; }
  .thread-preview { color: #666; margin-top: 4px; }
  .unread-badge { background: #0066ff; color: white; padding: 2px 8px; border-radius: 10px; font-size: 12px; }
  .message { padding: 12px; margin: 8px 0; border-radius: 8px; }
  .message.received { background: #f0f0f0; margin-right: 100px; }
  .message.sent { background: #0066ff; color: white; margin-left: 100px; }
  .message .meta { font-size: 12px; color: #999; margin-top: 4px; }
  .message.sent .meta { color: #cce; }
</style>
</head>
<body>
  <nav>
    <h1>TestApp - Messages</h1>
    <div>
      <a href="/dashboard">Dashboard</a>
      <a href="/messages">Messages</a>
      <a href="/api/logout">Logout</a>
    </div>
  </nav>
  <div id="threadList">Loading threads...</div>
  <div id="messageView" style="display:none">
    <a href="/messages" style="color:#0066ff;text-decoration:none">&larr; Back to threads</a>
    <h2 id="threadTitle"></h2>
    <div id="messageList"></div>
  </div>
  <script>
    async function loadThreads() {
      const res = await fetch('/api/threads');
      const data = await res.json();
      const html = data.threads.map(t =>
        '<div class="thread" onclick="loadThread(\\''+t.id+'\\')">' +
        '<div class="thread-header">' +
        '<strong>'+t.participant.avatar+' '+t.participant.name+'</strong>' +
        (t.unreadCount > 0 ? '<span class="unread-badge">'+t.unreadCount+' new</span>' : '') +
        '</div>' +
        '<div class="thread-preview">'+t.lastMessage.text+'</div>' +
        '</div>'
      ).join('');
      document.getElementById('threadList').innerHTML = html;
    }

    async function loadThread(threadId) {
      const res = await fetch('/api/threads/' + threadId + '/messages');
      const data = await res.json();
      document.getElementById('threadList').style.display = 'none';
      document.getElementById('messageView').style.display = 'block';
      document.getElementById('threadTitle').textContent = data.participant.avatar + ' ' + data.participant.name;
      const html = data.messages.map(m =>
        '<div class="message received">' +
        '<div>'+m.text+'</div>' +
        '<div class="meta">'+new Date(m.timestamp).toLocaleString()+'</div>' +
        '</div>'
      ).join('');
      document.getElementById('messageList').innerHTML = html;
    }

    loadThreads();
  </script>
</body>
</html>`;

// Server handler
const server = createServer(async (req, res) => {
  const url = new URL(req.url || '/', `http://localhost:${PORT}`);
  const path = url.pathname;
  const method = req.method || 'GET';

  // CORS preflight
  if (method === 'OPTIONS') {
    res.writeHead(204, {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type',
    });
    res.end();
    return;
  }

  // --- Public routes ---

  // Login page
  if (path === '/' || path === '/login') {
    if (isAuthenticated(req)) {
      res.writeHead(302, { Location: '/dashboard' });
      res.end();
      return;
    }
    sendHtml(res, loginPageHtml);
    return;
  }

  // Login API
  if (path === '/api/login' && method === 'POST') {
    const body = await getBody(req);
    let data: any;
    try {
      data = JSON.parse(body);
    } catch {
      sendJson(res, { success: false, error: 'Invalid JSON' }, 400);
      return;
    }

    if (data.username === 'testuser' && data.password === 'testpass') {
      res.writeHead(200, {
        'Content-Type': 'application/json',
        'Set-Cookie': `session=${AUTH_TOKEN}; Path=/; HttpOnly`,
      });
      res.end(JSON.stringify({ success: true, user: { name: 'Test User', id: 100 } }));
    } else {
      sendJson(res, { success: false, error: 'Invalid credentials' }, 401);
    }
    return;
  }

  // --- Authenticated routes ---

  if (!isAuthenticated(req)) {
    if (path.startsWith('/api/')) {
      sendJson(res, { error: 'Unauthorized' }, 401);
    } else {
      res.writeHead(302, { Location: '/login' });
      res.end();
    }
    return;
  }

  // Logout
  if (path === '/api/logout') {
    res.writeHead(302, {
      Location: '/login',
      'Set-Cookie': 'session=; Path=/; Max-Age=0',
    });
    res.end();
    return;
  }

  // Dashboard
  if (path === '/dashboard') {
    sendHtml(res, dashboardHtml);
    return;
  }

  // Messages page
  if (path === '/messages' || path.startsWith('/messages/')) {
    sendHtml(res, messagesHtml);
    return;
  }

  // --- API endpoints ---

  // Stats
  if (path === '/api/stats' && method === 'GET') {
    sendJson(res, {
      totalMessages: messages.length,
      unreadMessages: messages.filter(m => !m.read).length,
      totalContacts: users.length,
      totalThreads: threads.length,
    });
    return;
  }

  // List threads
  if (path === '/api/threads' && method === 'GET') {
    const page = parseInt(url.searchParams.get('page') || '1');
    const limit = parseInt(url.searchParams.get('limit') || '10');
    const start = (page - 1) * limit;
    const paginatedThreads = threads.slice(start, start + limit);

    sendJson(res, {
      threads: paginatedThreads,
      pagination: {
        page,
        limit,
        total: threads.length,
        hasMore: start + limit < threads.length,
      },
    });
    return;
  }

  // Get thread messages
  const threadMatch = path.match(/^\/api\/threads\/(t\d+)\/messages$/);
  if (threadMatch && method === 'GET') {
    const threadId = threadMatch[1];
    const thread = threads.find(t => t.id === threadId);
    if (!thread) {
      sendJson(res, { error: 'Thread not found' }, 404);
      return;
    }

    const threadMessages = messages.filter(
      m => m.from.id === thread.participant.id
    );

    sendJson(res, {
      threadId,
      participant: thread.participant,
      messages: threadMessages,
      messageCount: threadMessages.length,
    });
    return;
  }

  // List all messages (flat)
  if (path === '/api/messages' && method === 'GET') {
    const page = parseInt(url.searchParams.get('page') || '1');
    const limit = parseInt(url.searchParams.get('limit') || '20');
    const start = (page - 1) * limit;
    const paginatedMessages = messages.slice(start, start + limit);

    sendJson(res, {
      messages: paginatedMessages,
      pagination: {
        page,
        limit,
        total: messages.length,
        hasMore: start + limit < messages.length,
      },
    });
    return;
  }

  // List users/contacts
  if (path === '/api/users' && method === 'GET') {
    sendJson(res, { users });
    return;
  }

  // Current user
  if (path === '/api/me' && method === 'GET') {
    sendJson(res, { user: { id: 100, name: 'Test User', email: 'test@example.com' } });
    return;
  }

  // 404
  if (path.startsWith('/api/')) {
    sendJson(res, { error: 'Not found' }, 404);
  } else {
    sendHtml(res, '<h1>404 Not Found</h1>', 404);
  }
});

server.listen(PORT, () => {
  console.log(`\n🧪 Test server running at http://localhost:${PORT}`);
  console.log(`   Login: testuser / testpass`);
  console.log(`   Dashboard: http://localhost:${PORT}/dashboard`);
  console.log(`   Messages: http://localhost:${PORT}/messages`);
  console.log(`\n   API endpoints:`);
  console.log(`   POST /api/login     - Login ({"username":"testuser","password":"testpass"})`);
  console.log(`   GET  /api/me        - Current user`);
  console.log(`   GET  /api/stats     - Message stats`);
  console.log(`   GET  /api/threads   - List message threads`);
  console.log(`   GET  /api/threads/:id/messages - Get thread messages`);
  console.log(`   GET  /api/messages  - All messages (paginated)`);
  console.log(`   GET  /api/users     - List contacts\n`);
});
