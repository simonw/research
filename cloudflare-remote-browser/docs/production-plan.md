# 🚀 Remote Browser Service: Production Plan

> **Goal**: Transform the prototype into a production-ready, stateless, secure browser automation service with precompiled scripts, excellent DX, and robust CAPTCHA handling.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Security Hardening](#2-security-hardening)
3. [Precompiled Scripts System](#3-precompiled-scripts-system)
4. [Stateless Session Management](#4-stateless-session-management)
5. [CAPTCHA Handling v2](#5-captcha-handling-v2)
6. [Local Development with Browser Rendering](#6-local-development-with-browser-rendering)
7. [API Design & Documentation](#7-api-design--documentation)
8. [Code Structure](#8-code-structure)
9. [Implementation Phases](#9-implementation-phases)

---

## 1. Architecture Overview

### Current (Problematic)
```
┌─────────────┐     ┌──────────────────────────────────────────┐
│   Frontend  │────▶│ CF Worker + Durable Object               │
│  (Executes  │     │ ┌──────────────────────────────────────┐ │
│   scripts   │◀────│ │ BrowserSession (holds browser state) │ │
│  via proxy) │ WS  │ │ - WebSocket clients                  │ │
└─────────────┘     │ │ - Network capture (unbounded)        │ │
                    │ │ - CAPTCHA solving                    │ │
                    │ └──────────────────────────────────────┘ │
                    └──────────────────────────────────────────┘
```

**Problems**: Security holes, resource leaks, stateful mess, code injection.

### Target (Production)
```
┌─────────────────────────────────────────────────────────────────────┐
│                         API Gateway                                  │
│   Authentication │ Rate Limiting │ Request Validation               │
└──────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Cloudflare Worker                                │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────────┐│
│  │ Session Manager│  │ Script Registry│  │ CAPTCHA Service        ││
│  │ - Create       │  │ - instagram    │  │ - 2captcha fallback    ││
│  │ - List         │  │ - linkedin     │  │ - Turnstile detection  ││
│  │ - Get          │  │ - twitter      │  │ - Human takeover       ││
│  │ - Delete       │  │ - custom       │  └────────────────────────┘│
│  └────────────────┘  └────────────────┘                             │
└──────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                 BrowserSession (Durable Object)                      │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌─────────────┐│
│  │ Browser/Page │ │ Script Exec  │ │ Event Stream │ │ Cleanup     ││
│  │ Playwright   │ │ Precompiled  │ │ WebSocket    │ │ Automatic   ││
│  │ Full API     │ │ Functions    │ │ Status only  │ │ Registry    ││
│  └──────────────┘ └──────────────┘ └──────────────┘ └─────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

**Key Changes**:
- Scripts run server-side (no proxy)
- WebSocket for events only (no commands)
- Stateless API with session tokens
- Automatic resource cleanup
- Proper authentication everywhere

---

## 2. Security Hardening

### 2.1 Authentication

**Session Tokens**: Each session gets a cryptographic token at creation.

```typescript
// worker/src/lib/auth.ts

interface SessionToken {
  sessionId: string;
  createdAt: number;
  expiresAt: number;
}

export function generateSessionToken(sessionId: string, env: Env): string {
  const payload: SessionToken = {
    sessionId,
    createdAt: Date.now(),
    expiresAt: Date.now() + (30 * 60 * 1000), // 30 minutes
  };
  
  // Sign with HMAC-SHA256
  const encoder = new TextEncoder();
  const data = encoder.encode(JSON.stringify(payload));
  const key = encoder.encode(env.SESSION_SECRET);
  
  // In production, use crypto.subtle.sign()
  return `${btoa(JSON.stringify(payload))}.${signHMAC(data, key)}`;
}

export async function validateSessionToken(
  token: string, 
  sessionId: string,
  env: Env
): Promise<boolean> {
  const [payloadB64, signature] = token.split('.');
  if (!payloadB64 || !signature) return false;
  
  const payload: SessionToken = JSON.parse(atob(payloadB64));
  
  // Validate session ID matches
  if (payload.sessionId !== sessionId) return false;
  
  // Validate not expired
  if (Date.now() > payload.expiresAt) return false;
  
  // Validate signature
  return verifyHMAC(payloadB64, signature, env.SESSION_SECRET);
}
```

### 2.2 All Requests Authenticated

```typescript
// worker/src/middleware/auth.ts

export async function authenticate(
  request: Request, 
  env: Env,
  sessionId?: string
): Promise<{ valid: boolean; error?: string }> {
  // 1. API Key authentication
  const authHeader = request.headers.get('Authorization');
  if (!authHeader?.startsWith('Bearer ')) {
    return { valid: false, error: 'Missing authorization header' };
  }
  
  const token = authHeader.slice(7);
  
  // For session-specific operations, validate session token
  if (sessionId) {
    const sessionToken = request.headers.get('X-Session-Token');
    if (!sessionToken) {
      return { valid: false, error: 'Missing session token' };
    }
    
    const valid = await validateSessionToken(sessionToken, sessionId, env);
    if (!valid) {
      return { valid: false, error: 'Invalid or expired session token' };
    }
  }
  
  // Validate API key
  if (token !== env.API_KEY) {
    return { valid: false, error: 'Invalid API key' };
  }
  
  return { valid: true };
}
```

### 2.3 CORS Restrictions

```typescript
// worker/src/lib/cors.ts

const ALLOWED_ORIGINS = new Set([
  'https://app.yourcompany.com',
  'https://dashboard.yourcompany.com',
]);

// Allow localhost in development
if (env.ENVIRONMENT === 'development') {
  ALLOWED_ORIGINS.add('http://localhost:3000');
}

export function getCorsHeaders(request: Request, env: Env): HeadersInit {
  const origin = request.headers.get('Origin');
  
  if (origin && ALLOWED_ORIGINS.has(origin)) {
    return {
      'Access-Control-Allow-Origin': origin,
      'Access-Control-Allow-Methods': 'GET, POST, DELETE, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization, X-Session-Token',
      'Access-Control-Max-Age': '86400',
    };
  }
  
  // No CORS headers for unknown origins
  return {};
}
```

### 2.4 Input Validation

```typescript
// worker/src/lib/validation.ts
import { z } from 'zod';

// Request size limits
export const MAX_REQUEST_SIZE = 1024 * 1024; // 1MB

export const RunScriptSchema = z.object({
  scriptId: z.string().min(1).max(64).regex(/^[a-z0-9-]+$/),
  params: z.record(z.unknown()).optional(),
  timeout: z.number().min(1000).max(600000).optional(), // 1s - 10min
  webhookUrl: z.string().url().optional(),
});

export const CreateSessionSchema = z.object({
  maxDuration: z.number().min(60000).max(900000).optional(), // 1-15 min
  viewport: z.object({
    width: z.number().min(320).max(1920),
    height: z.number().min(240).max(1080),
  }).optional(),
});

export function validateRequest<T>(
  schema: z.ZodSchema<T>,
  data: unknown
): { success: true; data: T } | { success: false; error: string } {
  const result = schema.safeParse(data);
  if (result.success) {
    return { success: true, data: result.data };
  }
  return { 
    success: false, 
    error: result.error.errors.map(e => `${e.path.join('.')}: ${e.message}`).join(', ')
  };
}
```

### 2.5 Error Responses (No Stack Traces)

```typescript
// worker/src/lib/errors.ts

export class APIError extends Error {
  constructor(
    public readonly code: string,
    public readonly message: string,
    public readonly status: number = 400,
    public readonly details?: Record<string, unknown>
  ) {
    super(message);
  }
}

export function errorResponse(error: unknown, env: Env): Response {
  // Log full error internally
  console.error('[Error]', error);
  
  if (error instanceof APIError) {
    return Response.json({
      error: {
        code: error.code,
        message: error.message,
        ...(error.details && { details: error.details }),
      }
    }, { status: error.status });
  }
  
  // Generic error for unknown exceptions (no stack trace!)
  return Response.json({
    error: {
      code: 'INTERNAL_ERROR',
      message: 'An unexpected error occurred',
    }
  }, { status: 500 });
}
```

---

## 3. Precompiled Scripts System

### 3.1 Script Interface

```typescript
// worker/src/scripts/types.ts

import { Page, Browser, CDPSession } from '@cloudflare/playwright';

export interface ScriptContext {
  // Browser access
  page: Page;
  browser: Browser;
  cdp: CDPSession;
  
  // Parameters passed by API caller
  params: Record<string, unknown>;
  
  // User interaction
  requestTakeover(message: string, options?: TakeoverOptions): Promise<void>;
  getInput(schema: InputSchema): Promise<Record<string, unknown>>;
  
  // Status updates (streamed to client)
  setStatus(message: string): void;
  setProgress(current: number, total: number): void;
  setData(key: string, value: unknown): void;
  
  // Network capture
  captureNetwork(options: CaptureOptions): void;
  getCapturedResponse(key: string): Promise<CapturedResponse | null>;
  waitForCapturedResponse(key: string, timeout?: number): Promise<CapturedResponse>;
  
  // CAPTCHA handling
  solveCaptcha(options?: CaptchaOptions): Promise<CaptchaResult>;
  
  // Logging
  log: Logger;
}

export interface TakeoverOptions {
  timeout?: number;           // Max wait time (default: 5 min)
  autoComplete?: {
    selector: string;         // Complete when this appears
    urlPattern?: string;      // Or when URL matches
  };
}

export type ScriptFunction<TParams = unknown, TResult = unknown> = (
  ctx: ScriptContext
) => Promise<TResult>;

export interface ScriptDefinition<TParams = unknown, TResult = unknown> {
  id: string;
  name: string;
  description: string;
  version: string;
  
  // JSON Schema for params validation
  paramsSchema?: JSONSchema;
  
  // The actual script
  execute: ScriptFunction<TParams, TResult>;
  
  // Optional hooks
  beforeExecute?: (ctx: ScriptContext) => Promise<void>;
  afterExecute?: (ctx: ScriptContext, result: TResult) => Promise<void>;
  onError?: (ctx: ScriptContext, error: Error) => Promise<void>;
}
```

### 3.2 Script Registry

```typescript
// worker/src/scripts/registry.ts

import { ScriptDefinition } from './types';
import { instagramScrape } from './instagram';
import { linkedinExport } from './linkedin';
import { twitterArchive } from './twitter';
import { genericNavigate } from './generic';

const SCRIPTS: Map<string, ScriptDefinition> = new Map();

// Register all scripts
[
  instagramScrape,
  linkedinExport,
  twitterArchive,
  genericNavigate,
].forEach(script => SCRIPTS.set(script.id, script));

export function getScript(id: string): ScriptDefinition | undefined {
  return SCRIPTS.get(id);
}

export function listScripts(): Array<{
  id: string;
  name: string;
  description: string;
  version: string;
  paramsSchema?: JSONSchema;
}> {
  return Array.from(SCRIPTS.values()).map(s => ({
    id: s.id,
    name: s.name,
    description: s.description,
    version: s.version,
    paramsSchema: s.paramsSchema,
  }));
}
```

### 3.3 Example Script: Instagram

```typescript
// worker/src/scripts/instagram/index.ts

import { ScriptDefinition, ScriptContext } from '../types';
import { fetchWebInfo, transformToSchema } from './helpers';

export const instagramScrape: ScriptDefinition<InstagramParams, InstagramResult> = {
  id: 'instagram-scrape',
  name: 'Instagram Profile Scraper',
  description: 'Scrapes profile data and posts from Instagram',
  version: '2.0.0',
  
  paramsSchema: {
    type: 'object',
    properties: {
      maxPosts: { type: 'number', minimum: 1, maximum: 500, default: 100 },
      includeStories: { type: 'boolean', default: false },
    },
  },
  
  async execute(ctx) {
    const { page, params } = ctx;
    const maxPosts = (params.maxPosts as number) || 100;
    
    // Navigate to Instagram
    ctx.setStatus('Navigating to Instagram...');
    await page.goto('https://www.instagram.com');
    await page.waitForTimeout(2000);
    
    // Check login status
    ctx.setStatus('Checking login status...');
    const webInfo = await fetchWebInfo(page);
    
    if (!webInfo?.username) {
      // Request user login
      await page.goto('https://www.instagram.com/accounts/login/');
      
      await ctx.requestTakeover('Please log in to Instagram', {
        timeout: 5 * 60 * 1000,
        autoComplete: {
          selector: '[data-testid="user-avatar"]',
        },
      });
      
      // Re-fetch after login
      const newWebInfo = await fetchWebInfo(page);
      if (!newWebInfo?.username) {
        throw new Error('Login failed - could not detect username');
      }
    }
    
    const username = webInfo?.username;
    ctx.setStatus(`Logged in as @${username}`);
    ctx.setData('username', username);
    
    // Set up network capture BEFORE navigating to profile
    ctx.captureNetwork({
      key: 'profile',
      urlPattern: /\/graphql/,
      bodyPattern: /PolarisProfilePageContentQuery/,
    });
    
    ctx.captureNetwork({
      key: 'posts',
      urlPattern: /\/graphql/,
      bodyPattern: /PolarisProfilePostsQuery/,
    });
    
    // Navigate to profile
    ctx.setStatus(`Navigating to profile...`);
    await page.goto(`https://www.instagram.com/${username}/`);
    await page.waitForTimeout(3000);
    
    // Wait for profile data
    ctx.setStatus('Waiting for profile data...');
    const profileResponse = await ctx.waitForCapturedResponse('profile', 30000);
    const profileData = extractProfileData(profileResponse);
    ctx.setData('profile', profileData);
    
    // Collect posts
    ctx.setStatus('Collecting posts...');
    const posts: Post[] = [];
    let hasMore = true;
    let scrollAttempts = 0;
    
    while (hasMore && posts.length < maxPosts && scrollAttempts < 50) {
      scrollAttempts++;
      ctx.setProgress(posts.length, maxPosts);
      
      // Try to get posts response
      const postsResponse = await ctx.getCapturedResponse('posts');
      if (postsResponse) {
        const { edges, pageInfo } = extractPosts(postsResponse);
        
        // Add unique posts
        for (const post of edges) {
          if (!posts.some(p => p.id === post.id)) {
            posts.push(post);
          }
        }
        
        hasMore = pageInfo.hasNextPage && posts.length < maxPosts;
      }
      
      if (hasMore) {
        // Scroll to load more
        await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
        await page.waitForTimeout(2000);
        
        // Re-register capture for next page
        ctx.captureNetwork({
          key: 'posts',
          urlPattern: /\/graphql/,
          bodyPattern: /PolarisProfilePostsQuery/,
        });
      }
    }
    
    // Build result
    const result = transformToSchema(profileData, posts, webInfo);
    
    ctx.setStatus(`Complete! Collected ${posts.length} posts for @${username}`);
    ctx.setData('result', result);
    
    return result;
  },
  
  async onError(ctx, error) {
    ctx.log.error('Instagram scrape failed', { error: error.message });
    ctx.setData('error', error.message);
  },
};
```

### 3.4 Script Execution Engine

```typescript
// worker/src/session/executor.ts

import { ScriptContext, ScriptDefinition } from '../scripts/types';
import { getScript } from '../scripts/registry';

export class ScriptExecutor {
  private session: BrowserSession;
  private abortController: AbortController;
  
  constructor(session: BrowserSession) {
    this.session = session;
    this.abortController = new AbortController();
  }
  
  async execute(
    scriptId: string, 
    params: Record<string, unknown>
  ): Promise<unknown> {
    const script = getScript(scriptId);
    if (!script) {
      throw new APIError('SCRIPT_NOT_FOUND', `Script '${scriptId}' not found`, 404);
    }
    
    // Validate params against schema
    if (script.paramsSchema) {
      const validation = validateAgainstSchema(params, script.paramsSchema);
      if (!validation.valid) {
        throw new APIError('INVALID_PARAMS', validation.errors.join(', '), 400);
      }
    }
    
    // Create script context
    const ctx = this.createContext(params);
    
    try {
      // Before hook
      if (script.beforeExecute) {
        await script.beforeExecute(ctx);
      }
      
      // Execute with timeout
      const timeout = (params.timeout as number) || 300000; // 5 min default
      const result = await Promise.race([
        script.execute(ctx),
        this.timeoutPromise(timeout),
        this.abortPromise(),
      ]);
      
      // After hook
      if (script.afterExecute) {
        await script.afterExecute(ctx, result);
      }
      
      return result;
    } catch (error) {
      // Error hook
      if (script.onError) {
        await script.onError(ctx, error as Error);
      }
      throw error;
    }
  }
  
  abort(): void {
    this.abortController.abort();
  }
  
  private createContext(params: Record<string, unknown>): ScriptContext {
    return {
      page: this.session.page!,
      browser: this.session.browser!,
      cdp: this.session.cdp!,
      params,
      
      requestTakeover: (msg, opts) => this.session.requestTakeover(msg, opts),
      getInput: (schema) => this.session.getInput(schema),
      
      setStatus: (msg) => this.session.broadcast({ type: 'status', message: msg }),
      setProgress: (cur, total) => this.session.broadcast({ type: 'progress', current: cur, total }),
      setData: (k, v) => this.session.broadcast({ type: 'data', key: k, value: v }),
      
      captureNetwork: (opts) => this.session.captureNetwork(opts),
      getCapturedResponse: (key) => this.session.getCapturedResponse(key),
      waitForCapturedResponse: (key, timeout) => this.session.waitForCapturedResponse(key, timeout),
      
      solveCaptcha: (opts) => this.session.solveCaptcha(opts),
      
      log: this.session.logger,
    };
  }
  
  private timeoutPromise(ms: number): Promise<never> {
    return new Promise((_, reject) => {
      setTimeout(() => reject(new APIError('TIMEOUT', 'Script execution timed out')), ms);
    });
  }
  
  private abortPromise(): Promise<never> {
    return new Promise((_, reject) => {
      this.abortController.signal.addEventListener('abort', () => {
        reject(new APIError('ABORTED', 'Script execution was aborted'));
      });
    });
  }
}
```

---

## 4. Stateless Session Management

### 4.1 Session Lifecycle

```
CREATE ─────► INITIALIZING ─────► READY ─────► RUNNING ─────► COMPLETED
    │              │                │             │              │
    │              ▼                │             ▼              │
    │           ERROR ◄─────────────┴──────── TAKEOVER           │
    │              │                                             │
    └──────────────┴─────────────────────────────────────────────┘
                                   │
                                   ▼
                               CLEANUP ─────► DESTROYED
```

### 4.2 Session Manager (Stateless Router)

```typescript
// worker/src/session/manager.ts

import { SessionRegistry } from './registry';
import { generateSessionToken } from '../lib/auth';

export interface SessionInfo {
  sessionId: string;
  token: string;
  status: SessionStatus;
  createdAt: number;
  expiresAt: number;
  script?: {
    id: string;
    startedAt?: number;
  };
}

export class SessionManager {
  private env: Env;
  
  constructor(env: Env) {
    this.env = env;
  }
  
  async createSession(options: CreateSessionOptions = {}): Promise<SessionInfo> {
    const sessionId = crypto.randomUUID();
    const token = generateSessionToken(sessionId, this.env);
    
    // Get Durable Object stub
    const stub = this.env.BROWSER_SESSION.get(
      this.env.BROWSER_SESSION.idFromName(sessionId)
    );
    
    // Initialize the session
    const response = await stub.fetch(new Request('http://internal/init', {
      method: 'POST',
      body: JSON.stringify(options),
      headers: { 'Content-Type': 'application/json' },
    }));
    
    if (!response.ok) {
      const error = await response.json();
      throw new APIError('SESSION_INIT_FAILED', error.message, 500);
    }
    
    // Register session
    await this.getRegistry().add(sessionId);
    
    const now = Date.now();
    return {
      sessionId,
      token,
      status: 'ready',
      createdAt: now,
      expiresAt: now + (options.maxDuration || 900000), // 15 min default
    };
  }
  
  async listSessions(): Promise<SessionInfo[]> {
    const sessions = await this.getRegistry().list();
    
    // Fetch status for each session (in parallel)
    const statuses = await Promise.all(
      sessions.map(async (s) => {
        try {
          const stub = this.env.BROWSER_SESSION.get(
            this.env.BROWSER_SESSION.idFromName(s.sessionId)
          );
          const response = await stub.fetch(new Request('http://internal/status'));
          return { sessionId: s.sessionId, ...(await response.json()) };
        } catch {
          return { sessionId: s.sessionId, status: 'unknown' };
        }
      })
    );
    
    return statuses;
  }
  
  async getSession(sessionId: string): Promise<SessionInfo> {
    const stub = this.env.BROWSER_SESSION.get(
      this.env.BROWSER_SESSION.idFromName(sessionId)
    );
    
    const response = await stub.fetch(new Request('http://internal/status'));
    if (!response.ok) {
      throw new APIError('SESSION_NOT_FOUND', 'Session not found', 404);
    }
    
    return response.json();
  }
  
  async deleteSession(sessionId: string): Promise<void> {
    const stub = this.env.BROWSER_SESSION.get(
      this.env.BROWSER_SESSION.idFromName(sessionId)
    );
    
    // Destroy the session (cleanup browser, WebSocket, etc.)
    await stub.fetch(new Request('http://internal/destroy', { method: 'POST' }));
    
    // Remove from registry
    await this.getRegistry().remove(sessionId);
  }
  
  private getRegistry(): SessionRegistry {
    return this.env.SESSION_REGISTRY.get(
      this.env.SESSION_REGISTRY.idFromName('global')
    );
  }
}
```

### 4.3 Registry with Auto-Cleanup

```typescript
// worker/src/session/registry.ts

interface RegistryEntry {
  sessionId: string;
  createdAt: number;
  expiresAt: number;
  lastActivity: number;
}

export class SessionRegistry implements DurableObject {
  private state: DurableObjectState;
  private sessions: Map<string, RegistryEntry> = new Map();
  
  constructor(state: DurableObjectState, env: Env) {
    this.state = state;
    
    // Load sessions from storage on init
    state.blockConcurrencyWhile(async () => {
      const stored = await state.storage.get<Map<string, RegistryEntry>>('sessions');
      if (stored) {
        this.sessions = stored;
      }
      
      // Schedule cleanup alarm
      await this.scheduleCleanup();
    });
  }
  
  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;
    
    if (path === '/add' && request.method === 'POST') {
      const { sessionId, expiresAt } = await request.json();
      await this.add(sessionId, expiresAt);
      return Response.json({ ok: true });
    }
    
    if (path === '/remove' && request.method === 'POST') {
      const { sessionId } = await request.json();
      await this.remove(sessionId);
      return Response.json({ ok: true });
    }
    
    if (path === '/list' && request.method === 'GET') {
      return Response.json({ sessions: this.list() });
    }
    
    if (path === '/heartbeat' && request.method === 'POST') {
      const { sessionId } = await request.json();
      await this.heartbeat(sessionId);
      return Response.json({ ok: true });
    }
    
    return new Response('Not Found', { status: 404 });
  }
  
  async alarm(): Promise<void> {
    const now = Date.now();
    const expired: string[] = [];
    
    for (const [sessionId, entry] of this.sessions) {
      if (now > entry.expiresAt) {
        expired.push(sessionId);
      }
    }
    
    // Clean up expired sessions
    for (const sessionId of expired) {
      this.sessions.delete(sessionId);
      
      // Notify the session to clean up
      try {
        const stub = this.env.BROWSER_SESSION.get(
          this.env.BROWSER_SESSION.idFromName(sessionId)
        );
        await stub.fetch(new Request('http://internal/destroy', { method: 'POST' }));
      } catch {
        // Session may already be gone
      }
    }
    
    await this.persist();
    await this.scheduleCleanup();
  }
  
  private async add(sessionId: string, expiresAt?: number): Promise<void> {
    const now = Date.now();
    this.sessions.set(sessionId, {
      sessionId,
      createdAt: now,
      expiresAt: expiresAt || now + 900000,
      lastActivity: now,
    });
    await this.persist();
  }
  
  private async remove(sessionId: string): Promise<void> {
    this.sessions.delete(sessionId);
    await this.persist();
  }
  
  private list(): RegistryEntry[] {
    return Array.from(this.sessions.values())
      .sort((a, b) => b.createdAt - a.createdAt);
  }
  
  private async heartbeat(sessionId: string): Promise<void> {
    const entry = this.sessions.get(sessionId);
    if (entry) {
      entry.lastActivity = Date.now();
      await this.persist();
    }
  }
  
  private async persist(): Promise<void> {
    await this.state.storage.put('sessions', this.sessions);
  }
  
  private async scheduleCleanup(): Promise<void> {
    // Run cleanup every minute
    await this.state.storage.setAlarm(Date.now() + 60000);
  }
}
```

### 4.4 Session Cleanup (Fixed)

```typescript
// worker/src/session/cleanup.ts

export async function cleanupSession(session: BrowserSession): Promise<void> {
  const errors: Error[] = [];
  
  // 1. Stop screencast
  if (session.cdp) {
    try {
      await session.cdp.send('Page.stopScreencast');
    } catch (e) {
      errors.push(e as Error);
    }
    
    try {
      await session.cdp.send('Network.disable');
    } catch (e) {
      errors.push(e as Error);
    }
  }
  
  // 2. Close browser
  if (session.browser) {
    try {
      await session.browser.close();
    } catch (e) {
      errors.push(e as Error);
    }
  }
  
  // 3. Clear all data structures
  session.networkRequests.clear();
  session.capturePatterns.clear();
  session.capturedResponses.clear();
  session.automationData = {};
  
  // 4. Reject pending promises
  for (const resolver of session.inputResolvers.values()) {
    resolver.reject(new Error('Session closed'));
  }
  session.inputResolvers.clear();
  
  if (session.takeoverResolver) {
    session.takeoverResolver();
    session.takeoverResolver = null;
  }
  
  // 5. Close WebSocket connections
  for (const ws of session.wsConnections) {
    try {
      ws.close(1000, 'Session closed');
    } catch (e) {
      errors.push(e as Error);
    }
  }
  session.wsConnections.clear();
  
  // 6. Nullify references
  session.browser = null;
  session.page = null;
  session.cdp = null;
  session.status = 'destroyed';
  
  // 7. Notify registry to remove this session
  try {
    await session.notifyRegistryRemoval();
  } catch (e) {
    errors.push(e as Error);
  }
  
  // Log any cleanup errors
  if (errors.length > 0) {
    console.error('[cleanup] Errors during cleanup:', errors);
  }
}
```

---

## 5. CAPTCHA Handling v2

### 5.1 Improved Detection

```typescript
// worker/src/captcha/detector.ts

export interface CaptchaInfo {
  type: 'recaptcha_v2' | 'recaptcha_v3' | 'hcaptcha' | 'turnstile' | 'unknown';
  sitekey: string;
  pageUrl: string;
  isEnterprise: boolean;
  isInvisible: boolean;
  frameUrl?: string;
  widgetId?: string;
  callback?: string;
}

export async function detectCaptcha(page: Page): Promise<CaptchaInfo | null> {
  // Check all frames (including iframes)
  for (const frame of page.frames()) {
    try {
      const info = await frame.evaluate(detectCaptchaInFrame);
      if (info) {
        return { ...info, frameUrl: frame.url() };
      }
    } catch {
      // Frame may be cross-origin or detached
      continue;
    }
  }
  
  return null;
}

function detectCaptchaInFrame(): CaptchaInfo | null {
  const pageUrl = window.location.href;
  
  // 1. reCAPTCHA - Check ___grecaptcha_cfg (source of truth)
  const cfg = (window as any).___grecaptcha_cfg;
  if (cfg?.clients) {
    for (const id in cfg.clients) {
      const client = cfg.clients[id];
      const sitekey = findInObject(client, 'sitekey') || findInObject(client, 'googlekey');
      
      if (sitekey) {
        const isEnterprise = 
          !!document.querySelector('script[src*="enterprise"]') ||
          !!document.querySelector('iframe[src*="/enterprise/"]');
        
        const callback = findInObject(client, 'callback');
        const size = findInObject(client, 'size');
        
        return {
          type: isEnterprise ? 'recaptcha_v3' : 'recaptcha_v2',
          sitekey,
          pageUrl,
          isEnterprise,
          isInvisible: size === 'invisible',
          widgetId: id,
          callback: typeof callback === 'function' ? callback.name : callback,
        };
      }
    }
  }
  
  // 2. hCaptcha
  const hcaptchaEl = document.querySelector('.h-captcha[data-sitekey]');
  if (hcaptchaEl) {
    return {
      type: 'hcaptcha',
      sitekey: hcaptchaEl.getAttribute('data-sitekey')!,
      pageUrl,
      isEnterprise: false,
      isInvisible: hcaptchaEl.getAttribute('data-size') === 'invisible',
    };
  }
  
  // 3. Cloudflare Turnstile
  const turnstileEl = document.querySelector('.cf-turnstile[data-sitekey]');
  if (turnstileEl) {
    return {
      type: 'turnstile',
      sitekey: turnstileEl.getAttribute('data-sitekey')!,
      pageUrl,
      isEnterprise: false,
      isInvisible: false,
    };
  }
  
  return null;
}

function findInObject(obj: any, key: string, maxDepth = 5): any {
  if (!obj || typeof obj !== 'object' || maxDepth <= 0) return null;
  if (key in obj && obj[key] != null) return obj[key];
  
  for (const k in obj) {
    const found = findInObject(obj[k], key, maxDepth - 1);
    if (found != null) return found;
  }
  return null;
}
```

### 5.2 Multi-Provider Solver

```typescript
// worker/src/captcha/solver.ts

import { Solver } from '@2captcha/captcha-solver';

export interface CaptchaResult {
  success: boolean;
  token?: string;
  method: 'auto' | 'human' | 'skipped';
  duration?: number;
  error?: string;
}

export class CaptchaSolver {
  private env: Env;
  private session: BrowserSession;
  
  constructor(session: BrowserSession, env: Env) {
    this.session = session;
    this.env = env;
  }
  
  async solve(options?: CaptchaOptions): Promise<CaptchaResult> {
    const startTime = Date.now();
    
    // 1. Detect CAPTCHA
    const info = await detectCaptcha(this.session.page!);
    if (!info) {
      return { success: true, method: 'skipped' };
    }
    
    this.session.broadcast({ 
      type: 'status', 
      message: `Detected ${info.type} CAPTCHA` 
    });
    
    // 2. Try automatic solving if API key is configured
    if (this.env.TWOCAPTCHA_API_KEY) {
      try {
        const token = await this.solveWithProvider(info);
        await this.injectToken(token, info);
        
        return {
          success: true,
          token: token.substring(0, 20) + '...',
          method: 'auto',
          duration: Date.now() - startTime,
        };
      } catch (error) {
        console.error('[CaptchaSolver] Auto-solve failed:', error);
        // Fall through to human fallback
      }
    }
    
    // 3. Fall back to human takeover
    this.session.broadcast({ 
      type: 'status', 
      message: 'Please solve the CAPTCHA manually' 
    });
    
    await this.session.requestTakeover(
      'Please solve the security challenge and continue',
      { timeout: options?.humanTimeout || 5 * 60 * 1000 }
    );
    
    return {
      success: true,
      method: 'human',
      duration: Date.now() - startTime,
    };
  }
  
  private async solveWithProvider(info: CaptchaInfo): Promise<string> {
    const solver = new Solver(this.env.TWOCAPTCHA_API_KEY!);
    
    switch (info.type) {
      case 'recaptcha_v2':
      case 'recaptcha_v3': {
        const result = await solver.recaptcha({
          googlekey: info.sitekey,
          pageurl: info.pageUrl,
          enterprise: info.isEnterprise,
          invisible: info.isInvisible,
        });
        return result.data;
      }
      
      case 'hcaptcha': {
        const result = await solver.hcaptcha({
          sitekey: info.sitekey,
          pageurl: info.pageUrl,
        });
        return result.data;
      }
      
      case 'turnstile': {
        const result = await solver.cloudflareTurnstile({
          sitekey: info.sitekey,
          pageurl: info.pageUrl,
        });
        return result.data;
      }
      
      default:
        throw new Error(`Unsupported CAPTCHA type: ${info.type}`);
    }
  }
  
  private async injectToken(token: string, info: CaptchaInfo): Promise<void> {
    const page = this.session.page!;
    
    // Find the right frame
    const targetFrame = info.frameUrl 
      ? page.frames().find(f => f.url() === info.frameUrl)
      : page.mainFrame();
    
    if (!targetFrame) {
      throw new Error('Could not find target frame for token injection');
    }
    
    await targetFrame.evaluate(({ token, info }) => {
      // Fill textarea fields
      const textareas = [
        'g-recaptcha-response',
        'h-captcha-response', 
        'cf-turnstile-response',
      ];
      
      for (const name of textareas) {
        const el = document.querySelector(`[name="${name}"], #${name}`) as HTMLTextAreaElement;
        if (el) {
          el.value = token;
          el.innerHTML = token;
          el.dispatchEvent(new Event('input', { bubbles: true }));
          el.dispatchEvent(new Event('change', { bubbles: true }));
        }
      }
      
      // Trigger callback if available
      if (info.widgetId != null) {
        const cfg = (window as any).___grecaptcha_cfg;
        const client = cfg?.clients?.[info.widgetId];
        if (client) {
          for (const key in client) {
            if (typeof client[key]?.callback === 'function') {
              client[key].callback(token);
              break;
            }
          }
        }
      }
      
      // Visual feedback
      const iframes = document.querySelectorAll('iframe');
      iframes.forEach(iframe => {
        if (iframe.src.includes('recaptcha') || 
            iframe.src.includes('hcaptcha') || 
            iframe.src.includes('turnstile')) {
          iframe.style.filter = 'opacity(60%) hue-rotate(120deg)';
        }
      });
    }, { token, info });
  }
}
```

---

## 6. Local Development with Browser Rendering

### 6.1 Wrangler Configuration

```toml
# worker/wrangler.toml

name = "remote-browser"
main = "src/index.ts"
compatibility_date = "2025-09-17"
compatibility_flags = ["nodejs_compat"]

# Browser Rendering binding - works locally with npx wrangler dev!
[browser]
binding = "BROWSER"

# Durable Objects
[[durable_objects.bindings]]
name = "BROWSER_SESSION"
class_name = "BrowserSession"

[[durable_objects.bindings]]
name = "SESSION_REGISTRY"
class_name = "SessionRegistry"

[[migrations]]
tag = "v1"
new_classes = ["BrowserSession", "SessionRegistry"]

# Environment variables
[vars]
ENVIRONMENT = "development"
LOG_LEVEL = "debug"

# Secrets (set via wrangler secret put)
# API_KEY
# SESSION_SECRET
# TWOCAPTCHA_API_KEY

[observability]
enabled = true
head_sampling_rate = 1
```

### 6.2 Local Script Testing

```typescript
// scripts/test-local.ts
// Run with: npx tsx scripts/test-local.ts

import { chromium } from 'playwright';
import { instagramScrape } from '../worker/src/scripts/instagram';

async function testScript() {
  const browser = await chromium.launch({ 
    headless: false, // See what's happening
    slowMo: 100,     // Slow down for debugging
  });
  
  const page = await browser.newPage();
  
  // Create mock context that matches ScriptContext
  const ctx = {
    page,
    browser,
    cdp: await page.context().newCDPSession(page),
    params: { maxPosts: 10 },
    
    // Mock implementations
    requestTakeover: async (msg: string) => {
      console.log(`[TAKEOVER] ${msg}`);
      console.log('Press Enter to continue...');
      await new Promise(r => process.stdin.once('data', r));
    },
    
    getInput: async (schema: any) => {
      console.log('[INPUT]', JSON.stringify(schema, null, 2));
      return {};
    },
    
    setStatus: (msg: string) => console.log(`[STATUS] ${msg}`),
    setProgress: (cur: number, total: number) => console.log(`[PROGRESS] ${cur}/${total}`),
    setData: (key: string, value: unknown) => console.log(`[DATA] ${key}:`, value),
    
    captureNetwork: () => {},
    getCapturedResponse: async () => null,
    waitForCapturedResponse: async () => ({}),
    
    solveCaptcha: async () => ({ success: true, method: 'skipped' as const }),
    
    log: console,
  };
  
  try {
    const result = await instagramScrape.execute(ctx as any);
    console.log('\n[RESULT]', JSON.stringify(result, null, 2));
  } catch (error) {
    console.error('[ERROR]', error);
  } finally {
    await browser.close();
  }
}

testScript();
```

### 6.3 Development Commands

```json
// worker/package.json

{
  "name": "remote-browser-worker",
  "scripts": {
    "dev": "wrangler dev --local",
    "dev:remote": "wrangler dev",
    "deploy": "wrangler deploy",
    "deploy:staging": "wrangler deploy --env staging",
    "deploy:production": "wrangler deploy --env production",
    
    "test:scripts": "tsx scripts/test-local.ts",
    "test:instagram": "tsx scripts/test-instagram.ts",
    "test:linkedin": "tsx scripts/test-linkedin.ts",
    
    "types": "wrangler types",
    "lint": "eslint src/",
    "typecheck": "tsc --noEmit"
  }
}
```

### 6.4 Script Development Workflow

```bash
# 1. Develop script locally with real browser
npm run test:instagram

# 2. Run worker locally with local browser
npm run dev

# 3. Test API calls against local worker
curl http://localhost:8787/sessions \
  -X POST \
  -H "Authorization: Bearer dev-key" \
  -H "Content-Type: application/json"

# 4. Deploy to staging
npm run deploy:staging

# 5. Test against staging
curl https://remote-browser-staging.company.workers.dev/sessions \
  -X POST \
  -H "Authorization: Bearer $STAGING_API_KEY"

# 6. Deploy to production
npm run deploy:production
```

---

## 7. API Design & Documentation

### 7.1 OpenAPI Specification

```yaml
# api/openapi.yaml

openapi: 3.1.0
info:
  title: Remote Browser API
  version: 2.0.0
  description: |
    Browser automation service powered by Cloudflare Browser Rendering.
    
    ## Authentication
    All requests require a Bearer token in the Authorization header:
    ```
    Authorization: Bearer your-api-key
    ```
    
    Session-specific operations also require an X-Session-Token header.

servers:
  - url: https://remote-browser.company.workers.dev
    description: Production
  - url: https://remote-browser-staging.company.workers.dev
    description: Staging

paths:
  /sessions:
    get:
      summary: List active sessions
      operationId: listSessions
      responses:
        '200':
          description: List of sessions
          content:
            application/json:
              schema:
                type: object
                properties:
                  sessions:
                    type: array
                    items:
                      $ref: '#/components/schemas/Session'
    
    post:
      summary: Create a new session
      operationId: createSession
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/CreateSessionRequest'
      responses:
        '201':
          description: Session created
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/SessionCreated'

  /sessions/{sessionId}:
    get:
      summary: Get session status
      operationId: getSession
      parameters:
        - $ref: '#/components/parameters/sessionId'
        - $ref: '#/components/parameters/sessionToken'
      responses:
        '200':
          description: Session details
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Session'
    
    delete:
      summary: Delete session
      operationId: deleteSession
      parameters:
        - $ref: '#/components/parameters/sessionId'
        - $ref: '#/components/parameters/sessionToken'
      responses:
        '204':
          description: Session deleted

  /sessions/{sessionId}/run:
    post:
      summary: Run a script
      operationId: runScript
      parameters:
        - $ref: '#/components/parameters/sessionId'
        - $ref: '#/components/parameters/sessionToken'
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/RunScriptRequest'
      responses:
        '202':
          description: Script execution started
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ScriptStarted'

  /sessions/{sessionId}/ws:
    get:
      summary: WebSocket connection for real-time events
      operationId: connectWebSocket
      description: |
        Connect via WebSocket to receive real-time events:
        - `status`: Status updates
        - `progress`: Progress updates
        - `data`: Data updates
        - `frame`: Screen frames (base64 JPEG)
        - `takeover`: User takeover requests
        - `result`: Script completion
        - `error`: Error events

  /scripts:
    get:
      summary: List available scripts
      operationId: listScripts
      responses:
        '200':
          description: Available scripts
          content:
            application/json:
              schema:
                type: object
                properties:
                  scripts:
                    type: array
                    items:
                      $ref: '#/components/schemas/ScriptInfo'

components:
  schemas:
    Session:
      type: object
      properties:
        sessionId:
          type: string
          format: uuid
        status:
          type: string
          enum: [initializing, ready, running, takeover, completed, error, destroyed]
        createdAt:
          type: integer
          format: int64
        expiresAt:
          type: integer
          format: int64
        script:
          type: object
          properties:
            id:
              type: string
            startedAt:
              type: integer
            progress:
              type: object
              properties:
                current:
                  type: integer
                total:
                  type: integer

    CreateSessionRequest:
      type: object
      properties:
        maxDuration:
          type: integer
          minimum: 60000
          maximum: 900000
          default: 900000
          description: Maximum session duration in milliseconds
        viewport:
          type: object
          properties:
            width:
              type: integer
              minimum: 320
              maximum: 1920
              default: 1280
            height:
              type: integer
              minimum: 240
              maximum: 1080
              default: 720

    SessionCreated:
      type: object
      required:
        - sessionId
        - token
        - wsUrl
      properties:
        sessionId:
          type: string
          format: uuid
        token:
          type: string
          description: Session token for subsequent requests
        wsUrl:
          type: string
          format: uri
          description: WebSocket URL for real-time events
        expiresAt:
          type: integer
          format: int64

    RunScriptRequest:
      type: object
      required:
        - scriptId
      properties:
        scriptId:
          type: string
          description: ID of the script to run
        params:
          type: object
          description: Parameters to pass to the script
        timeout:
          type: integer
          minimum: 1000
          maximum: 600000
          description: Script timeout in milliseconds
        webhookUrl:
          type: string
          format: uri
          description: URL to call when script completes

    ScriptInfo:
      type: object
      properties:
        id:
          type: string
        name:
          type: string
        description:
          type: string
        version:
          type: string
        paramsSchema:
          type: object

  parameters:
    sessionId:
      name: sessionId
      in: path
      required: true
      schema:
        type: string
        format: uuid
    
    sessionToken:
      name: X-Session-Token
      in: header
      required: true
      schema:
        type: string
```

### 7.2 SDK Generation

```bash
# Generate TypeScript SDK from OpenAPI
npx openapi-typescript-codegen \
  --input api/openapi.yaml \
  --output sdk/typescript/src \
  --name RemoteBrowserClient

# Generate Python SDK
openapi-generator generate \
  -i api/openapi.yaml \
  -g python \
  -o sdk/python
```

### 7.3 SDK Usage Example

```typescript
// Example: Using the generated SDK

import { RemoteBrowserClient } from '@company/remote-browser-sdk';

const client = new RemoteBrowserClient({
  baseUrl: 'https://remote-browser.company.workers.dev',
  apiKey: process.env.REMOTE_BROWSER_API_KEY,
});

async function scrapeInstagram() {
  // Create session
  const { sessionId, token, wsUrl } = await client.sessions.create({
    maxDuration: 600000, // 10 minutes
  });
  
  // Connect WebSocket for real-time updates
  const ws = new WebSocket(wsUrl);
  ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    switch (msg.type) {
      case 'status':
        console.log('Status:', msg.message);
        break;
      case 'progress':
        console.log(`Progress: ${msg.current}/${msg.total}`);
        break;
      case 'takeover':
        console.log('User action required:', msg.message);
        // Open browser UI for user
        break;
      case 'result':
        console.log('Result:', msg.data);
        break;
    }
  };
  
  // Run script
  await client.sessions.runScript(sessionId, token, {
    scriptId: 'instagram-scrape',
    params: { maxPosts: 100 },
  });
  
  // Wait for completion (or handle via WebSocket)
  const result = await client.sessions.waitForCompletion(sessionId, token);
  
  // Cleanup
  await client.sessions.delete(sessionId, token);
  ws.close();
  
  return result;
}
```

---

## 8. Code Structure

### 8.1 Directory Layout

```
worker/
├── src/
│   ├── index.ts                 # Worker entrypoint
│   ├── router.ts                # Request routing
│   │
│   ├── middleware/
│   │   ├── auth.ts              # Authentication
│   │   ├── cors.ts              # CORS handling
│   │   ├── rateLimit.ts         # Rate limiting
│   │   └── validation.ts        # Request validation
│   │
│   ├── session/
│   │   ├── manager.ts           # Session lifecycle
│   │   ├── registry.ts          # Session registry (DO)
│   │   ├── session.ts           # BrowserSession (DO)
│   │   ├── executor.ts          # Script execution
│   │   ├── cleanup.ts           # Resource cleanup
│   │   └── types.ts             # Session types
│   │
│   ├── scripts/
│   │   ├── types.ts             # Script interface
│   │   ├── registry.ts          # Script registry
│   │   ├── instagram/
│   │   │   ├── index.ts         # Instagram script
│   │   │   ├── helpers.ts       # Instagram helpers
│   │   │   └── types.ts         # Instagram types
│   │   ├── linkedin/
│   │   │   └── index.ts
│   │   └── generic/
│   │       └── index.ts
│   │
│   ├── captcha/
│   │   ├── detector.ts          # CAPTCHA detection
│   │   ├── solver.ts            # CAPTCHA solving
│   │   └── types.ts
│   │
│   ├── network/
│   │   ├── capture.ts           # Network capture
│   │   └── types.ts
│   │
│   ├── lib/
│   │   ├── auth.ts              # Token generation
│   │   ├── errors.ts            # Error handling
│   │   ├── logger.ts            # Structured logging
│   │   └── lru-cache.ts         # LRU cache
│   │
│   └── types/
│       ├── env.d.ts             # Environment types
│       └── messages.ts          # WebSocket messages
│
├── scripts/                      # Local testing scripts
│   ├── test-local.ts
│   ├── test-instagram.ts
│   └── test-linkedin.ts
│
├── api/
│   └── openapi.yaml             # API specification
│
├── wrangler.toml
├── package.json
└── tsconfig.json
```

### 8.2 Clean Module Boundaries

```typescript
// worker/src/index.ts

import { Router } from './router';
import { authenticate } from './middleware/auth';
import { getCorsHeaders } from './middleware/cors';
import { errorResponse } from './lib/errors';

export { BrowserSession } from './session/session';
export { SessionRegistry } from './session/registry';

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const corsHeaders = getCorsHeaders(request, env);
    
    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }
    
    try {
      // Route request
      const router = new Router(env);
      const response = await router.handle(request);
      
      // Add CORS headers to response
      const headers = new Headers(response.headers);
      Object.entries(corsHeaders).forEach(([k, v]) => headers.set(k, v));
      
      return new Response(response.body, {
        status: response.status,
        headers,
      });
    } catch (error) {
      return errorResponse(error, env);
    }
  },
};
```

```typescript
// worker/src/router.ts

import { SessionManager } from './session/manager';
import { listScripts, getScript } from './scripts/registry';
import { authenticate } from './middleware/auth';
import { validateRequest, CreateSessionSchema, RunScriptSchema } from './middleware/validation';
import { APIError } from './lib/errors';

export class Router {
  private env: Env;
  private sessions: SessionManager;
  
  constructor(env: Env) {
    this.env = env;
    this.sessions = new SessionManager(env);
  }
  
  async handle(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;
    const method = request.method;
    
    // Public routes
    if (path === '/health' && method === 'GET') {
      return Response.json({ status: 'ok', timestamp: Date.now() });
    }
    
    // Authenticate
    const auth = await authenticate(request, this.env);
    if (!auth.valid) {
      throw new APIError('UNAUTHORIZED', auth.error!, 401);
    }
    
    // Scripts (read-only)
    if (path === '/scripts' && method === 'GET') {
      return Response.json({ scripts: listScripts() });
    }
    
    // Sessions
    if (path === '/sessions') {
      if (method === 'GET') {
        const sessions = await this.sessions.listSessions();
        return Response.json({ sessions });
      }
      
      if (method === 'POST') {
        const body = await request.json();
        const validation = validateRequest(CreateSessionSchema, body);
        if (!validation.success) {
          throw new APIError('VALIDATION_ERROR', validation.error, 400);
        }
        
        const session = await this.sessions.createSession(validation.data);
        return Response.json(session, { status: 201 });
      }
    }
    
    // Session-specific routes
    const sessionMatch = path.match(/^\/sessions\/([^/]+)(\/.*)?$/);
    if (sessionMatch) {
      const [, sessionId, subPath = ''] = sessionMatch;
      
      // Validate session token
      const tokenAuth = await authenticate(request, this.env, sessionId);
      if (!tokenAuth.valid) {
        throw new APIError('UNAUTHORIZED', tokenAuth.error!, 401);
      }
      
      const stub = this.env.BROWSER_SESSION.get(
        this.env.BROWSER_SESSION.idFromName(sessionId)
      );
      
      // WebSocket upgrade
      if (subPath === '/ws' && request.headers.get('Upgrade') === 'websocket') {
        return stub.fetch(request);
      }
      
      // Run script
      if (subPath === '/run' && method === 'POST') {
        const body = await request.json();
        const validation = validateRequest(RunScriptSchema, body);
        if (!validation.success) {
          throw new APIError('VALIDATION_ERROR', validation.error, 400);
        }
        
        const response = await stub.fetch(new Request('http://internal/run', {
          method: 'POST',
          body: JSON.stringify(validation.data),
          headers: { 'Content-Type': 'application/json' },
        }));
        
        return response;
      }
      
      // Complete takeover
      if (subPath === '/takeover/complete' && method === 'POST') {
        return stub.fetch(new Request('http://internal/takeover/complete', { method: 'POST' }));
      }
      
      // Get status
      if (!subPath && method === 'GET') {
        return stub.fetch(new Request('http://internal/status'));
      }
      
      // Delete session
      if (!subPath && method === 'DELETE') {
        await this.sessions.deleteSession(sessionId);
        return new Response(null, { status: 204 });
      }
    }
    
    throw new APIError('NOT_FOUND', 'Endpoint not found', 404);
  }
}
```

---

## 9. Implementation Phases

### Phase 1: Security Foundation (Week 1)

**Goal**: Make the service secure enough for internal testing.

| Task | Priority | Effort |
|------|----------|--------|
| Implement session tokens | 🔴 Critical | 4h |
| Authenticate WebSocket connections | 🔴 Critical | 2h |
| Remove hardcoded secrets | 🔴 Critical | 1h |
| Implement CORS restrictions | 🔴 Critical | 2h |
| Remove stack traces from errors | 🔴 Critical | 1h |
| Add input validation (zod) | 🟠 High | 4h |
| Add rate limiting | 🟡 Medium | 4h |

**Deliverables**:
- [ ] All requests authenticated
- [ ] Session tokens generated and validated
- [ ] CORS locked to allowed origins
- [ ] No sensitive data in error responses

### Phase 2: Precompiled Scripts (Week 2)

**Goal**: Replace proxy-based execution with server-side scripts.

| Task | Priority | Effort |
|------|----------|--------|
| Design ScriptContext interface | 🔴 Critical | 2h |
| Implement ScriptExecutor | 🔴 Critical | 6h |
| Migrate Instagram script | 🔴 Critical | 4h |
| Implement script registry | 🟠 High | 2h |
| Add script parameter validation | 🟠 High | 2h |
| Remove frontend script execution | 🟠 High | 4h |
| Add /scripts API endpoint | 🟡 Medium | 2h |

**Deliverables**:
- [ ] Scripts execute server-side with full Playwright API
- [ ] Instagram script working in new format
- [ ] Frontend simplified to viewer + takeover UI

### Phase 3: Session Management (Week 3)

**Goal**: Stateless, self-cleaning session system.

| Task | Priority | Effort |
|------|----------|--------|
| Fix registry cleanup | 🔴 Critical | 4h |
| Implement session expiration | 🔴 Critical | 4h |
| Add cleanup error handling | 🟠 High | 2h |
| Implement heartbeat mechanism | 🟠 High | 4h |
| Add session limits | 🟠 High | 4h |
| Add memory monitoring | 🟡 Medium | 4h |
| Implement graceful shutdown | 🟡 Medium | 4h |

**Deliverables**:
- [ ] Sessions auto-expire after timeout
- [ ] Registry always in sync with actual sessions
- [ ] Resource leaks eliminated
- [ ] Clear cleanup on all error paths

### Phase 4: CAPTCHA & DX (Week 4)

**Goal**: Robust CAPTCHA handling and great developer experience.

| Task | Priority | Effort |
|------|----------|--------|
| Improve CAPTCHA detection | 🟠 High | 4h |
| Add multi-provider support | 🟠 High | 4h |
| Implement human fallback flow | 🟠 High | 4h |
| Set up local dev with browser | 🟠 High | 4h |
| Create script testing harness | 🟠 High | 4h |
| Write API documentation | 🟠 High | 6h |
| Generate SDK | 🟡 Medium | 4h |

**Deliverables**:
- [ ] CAPTCHA solving works for reCAPTCHA, hCaptcha, Turnstile
- [ ] `npm run dev` spins up local browser
- [ ] Scripts testable locally before deploy
- [ ] OpenAPI spec and generated SDK

### Phase 5: Production Hardening (Week 5)

**Goal**: Ready for production traffic.

| Task | Priority | Effort |
|------|----------|--------|
| Add structured logging | 🟠 High | 4h |
| Integrate error tracking (Sentry) | 🟠 High | 4h |
| Add metrics/observability | 🟠 High | 6h |
| Load testing | 🟠 High | 4h |
| Security audit | 🔴 Critical | 8h |
| Create runbook | 🟡 Medium | 4h |
| Set up staging environment | 🟡 Medium | 4h |

**Deliverables**:
- [ ] Errors tracked in Sentry
- [ ] Metrics dashboard
- [ ] Load tested to expected capacity
- [ ] Staging environment matching production
- [ ] Runbook for common issues

---

## Summary

This plan transforms a prototype into a production service by:

1. **Eliminating security vulnerabilities** - Proper auth everywhere, no code injection
2. **Moving scripts server-side** - Full Playwright API, no proxy overhead
3. **Making sessions stateless** - Clean lifecycle, automatic cleanup
4. **Improving CAPTCHA handling** - Better detection, multi-provider, human fallback
5. **Enabling local development** - Test scripts with real browser locally
6. **Documenting everything** - OpenAPI spec, generated SDKs, clear examples

Total estimated effort: **~5 weeks** for one engineer, or **~2-3 weeks** for a small team.
