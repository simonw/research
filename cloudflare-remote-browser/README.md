# Cloudflare Remote Browser with User Takeover

A system for running Playwright scripts on a remote browser with live streaming and user takeover capability for handling logins, captchas, and 2FA.

## Overview

This project replicates key functionality from ChatGPT's Operator/Agent mode:
- **Remote browser automation** using Playwright scripts
- **Live browser streaming** via CDP screencast
- **User takeover** for interactive elements (captchas, logins, 2FA)

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Next.js Frontend (Vercel)                       │
│                                                                     │
│  ┌──────────────────┐  ┌──────────────────────────────────────────┐│
│  │  Script Editor   │  │  Browser Viewer                          ││
│  │  (Monaco)        │  │  ┌────────────────────────────────────┐  ││
│  │                  │  │  │  Canvas (CDP screencast frames)    │  ││
│  │  [▶ Run]         │  │  │  + Mouse/keyboard capture          │  ││
│  │  [⏹ Stop]        │  │  └────────────────────────────────────┘  ││
│  │                  │  │  Status: Running | Waiting for you       ││
│  └──────────────────┘  │  [✓ I'm Done]                            ││
│                        │  Error: (displayed below)                 ││
│                        └──────────────────────────────────────────┘│
└────────────────────────────────┬────────────────────────────────────┘
                                 │ WebSocket + REST
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Cloudflare Worker                               │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │              BrowserSession (Durable Object)                  │ │
│  │  - Manages Playwright browser/page                            │ │
│  │  - Runs user scripts with takeover support                    │ │
│  │  - Streams CDP screencast frames                              │ │
│  │  - Forwards user input during takeover                        │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                 │                                   │
│                                 ▼                                   │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │           Cloudflare Browser Rendering (Managed Chrome)       │ │
│  └───────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

## Technology Stack

| Component | Technology |
|-----------|------------|
| Browser Runtime | Cloudflare Browser Rendering |
| Backend | Cloudflare Workers + Durable Objects |
| Browser Automation | @cloudflare/playwright |
| Streaming | CDP Screencast (Page.startScreencast) |
| Frontend | Next.js 14+ (App Router) |
| Script Editor | Monaco Editor |
| Deployment | Cloudflare (Worker), Vercel (Frontend) |

---

## Implementation Stages

### Stage 1: Cloudflare Worker Setup

**Goal:** Basic Worker with Browser Rendering and Durable Object

#### File Structure
```
worker/
├── src/
│   ├── index.ts              # Worker entrypoint, routing
│   ├── session.ts            # BrowserSession Durable Object
│   └── types.ts              # TypeScript types
├── wrangler.toml
├── package.json
└── tsconfig.json
```

#### wrangler.toml
```toml
name = "remote-browser"
main = "src/index.ts"
compatibility_date = "2025-09-15"
compatibility_flags = ["nodejs_compat"]

# Browser Rendering binding
[browser]
binding = "BROWSER"

# Durable Object for session management
[[durable_objects.bindings]]
name = "BROWSER_SESSION"
class_name = "BrowserSession"

[[migrations]]
tag = "v1"
new_classes = ["BrowserSession"]
```

#### package.json
```json
{
  "name": "remote-browser-worker",
  "version": "1.0.0",
  "private": true,
  "scripts": {
    "dev": "wrangler dev",
    "deploy": "wrangler deploy",
    "types": "wrangler types"
  },
  "dependencies": {
    "@cloudflare/playwright": "^1.55.0"
  },
  "devDependencies": {
    "@cloudflare/workers-types": "^4.20241205.0",
    "typescript": "^5.0.0",
    "wrangler": "^3.99.0"
  }
}
```

#### src/types.ts
```typescript
export interface Env {
  BROWSER: Fetcher;
  BROWSER_SESSION: DurableObjectNamespace;
}

export type SessionStatus = 'idle' | 'starting' | 'running' | 'takeover' | 'done' | 'error';

export interface SessionState {
  status: SessionStatus;
  takeoverMessage?: string;
  error?: string;
  result?: unknown;
}

// WebSocket message types
export type ServerMessage =
  | { type: 'frame'; data: string; timestamp: number }
  | { type: 'status'; status: SessionStatus; message?: string }
  | { type: 'result'; data: unknown }
  | { type: 'error'; message: string };

export type ClientMessage =
  | { type: 'mouse'; action: string; x: number; y: number; button?: string }
  | { type: 'keyboard'; action: string; key: string; text?: string }
  | { type: 'done' }
  | { type: 'scroll'; deltaX: number; deltaY: number; x: number; y: number };
```

#### src/index.ts
```typescript
import { Env } from './types';

export { BrowserSession } from './session';

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    // CORS headers
    const corsHeaders = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, DELETE, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    };

    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    // Simple API key auth
    const authHeader = request.headers.get('Authorization');
    const apiKey = env.API_KEY; // Set in wrangler.toml or dashboard
    if (apiKey && authHeader !== `Bearer ${apiKey}`) {
      return new Response('Unauthorized', { status: 401 });
    }

    try {
      // POST /sessions - Create new session
      if (path === '/sessions' && request.method === 'POST') {
        const id = crypto.randomUUID();
        const stub = env.BROWSER_SESSION.get(env.BROWSER_SESSION.idFromName(id));
        await stub.fetch(new Request('http://internal/init', { method: 'POST' }));
        return Response.json({ sessionId: id }, { headers: corsHeaders });
      }

      // Match /sessions/:id routes
      const sessionMatch = path.match(/^\/sessions\/([^/]+)(\/.*)?$/);
      if (sessionMatch) {
        const [, sessionId, subPath = ''] = sessionMatch;
        const stub = env.BROWSER_SESSION.get(env.BROWSER_SESSION.idFromName(sessionId));

        // WebSocket upgrade for streaming
        if (subPath === '/ws' && request.headers.get('Upgrade') === 'websocket') {
          return stub.fetch(request);
        }

        // POST /sessions/:id/script - Run script
        if (subPath === '/script' && request.method === 'POST') {
          const { code } = await request.json() as { code: string };
          return stub.fetch(new Request('http://internal/script', {
            method: 'POST',
            body: JSON.stringify({ code }),
            headers: { 'Content-Type': 'application/json' }
          }));
        }

        // POST /sessions/:id/done - Complete takeover
        if (subPath === '/done' && request.method === 'POST') {
          return stub.fetch(new Request('http://internal/done', { method: 'POST' }));
        }

        // DELETE /sessions/:id - End session
        if (request.method === 'DELETE') {
          return stub.fetch(new Request('http://internal/destroy', { method: 'POST' }));
        }

        // GET /sessions/:id - Get status
        if (request.method === 'GET' && !subPath) {
          return stub.fetch(new Request('http://internal/status'));
        }
      }

      return new Response('Not Found', { status: 404, headers: corsHeaders });
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      return Response.json({ error: message }, { status: 500, headers: corsHeaders });
    }
  }
};
```

#### src/session.ts
```typescript
import { launch, Browser, Page, CDPSession } from '@cloudflare/playwright';
import { Env, SessionStatus, ServerMessage, ClientMessage } from './types';

export class BrowserSession {
  private state: DurableObjectState;
  private env: Env;
  
  private browser: Browser | null = null;
  private page: Page | null = null;
  private cdp: CDPSession | null = null;
  
  private status: SessionStatus = 'idle';
  private takeoverMessage = '';
  private error = '';
  private result: unknown = null;
  
  private wsConnections: Set<WebSocket> = new Set();
  private takeoverResolver: (() => void) | null = null;
  
  private viewportWidth = 1280;
  private viewportHeight = 720;

  constructor(state: DurableObjectState, env: Env) {
    this.state = state;
    this.env = env;
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    // WebSocket connection
    if (request.headers.get('Upgrade') === 'websocket') {
      return this.handleWebSocket(request);
    }

    // Initialize session
    if (path === '/init' && request.method === 'POST') {
      return this.initSession();
    }

    // Run script
    if (path === '/script' && request.method === 'POST') {
      const { code } = await request.json() as { code: string };
      // Run async, don't await
      this.runScript(code).catch(e => this.handleError(e));
      return Response.json({ status: 'started' });
    }

    // Complete takeover
    if (path === '/done' && request.method === 'POST') {
      this.completeTakeover();
      return Response.json({ status: 'ok' });
    }

    // Get status
    if (path === '/status') {
      return Response.json({
        status: this.status,
        takeoverMessage: this.takeoverMessage,
        error: this.error,
        result: this.result
      });
    }

    // Destroy session
    if (path === '/destroy') {
      await this.cleanup();
      return Response.json({ status: 'destroyed' });
    }

    return new Response('Not Found', { status: 404 });
  }

  private async initSession(): Promise<Response> {
    try {
      this.status = 'starting';
      this.broadcast({ type: 'status', status: 'starting' });

      // Launch browser
      this.browser = await launch(this.env.BROWSER);
      this.page = await this.browser.newPage();
      await this.page.setViewportSize({ 
        width: this.viewportWidth, 
        height: this.viewportHeight 
      });

      // Setup CDP for screencast
      this.cdp = await this.page.context().newCDPSession(this.page);
      await this.startScreencast();

      this.status = 'idle';
      this.broadcast({ type: 'status', status: 'idle' });

      return Response.json({ status: 'ready' });
    } catch (error) {
      this.handleError(error);
      return Response.json({ error: this.error }, { status: 500 });
    }
  }

  private async startScreencast(): Promise<void> {
    if (!this.cdp) return;

    this.cdp.on('Page.screencastFrame', async (params) => {
      // Broadcast frame to all connected clients
      this.broadcast({
        type: 'frame',
        data: params.data,
        timestamp: Date.now()
      });

      // Acknowledge frame to receive next one (critical!)
      await this.cdp!.send('Page.screencastFrameAck', {
        sessionId: params.sessionId
      });
    });

    await this.cdp.send('Page.startScreencast', {
      format: 'jpeg',
      quality: 60,
      maxWidth: this.viewportWidth,
      maxHeight: this.viewportHeight,
      everyNthFrame: 2
    });
  }

  private async runScript(code: string): Promise<void> {
    if (!this.page) {
      throw new Error('Browser not initialized');
    }

    this.status = 'running';
    this.error = '';
    this.result = null;
    this.broadcast({ type: 'status', status: 'running' });

    const page = this.page;

    // Create requestTakeover function
    const requestTakeover = (message: string): Promise<void> => {
      return new Promise((resolve) => {
        this.status = 'takeover';
        this.takeoverMessage = message;
        this.takeoverResolver = resolve;
        this.broadcast({ type: 'status', status: 'takeover', message });
      });
    };

    try {
      // Create async function from user code
      const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
      const scriptFn = new AsyncFunction('page', 'requestTakeover', code);

      // Run with 5 minute timeout
      const timeoutPromise = new Promise((_, reject) => {
        setTimeout(() => reject(new Error('Script timeout (5 minutes)')), 5 * 60 * 1000);
      });

      this.result = await Promise.race([
        scriptFn(page, requestTakeover),
        timeoutPromise
      ]);

      this.status = 'done';
      this.broadcast({ type: 'status', status: 'done' });
      this.broadcast({ type: 'result', data: this.result });
    } catch (error) {
      this.handleError(error);
    }
  }

  private completeTakeover(): void {
    if (this.takeoverResolver) {
      this.takeoverResolver();
      this.takeoverResolver = null;
      this.status = 'running';
      this.takeoverMessage = '';
      this.broadcast({ type: 'status', status: 'running' });
    }
  }

  private handleWebSocket(request: Request): Response {
    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair);

    server.accept();
    this.wsConnections.add(server);

    // Send current status on connect
    server.send(JSON.stringify({
      type: 'status',
      status: this.status,
      message: this.takeoverMessage
    }));

    server.addEventListener('message', async (event) => {
      try {
        const message = JSON.parse(event.data as string) as ClientMessage;
        await this.handleClientMessage(message);
      } catch (e) {
        console.error('WebSocket message error:', e);
      }
    });

    server.addEventListener('close', () => {
      this.wsConnections.delete(server);
    });

    return new Response(null, { status: 101, webSocket: client });
  }

  private async handleClientMessage(message: ClientMessage): Promise<void> {
    // Only allow input during takeover
    if (this.status !== 'takeover' || !this.cdp) return;

    switch (message.type) {
      case 'mouse':
        await this.cdp.send('Input.dispatchMouseEvent', {
          type: message.action as 'mousePressed' | 'mouseReleased' | 'mouseMoved',
          x: message.x,
          y: message.y,
          button: (message.button || 'left') as 'left' | 'right' | 'middle',
          clickCount: message.action === 'mousePressed' ? 1 : 0
        });
        break;

      case 'keyboard':
        await this.cdp.send('Input.dispatchKeyEvent', {
          type: message.action as 'keyDown' | 'keyUp' | 'char',
          key: message.key,
          text: message.text
        });
        break;

      case 'scroll':
        await this.cdp.send('Input.dispatchMouseEvent', {
          type: 'mouseWheel',
          x: message.x,
          y: message.y,
          deltaX: message.deltaX,
          deltaY: message.deltaY
        });
        break;

      case 'done':
        this.completeTakeover();
        break;
    }
  }

  private broadcast(message: ServerMessage): void {
    const data = JSON.stringify(message);
    for (const ws of this.wsConnections) {
      try {
        ws.send(data);
      } catch (e) {
        this.wsConnections.delete(ws);
      }
    }
  }

  private handleError(error: unknown): void {
    const message = error instanceof Error ? error.message : 'Unknown error';
    this.status = 'error';
    this.error = message;
    this.broadcast({ type: 'error', message });
    this.broadcast({ type: 'status', status: 'error' });
  }

  private async cleanup(): Promise<void> {
    if (this.cdp) {
      try {
        await this.cdp.send('Page.stopScreencast');
      } catch (e) { /* ignore */ }
    }
    if (this.browser) {
      try {
        await this.browser.close();
      } catch (e) { /* ignore */ }
    }
    this.browser = null;
    this.page = null;
    this.cdp = null;
    this.status = 'idle';
    
    // Close all WebSocket connections
    for (const ws of this.wsConnections) {
      try {
        ws.close();
      } catch (e) { /* ignore */ }
    }
    this.wsConnections.clear();
  }
}
```

---

### Stage 2: Next.js Frontend

**Goal:** UI with script editor, browser viewer, and takeover controls

#### File Structure
```
frontend/
├── src/
│   ├── app/
│   │   ├── page.tsx
│   │   ├── layout.tsx
│   │   └── globals.css
│   ├── components/
│   │   ├── ScriptEditor.tsx
│   │   ├── BrowserViewer.tsx
│   │   ├── SessionControls.tsx
│   │   └── StatusBar.tsx
│   ├── hooks/
│   │   └── useSession.ts
│   └── lib/
│       ├── api.ts
│       └── types.ts
├── package.json
├── next.config.js
└── .env.local
```

#### .env.local
```
NEXT_PUBLIC_WORKER_URL=https://remote-browser.your-subdomain.workers.dev
WORKER_API_KEY=your-api-key-here
```

#### src/lib/types.ts
```typescript
export type SessionStatus = 'idle' | 'starting' | 'running' | 'takeover' | 'done' | 'error';

export interface SessionState {
  sessionId: string | null;
  status: SessionStatus;
  takeoverMessage: string;
  error: string;
  result: unknown;
  connected: boolean;
}

export type ServerMessage =
  | { type: 'frame'; data: string; timestamp: number }
  | { type: 'status'; status: SessionStatus; message?: string }
  | { type: 'result'; data: unknown }
  | { type: 'error'; message: string };

export type ClientMessage =
  | { type: 'mouse'; action: string; x: number; y: number; button?: string }
  | { type: 'keyboard'; action: string; key: string; text?: string }
  | { type: 'done' }
  | { type: 'scroll'; deltaX: number; deltaY: number; x: number; y: number };
```

#### src/lib/api.ts
```typescript
const WORKER_URL = process.env.NEXT_PUBLIC_WORKER_URL || 'http://localhost:8787';
const API_KEY = process.env.NEXT_PUBLIC_API_KEY || 'dev-api-key-12345';

const headers = {
  'Content-Type': 'application/json',
  'Authorization': `Bearer ${API_KEY}`
};

export async function createSession(): Promise<{ sessionId: string }> {
  const res = await fetch(`${WORKER_URL}/sessions`, {
    method: 'POST',
    headers
  });
  if (!res.ok) {
    const error = await res.text();
    throw new Error(`Failed to create session: ${error}`);
  }
  return res.json();
}

export async function runScript(sessionId: string, code: string): Promise<void> {
  const res = await fetch(`${WORKER_URL}/sessions/${sessionId}/script`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ code })
  });
  if (!res.ok) {
    const error = await res.text();
    throw new Error(`Failed to run script: ${error}`);
  }
}

export async function completeTakeover(sessionId: string): Promise<void> {
  const res = await fetch(`${WORKER_URL}/sessions/${sessionId}/done`, {
    method: 'POST',
    headers
  });
  if (!res.ok) {
    const error = await res.text();
    throw new Error(`Failed to complete takeover: ${error}`);
  }
}

export async function destroySession(sessionId: string): Promise<void> {
  const res = await fetch(`${WORKER_URL}/sessions/${sessionId}`, {
    method: 'DELETE',
    headers
  });
  if (!res.ok) {
    const error = await res.text();
    throw new Error(`Failed to destroy session: ${error}`);
  }
}

export function getWebSocketUrl(sessionId: string): string {
  const wsUrl = WORKER_URL.replace('https://', 'wss://').replace('http://', 'ws://');
  return `${wsUrl}/sessions/${sessionId}/ws`;
}
```

#### src/hooks/useSession.ts
```typescript
'use client';

import { useState, useCallback, useRef, useEffect } from 'react';
import { SessionState, ServerMessage, ClientMessage } from '@/lib/types';
import * as api from '@/lib/api';

const INITIAL_STATE: SessionState = {
  sessionId: null,
  status: 'idle',
  takeoverMessage: '',
  error: '',
  result: null,
  connected: false
};

export function useSession() {
  const [state, setState] = useState<SessionState>(INITIAL_STATE);
  const wsRef = useRef<WebSocket | null>(null);
  const onFrameRef = useRef<((data: string) => void) | null>(null);

  const connect = useCallback((sessionId: string) => {
    const ws = new WebSocket(api.getWebSocketUrl(sessionId));
    wsRef.current = ws;

    ws.onopen = () => {
      setState(s => ({ ...s, connected: true }));
    };

    ws.onmessage = (event) => {
      const message: ServerMessage = JSON.parse(event.data);
      
      switch (message.type) {
        case 'frame':
          onFrameRef.current?.(message.data);
          break;
        case 'status':
          setState(s => ({
            ...s,
            status: message.status,
            takeoverMessage: message.message || ''
          }));
          break;
        case 'result':
          setState(s => ({ ...s, result: message.data }));
          break;
        case 'error':
          setState(s => ({ ...s, error: message.message }));
          break;
      }
    };

    ws.onclose = () => {
      setState(s => ({ ...s, connected: false }));
    };

    ws.onerror = () => {
      setState(s => ({ ...s, error: 'WebSocket connection error' }));
    };
  }, []);

  const createSession = useCallback(async () => {
    try {
      setState(s => ({ ...s, status: 'starting', error: '' }));
      const { sessionId } = await api.createSession();
      setState(s => ({ ...s, sessionId }));
      connect(sessionId);
    } catch (e) {
      setState(s => ({ ...s, error: (e as Error).message, status: 'error' }));
    }
  }, [connect]);

  const runScript = useCallback(async (code: string) => {
    if (!state.sessionId) return;
    
    try {
      setState(s => ({ ...s, error: '' }));
      await api.runScript(state.sessionId, code);
    } catch (e) {
      setState(s => ({ ...s, error: (e as Error).message }));
    }
  }, [state.sessionId]);

  const completeTakeover = useCallback(async () => {
    if (!state.sessionId) return;
    
    try {
      await api.completeTakeover(state.sessionId);
    } catch (e) {
      setState(s => ({ ...s, error: (e as Error).message }));
    }
  }, [state.sessionId]);

  const destroySession = useCallback(async () => {
    if (state.sessionId) {
      try {
        await api.destroySession(state.sessionId);
      } catch { /* cleanup */ }
    }
    wsRef.current?.close();
    wsRef.current = null;
    setState(INITIAL_STATE);
  }, [state.sessionId]);

  const sendInput = useCallback((message: ClientMessage) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message));
    }
  }, []);

  const setOnFrame = useCallback((callback: (data: string) => void) => {
    onFrameRef.current = callback;
  }, []);

  useEffect(() => {
    return () => {
      wsRef.current?.close();
    };
  }, []);

  return {
    ...state,
    createSession,
    runScript,
    completeTakeover,
    destroySession,
    sendInput,
    setOnFrame
  };
}
```

#### src/components/BrowserViewer.tsx
```tsx
'use client';

import { useRef, useEffect, useCallback } from 'react';
import { ClientMessage } from '@/lib/types';

interface BrowserViewerProps {
  onFrame: (callback: (data: string) => void) => void;
  sendInput: (message: ClientMessage) => void;
  isInteractive: boolean;
  viewportWidth?: number;
  viewportHeight?: number;
}

export function BrowserViewer({
  onFrame,
  sendInput,
  isInteractive,
  viewportWidth = 1280,
  viewportHeight = 720
}: BrowserViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Calculate scale for coordinate translation
  const getScale = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return { scaleX: 1, scaleY: 1 };
    const rect = canvas.getBoundingClientRect();
    return {
      scaleX: viewportWidth / rect.width,
      scaleY: viewportHeight / rect.height
    };
  }, [viewportWidth, viewportHeight]);

  // Handle incoming frames
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    onFrame((data: string) => {
      const img = new Image();
      img.onload = () => {
        ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
      };
      img.src = `data:image/jpeg;base64,${data}`;
    });
  }, [onFrame]);

  // Mouse event handlers
  const handleMouseEvent = useCallback((
    e: React.MouseEvent<HTMLCanvasElement>,
    action: string
  ) => {
    if (!isInteractive) return;

    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;

    const { scaleX, scaleY } = getScale();
    const x = Math.round((e.clientX - rect.left) * scaleX);
    const y = Math.round((e.clientY - rect.top) * scaleY);

    sendInput({
      type: 'mouse',
      action,
      x,
      y,
      button: e.button === 0 ? 'left' : e.button === 2 ? 'right' : 'middle'
    });
  }, [isInteractive, getScale, sendInput]);

  // Keyboard event handlers
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!isInteractive) return;
    e.preventDefault();

    sendInput({
      type: 'keyboard',
      action: 'keyDown',
      key: e.key,
      text: e.key.length === 1 ? e.key : undefined
    });

    // For printable characters, also send char event
    if (e.key.length === 1) {
      sendInput({
        type: 'keyboard',
        action: 'char',
        key: e.key,
        text: e.key
      });
    }
  }, [isInteractive, sendInput]);

  const handleKeyUp = useCallback((e: React.KeyboardEvent) => {
    if (!isInteractive) return;
    e.preventDefault();

    sendInput({
      type: 'keyboard',
      action: 'keyUp',
      key: e.key
    });
  }, [isInteractive, sendInput]);

  // Scroll handler
  const handleWheel = useCallback((e: React.WheelEvent<HTMLCanvasElement>) => {
    if (!isInteractive) return;
    e.preventDefault();

    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;

    const { scaleX, scaleY } = getScale();
    const x = Math.round((e.clientX - rect.left) * scaleX);
    const y = Math.round((e.clientY - rect.top) * scaleY);

    sendInput({
      type: 'scroll',
      x,
      y,
      deltaX: e.deltaX,
      deltaY: e.deltaY
    });
  }, [isInteractive, getScale, sendInput]);

  return (
    <div 
      ref={containerRef}
      className={`relative border rounded-lg overflow-hidden ${
        isInteractive ? 'ring-2 ring-blue-500' : ''
      }`}
    >
      <canvas
        ref={canvasRef}
        width={viewportWidth}
        height={viewportHeight}
        tabIndex={0}
        className="w-full h-auto cursor-default focus:outline-none"
        style={{ 
          cursor: isInteractive ? 'crosshair' : 'default',
          aspectRatio: `${viewportWidth} / ${viewportHeight}`
        }}
        onMouseDown={(e) => handleMouseEvent(e, 'mousePressed')}
        onMouseUp={(e) => handleMouseEvent(e, 'mouseReleased')}
        onMouseMove={(e) => handleMouseEvent(e, 'mouseMoved')}
        onWheel={handleWheel}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
        onContextMenu={(e) => e.preventDefault()}
      />
      {isInteractive && (
        <div className="absolute top-2 left-2 bg-blue-500 text-white px-2 py-1 rounded text-sm">
          Interactive Mode - Click and type to control
        </div>
      )}
    </div>
  );
}
```

#### src/components/ScriptEditor.tsx
```tsx
'use client';

import { useRef, useEffect } from 'react';
import Editor, { OnMount } from '@monaco-editor/react';

interface ScriptEditorProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

const DEFAULT_SCRIPT = `// Remote Browser Automation Script
// 
// Available:
//   page - Playwright Page object
//   requestTakeover(message) - Pause for user interaction
//
// Return a value to see results after completion.

// Navigate to a website
await page.goto('https://example.com');

// Example: Fill a form
// await page.fill('#email', 'user@example.com');

// When you need user input (login, captcha, 2FA):
// await requestTakeover('Please log in and solve the captcha');

// Extract data
const title = await page.title();

return { title, url: page.url() };
`;

export function ScriptEditor({ value, onChange, disabled }: ScriptEditorProps) {
  const handleMount: OnMount = (editor) => {
    editor.focus();
  };

  return (
    <div className="h-full border rounded-lg overflow-hidden">
      <Editor
        height="100%"
        defaultLanguage="typescript"
        value={value || DEFAULT_SCRIPT}
        onChange={(v) => onChange(v || '')}
        onMount={handleMount}
        theme="vs-dark"
        options={{
          minimap: { enabled: false },
          fontSize: 14,
          lineNumbers: 'on',
          scrollBeyondLastLine: false,
          wordWrap: 'on',
          readOnly: disabled,
          automaticLayout: true
        }}
      />
    </div>
  );
}

export { DEFAULT_SCRIPT };
```

#### src/components/StatusBar.tsx
```tsx
'use client';

import { SessionStatus } from '@/lib/types';

interface StatusBarProps {
  status: SessionStatus;
  takeoverMessage: string;
  error: string;
  result: unknown;
  connected: boolean;
  onDone: () => void;
}

const STATUS_LABELS: Record<SessionStatus, string> = {
  idle: 'Ready',
  starting: 'Starting browser...',
  running: 'Script running...',
  takeover: 'Waiting for you',
  done: 'Completed',
  error: 'Error'
};

const STATUS_COLORS: Record<SessionStatus, string> = {
  idle: 'bg-gray-500',
  starting: 'bg-yellow-500',
  running: 'bg-blue-500',
  takeover: 'bg-orange-500',
  done: 'bg-green-500',
  error: 'bg-red-500'
};

export function StatusBar({
  status,
  takeoverMessage,
  error,
  result,
  connected,
  onDone
}: StatusBarProps) {
  return (
    <div className="space-y-2">
      {/* Connection and Status */}
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${connected ? 'bg-green-500' : 'bg-red-500'}`} />
          <span className="text-sm text-gray-600">
            {connected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${STATUS_COLORS[status]}`} />
          <span className="text-sm font-medium">{STATUS_LABELS[status]}</span>
        </div>
      </div>

      {/* Takeover Message */}
      {status === 'takeover' && takeoverMessage && (
        <div className="flex items-center gap-4 p-3 bg-orange-50 border border-orange-200 rounded-lg">
          <span className="flex-1 text-orange-800">{takeoverMessage}</span>
          <button
            onClick={onDone}
            className="px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 font-medium"
          >
            I'm Done
          </button>
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="p-3 bg-red-50 border border-red-200 rounded-lg">
          <p className="text-red-800 font-medium">Error</p>
          <p className="text-red-600 text-sm mt-1">{error}</p>
        </div>
      )}

      {/* Result */}
      {status === 'done' && result && (
        <div className="p-3 bg-green-50 border border-green-200 rounded-lg">
          <p className="text-green-800 font-medium">Result</p>
          <pre className="text-green-600 text-sm mt-1 overflow-auto max-h-32">
            {JSON.stringify(result, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}
```

#### src/components/SessionControls.tsx
```tsx
'use client';

import { SessionStatus } from '@/lib/types';

interface SessionControlsProps {
  status: SessionStatus;
  sessionId: string | null;
  onCreateSession: () => void;
  onRunScript: () => void;
  onStop: () => void;
}

export function SessionControls({
  status,
  sessionId,
  onCreateSession,
  onRunScript,
  onStop
}: SessionControlsProps) {
  const canCreate = !sessionId || status === 'error' || status === 'done';
  const canRun = sessionId && (status === 'idle' || status === 'done');
  const canStop = sessionId && (status === 'running' || status === 'takeover' || status === 'starting');

  return (
    <div className="flex items-center gap-2">
      {canCreate && (
        <button
          onClick={onCreateSession}
          className="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 font-medium"
        >
          {sessionId ? 'New Session' : 'Start Session'}
        </button>
      )}
      {canRun && (
        <button
          onClick={onRunScript}
          className="px-4 py-2 bg-green-500 text-white rounded-lg hover:bg-green-600 font-medium flex items-center gap-2"
        >
          <span>▶</span> Run Script
        </button>
      )}
      {canStop && (
        <button
          onClick={onStop}
          className="px-4 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 font-medium flex items-center gap-2"
        >
          <span>⏹</span> Stop
        </button>
      )}
    </div>
  );
}
```

#### src/app/page.tsx
```tsx
'use client';

import { useState } from 'react';
import { useSession } from '@/hooks/useSession';
import { ScriptEditor, DEFAULT_SCRIPT } from '@/components/ScriptEditor';
import { BrowserViewer } from '@/components/BrowserViewer';
import { SessionControls } from '@/components/SessionControls';
import { StatusBar } from '@/components/StatusBar';

export default function Home() {
  const [script, setScript] = useState(DEFAULT_SCRIPT);
  const session = useSession();

  const isInteractive = session.status === 'takeover';
  const isRunning = ['starting', 'running', 'takeover'].includes(session.status);

  return (
    <main className="min-h-screen p-6 bg-gray-100">
      <div className="max-w-7xl mx-auto space-y-6">
        <header className="flex items-center justify-between">
          <h1 className="text-2xl font-bold text-gray-800">
            Remote Browser Automation
          </h1>
          <SessionControls
            status={session.status}
            sessionId={session.sessionId}
            onCreateSession={session.createSession}
            onRunScript={() => session.runScript(script)}
            onStop={session.destroySession}
          />
        </header>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Script Editor */}
          <div className="bg-white rounded-lg shadow-sm p-4">
            <h2 className="text-lg font-semibold mb-3 text-gray-700">Script</h2>
            <div className="h-[500px]">
              <ScriptEditor
                value={script}
                onChange={setScript}
                disabled={isRunning}
              />
            </div>
          </div>

          {/* Browser Viewer */}
          <div className="bg-white rounded-lg shadow-sm p-4">
            <h2 className="text-lg font-semibold mb-3 text-gray-700">Browser</h2>
            <BrowserViewer
              onFrame={session.setOnFrame}
              sendInput={session.sendInput}
              isInteractive={isInteractive}
            />
          </div>
        </div>

        {/* Status Bar */}
        <div className="bg-white rounded-lg shadow-sm p-4">
          <StatusBar
            status={session.status}
            takeoverMessage={session.takeoverMessage}
            error={session.error}
            result={session.result}
            connected={session.connected}
            onDone={session.completeTakeover}
          />
        </div>
      </div>
    </main>
  );
}
```

#### src/app/layout.tsx
```tsx
import type { Metadata } from 'next';
import { Inter } from 'next/font/google';
import './globals.css';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: 'Remote Browser Automation',
  description: 'Run Playwright scripts with live streaming and user takeover',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className={inter.className}>{children}</body>
    </html>
  );
}
```

#### src/app/globals.css
```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

#### package.json
```json
{
  "name": "remote-browser-frontend",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "next dev",
    "build": "next build",
    "start": "next start",
    "lint": "next lint"
  },
  "dependencies": {
    "@monaco-editor/react": "^4.6.0",
    "next": "14.0.0",
    "react": "^18.2.0",
    "react-dom": "^18.2.0"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "@types/react": "^18.2.0",
    "@types/react-dom": "^18.2.0",
    "autoprefixer": "^10.4.0",
    "postcss": "^8.4.0",
    "tailwindcss": "^3.4.0",
    "typescript": "^5.0.0"
  }
}
```

### CAPTCHA Solving
The system includes robust, multi-frame captcha solving (inspired by PEPR: https://github.com/berstend/puppeteer-extra/tree/master/packages/puppeteer-extra-plugin-recaptcha):
- **Source of Truth Detection**: Inspects internal library configurations (`___grecaptcha_cfg`) rather than just DOM elements.
- **Cross-Origin Support**: Automatically scans all iframes (including cross-origin ones like on Instagram) to find the captcha engine.
- **Precision Injection**: Targets the specific widget and triggers the exact callback function.
- **Visual Feedback**: Solved captchas are highlighted in the browser view.
- **Fallback**: Automatically falls back to user takeover if solving fails.

---

## Deployment

### Cloudflare Worker

```bash
cd worker
npm install
npx wrangler deploy
```

Set the API key secret:
```bash
npx wrangler secret put API_KEY
```

### Next.js Frontend (Vercel)

1. Push to GitHub
2. Connect repo to Vercel
3. Set environment variables:
   - `NEXT_PUBLIC_WORKER_URL`: Your deployed Worker URL
   - `WORKER_API_KEY`: The API key you set in the Worker

---

## Usage Flow

1. **Start Session** - Click "Start Session" to launch a browser
2. **Edit Script** - Modify the script in the editor
3. **Run Script** - Click "Run Script" to execute
4. **Takeover** - When script calls `requestTakeover()`:
   - Status changes to "Waiting for you"
   - Message displays what action is needed
   - User can click/type in the browser viewer
   - Click "I'm Done" when finished
5. **View Results** - Script results displayed at completion
6. **Stop** - Click "Stop" to end the session

---

## Sample Scripts

### Basic Navigation and Screenshot
```typescript
await page.goto('https://news.ycombinator.com');
const title = await page.title();
const stories = await page.evaluate(() => {
  return Array.from(document.querySelectorAll('.titleline > a'))
    .slice(0, 5)
    .map(a => ({ title: a.textContent, href: a.href }));
});
return { title, stories };
```

### Login with Takeover
```typescript
await page.goto('https://example.com/login');
await requestTakeover('Please enter your credentials and click Login');
await page.waitForURL('**/dashboard');
const userData = await page.evaluate(() => {
  return document.querySelector('.user-info')?.textContent;
});
return { loggedIn: true, user: userData };
```

### CAPTCHA Handling
```typescript
await page.goto('https://example.com/form');
await page.fill('#name', 'John Doe');
await page.fill('#email', 'john@example.com');
await requestTakeover('Please solve the CAPTCHA and submit the form');
await page.waitForSelector('.success-message');
return { submitted: true };
```

---

## Known Limitations

1. **Session Duration**: Max ~10 minutes due to Browser Rendering limits
2. **Screencast Quality**: Base64 JPEG frames can be bandwidth-heavy
3. **Script Security**: User scripts run in async context (not fully sandboxed)
4. **Single Tab**: One browser tab per session

## Future Improvements

- [ ] Add script templates dropdown
- [ ] Session history/logging
- [ ] Multiple tabs support
- [ ] Script syntax validation
- [ ] Viewport size configuration
- [ ] Screenshot download
- [ ] Recording/playback
