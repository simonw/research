export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);

    // Hello World page
    if (url.pathname === '/' || url.pathname === '/hello') {
      return new Response("Hello World from Cloudflare Workers!", {
        headers: { 'Content-Type': 'text/plain' }
      });
    }

    // Form page
    if (url.pathname === '/form') {
      if (request.method === 'GET') {
        const html = `<!DOCTYPE html>
<html>
<head><title>Form Demo</title></head>
<body>
<h1>Server-Side Form Processing</h1>
<form method="POST" action="/form">
  <label>Your Name: <input type="text" name="name"></label><br><br>
  <label>Your Message: <textarea name="message"></textarea></label><br><br>
  <button type="submit">Submit</button>
</form>
</body>
</html>`;
        return new Response(html, {
          headers: { 'Content-Type': 'text/html' }
        });
      }

      if (request.method === 'POST') {
        const formData = await request.formData();
        const name = formData.get('name') || 'Anonymous';
        const message = formData.get('message') || '(no message)';

        const html = `<!DOCTYPE html>
<html>
<head><title>Form Result</title></head>
<body>
<h1>Form Submitted Successfully!</h1>
<p><strong>Name:</strong> ${escapeHtml(name)}</p>
<p><strong>Message:</strong> ${escapeHtml(message)}</p>
<p><a href="/form">Submit another</a></p>
</body>
</html>`;
        return new Response(html, {
          headers: { 'Content-Type': 'text/html' }
        });
      }
    }

    // Page counter with SQLite (D1)
    if (url.pathname === '/counter') {
      try {
        // Initialize table if it doesn't exist
        await env.DB.prepare(`
          CREATE TABLE IF NOT EXISTS page_views (
            page TEXT PRIMARY KEY,
            count INTEGER DEFAULT 0
          )
        `).run();

        // Increment counter
        await env.DB.prepare(`
          INSERT INTO page_views (page, count) VALUES ('counter', 1)
          ON CONFLICT(page) DO UPDATE SET count = count + 1
        `).run();

        // Get current count
        const result = await env.DB.prepare(
          'SELECT count FROM page_views WHERE page = ?'
        ).bind('counter').first();

        const count = result ? result.count : 0;

        const html = `<!DOCTYPE html>
<html>
<head><title>Page Counter</title></head>
<body>
<h1>SQLite Page Counter</h1>
<p>This page has been viewed <strong>${count}</strong> times.</p>
<p>Counter is persisted using Cloudflare D1 (SQLite).</p>
<p><a href="/counter">Refresh to increment</a></p>
</body>
</html>`;
        return new Response(html, {
          headers: { 'Content-Type': 'text/html' }
        });
      } catch (error) {
        return new Response(`Database error: ${error.message}`, { status: 500 });
      }
    }

    return new Response("Not Found", { status: 404 });
  },
};

function escapeHtml(str) {
  return str.replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
}
