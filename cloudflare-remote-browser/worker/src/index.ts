import { Env } from './types';

export { BrowserSession } from './session';

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    const corsHeaders = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, DELETE, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    };

    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    const upgradeHeader = request.headers.get('Upgrade');
    const isWebSocket = upgradeHeader?.toLowerCase() === 'websocket';

    if (!isWebSocket) {
      const authHeader = request.headers.get('Authorization');
      if (env.API_KEY && authHeader !== `Bearer ${env.API_KEY}`) {
        return new Response('Unauthorized', { status: 401, headers: corsHeaders });
      }
    }

    try {
      if (path === '/sessions' && request.method === 'POST') {
        const id = crypto.randomUUID();
        const stub = env.BROWSER_SESSION.get(env.BROWSER_SESSION.idFromName(id));
        await stub.fetch(new Request('http://internal/init', { method: 'POST' }));
        return Response.json({ sessionId: id }, { headers: corsHeaders });
      }

      const sessionMatch = path.match(/^\/sessions\/([^/]+)(\/.*)?$/);
      if (sessionMatch) {
        const [, sessionId, subPath = ''] = sessionMatch;
        const stub = env.BROWSER_SESSION.get(env.BROWSER_SESSION.idFromName(sessionId));

        if (subPath === '/ws' && isWebSocket) {
          return stub.fetch(request);
        }

        if (subPath === '/script' && request.method === 'POST') {
          const { code } = await request.json() as { code: string };
          const response = await stub.fetch(new Request('http://internal/script', {
            method: 'POST',
            body: JSON.stringify({ code }),
            headers: { 'Content-Type': 'application/json' }
          }));
          const data = await response.json();
          return Response.json(data, { headers: corsHeaders });
        }

        if (subPath === '/done' && request.method === 'POST') {
          const response = await stub.fetch(new Request('http://internal/done', { method: 'POST' }));
          const data = await response.json();
          return Response.json(data, { headers: corsHeaders });
        }

        if (request.method === 'DELETE' && !subPath) {
          const response = await stub.fetch(new Request('http://internal/destroy', { method: 'POST' }));
          const data = await response.json();
          return Response.json(data, { headers: corsHeaders });
        }

        if (request.method === 'GET' && !subPath) {
          const response = await stub.fetch(new Request('http://internal/status'));
          const data = await response.json();
          return Response.json(data, { headers: corsHeaders });
        }
      }

      return new Response('Not Found', { status: 404, headers: corsHeaders });
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      const stack = error instanceof Error ? error.stack : undefined;
      
      console.error('Worker Error:', message, stack);
      
      return Response.json({ 
        error: message,
        stack: stack
      }, { status: 500, headers: corsHeaders });
    }
  }
};
