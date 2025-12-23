import { Env } from './types';

interface RegistryEntry {
  createdAt: number;
}

export class SessionRegistry {
  private state: DurableObjectState;

  constructor(state: DurableObjectState, _env: Env) {
    this.state = state;
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    if (path === '/add' && request.method === 'POST') {
      const body = await request.json() as { sessionId?: string };
      if (!body.sessionId) {
        return Response.json({ error: 'Missing sessionId' }, { status: 400 });
      }
      await this.state.storage.put(body.sessionId, { createdAt: Date.now() } satisfies RegistryEntry);
      return Response.json({ ok: true });
    }

    if (path === '/remove' && request.method === 'POST') {
      const body = await request.json() as { sessionId?: string };
      if (!body.sessionId) {
        return Response.json({ error: 'Missing sessionId' }, { status: 400 });
      }
      await this.state.storage.delete(body.sessionId);
      return Response.json({ ok: true });
    }

    if (path === '/list' && request.method === 'GET') {
      const items = await this.state.storage.list<RegistryEntry>({});
      const sessions = Array.from(items.entries()).map(([sessionId, value]) => ({
        sessionId,
        createdAt: value.createdAt,
      }));
      sessions.sort((a, b) => b.createdAt - a.createdAt);
      return Response.json({ sessions });
    }

    return new Response('Not Found', { status: 404 });
  }
}
