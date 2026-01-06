import { launch, Browser, Page, CDPSession } from '@cloudflare/playwright';
import { Solver } from '@2captcha/captcha-solver';
import { 
  Env, 
  SessionStatus, 
  ServerMessage, 
  ClientMessage, 
  StoredNetworkRequest, 
  NetworkRequest,
  ResourceType,
  LRUCache,
  AutomationState,
  AutomationMode,
  InputSchema
} from './types';

const NETWORK_CACHE_SIZE = 1000;

interface CDPRequestWillBeSent {
  requestId: string;
  request: {
    url: string;
    method: string;
    headers: Record<string, string>;
    postData?: string;
  };
  type?: string;
  timestamp: number;
}

interface CDPResponseReceived {
  requestId: string;
  response: {
    url: string;
    status: number;
    statusText: string;
    headers: Record<string, string>;
    mimeType: string;
  };
  type?: string;
  timestamp: number;
}

interface CDPLoadingFinished {
  requestId: string;
  timestamp: number;
  encodedDataLength: number;
}

interface CDPLoadingFailed {
  requestId: string;
  timestamp: number;
  errorText: string;
  canceled?: boolean;
}

export class BrowserSession {
  private state: DurableObjectState;
  private env: Env;
  
  private static readonly IDLE_TIMEOUT_MS = 5 * 60 * 1000; // 5 minutes
  private lastActivityTime: number = Date.now();
  
  private browser: Browser | null = null;
  private page: Page | null = null;
  private cdp: CDPSession | null = null;
  
  private status: SessionStatus = 'idle';
  private takeoverMessage = '';
  private error = '';
  private result: unknown = null;
  
  private wsConnections: Set<WebSocket> = new Set();
  private takeoverResolver: (() => void) | null = null;
  private inputResolvers: Map<string, {
    resolve: (values: Record<string, unknown>) => void;
    reject: (error: Error) => void;
  }> = new Map();
  
  private viewportWidth = 800;
  private viewportHeight = 600;

  private networkRequests: LRUCache<string, StoredNetworkRequest> = new LRUCache(NETWORK_CACHE_SIZE);
  private capturePatterns: Map<string, { url?: RegExp; body?: RegExp }> = new Map();
  private capturedResponses: Map<string, unknown> = new Map();
  
  private automationData: Record<string, unknown> = {};
  private automationMode: AutomationMode = 'idle';

  constructor(state: DurableObjectState, env: Env) {
    this.state = state;
    this.env = env;
  }

  /**
   * Durable Object alarm handler - automatically called by Cloudflare
   * when the scheduled alarm time is reached.
   */
  async alarm(): Promise<void> {
    const now = Date.now();
    const idleTime = now - this.lastActivityTime;
    
    if (idleTime >= BrowserSession.IDLE_TIMEOUT_MS) {
      console.log(`[alarm] Session idle for ${Math.round(idleTime / 1000)}s, cleaning up...`);
      await this.cleanup();
    } else {
      // Session had activity since alarm was set, reschedule for remaining time
      const remainingTime = BrowserSession.IDLE_TIMEOUT_MS - idleTime;
      console.log(`[alarm] Session still active, rescheduling alarm for ${Math.round(remainingTime / 1000)}s`);
      await this.state.storage.setAlarm(now + remainingTime);
    }
  }

  /**
   * Reset the idle timer on any activity. Schedules an alarm to clean up
   * the session if there's no activity for IDLE_TIMEOUT_MS.
   */
  private async resetIdleTimer(): Promise<void> {
    this.lastActivityTime = Date.now();
    await this.state.storage.setAlarm(Date.now() + BrowserSession.IDLE_TIMEOUT_MS);
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    if (request.headers.get('Upgrade') === 'websocket') {
      return this.handleWebSocket();
    }

    if (path === '/init' && request.method === 'POST') {
      return this.initSession();
    }

    if (path === '/script' && request.method === 'POST') {
      const { code } = await request.json() as { code: string };
      await this.runScript(code);
      return Response.json({ status: 'started' });
    }

    if (path === '/done' && request.method === 'POST') {
      this.completeTakeover();
      return Response.json({ status: 'ok' });
    }

    if (path === '/finish' && request.method === 'POST') {
      const { result } = await request.json() as { result: unknown };
      await this.scriptFinished(result);
      return Response.json({ status: 'ok' });
    }

    if (path === '/status') {
      return Response.json({
        status: this.status,
        takeoverMessage: this.takeoverMessage,
        error: this.error,
        result: this.result
      });
    }

    if (path === '/destroy') {
      await this.cleanup();
      return Response.json({ status: 'destroyed' });
    }

    if (path === '/network/clear' && request.method === 'POST') {
      this.networkRequests.clear();
      this.broadcast({ type: 'network:clear' });
      return Response.json({ status: 'cleared' });
    }

    const networkMatch = path.match(/^\/network\/(.+)$/);
    if (networkMatch && request.method === 'GET') {
      const requestId = networkMatch[1];
      const storedRequest = this.networkRequests.get(requestId);
      if (!storedRequest) {
        return Response.json({ error: 'Request not found' }, { status: 404 });
      }
      return Response.json({
        requestHeaders: storedRequest.requestHeaders,
        requestBody: storedRequest.requestPostData,
        responseHeaders: storedRequest.responseHeaders || {},
        responseBody: storedRequest.responseBody,
        base64Encoded: storedRequest.responseBase64Encoded || false
      });
    }

    return new Response('Not Found', { status: 404 });
  }

  private async initSession(): Promise<Response> {
    try {
      console.log('[initSession] Starting browser session...');
      this.status = 'starting';
      this.broadcast({ type: 'status', status: 'starting' });

      // Start idle timer when session is initialized
      await this.resetIdleTimer();

      console.log('[initSession] Launching browser via BROWSER binding...');
      this.browser = await launch(this.env.BROWSER);
      console.log('[initSession] Browser launched successfully');
      
      console.log('[initSession] Creating new page...');
      this.page = await this.browser.newPage();
      console.log('[initSession] Page created');
      
      console.log('[initSession] Setting viewport size...');
      await this.page.setViewportSize({ 
        width: this.viewportWidth, 
        height: this.viewportHeight 
      });
      console.log('[initSession] Viewport set');

      this.cdp = await this.page.context().newCDPSession(this.page);
      await this.startScreencast();
      console.log('[initSession] Screencast started');

      console.log('[initSession] Enabling network capture...');
      await this.startNetworkCapture();
      console.log('[initSession] Network capture enabled');

      this.status = 'idle';
      this.broadcast({ type: 'status', status: 'idle' });
      console.log('[initSession] Session ready');

      return Response.json({ status: 'ready' });
    } catch (error) {
      console.error('[initSession] Error during initialization:', error);
      this.handleError(error);
      return Response.json({ error: this.error }, { status: 500 });
    }
  }

  private async startScreencast(): Promise<void> {
    if (!this.cdp) return;

    this.cdp.on('Page.screencastFrame', async (params: { data: string; sessionId: number }) => {
      this.broadcast({
        type: 'frame',
        data: params.data,
        timestamp: Date.now()
      });

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

  private async startNetworkCapture(): Promise<void> {
    if (!this.cdp) return;

    await this.cdp.send('Network.enable', {});

    this.cdp.on('Network.requestWillBeSent', (params: CDPRequestWillBeSent) => {
      const resourceType = this.normalizeResourceType(params.type || 'Other');
      
      const networkRequest: StoredNetworkRequest = {
        requestId: params.requestId,
        url: params.request.url,
        method: params.request.method,
        type: resourceType,
        timestamp: params.timestamp * 1000,
        requestHeaders: params.request.headers,
        requestPostData: params.request.postData
      };

      this.networkRequests.set(params.requestId, networkRequest);

      const broadcastRequest: NetworkRequest = {
        requestId: networkRequest.requestId,
        url: networkRequest.url,
        method: networkRequest.method,
        type: networkRequest.type,
        timestamp: networkRequest.timestamp
      };

      this.broadcast({ type: 'network:request', request: broadcastRequest });
    });

    this.cdp.on('Network.responseReceived', (params: CDPResponseReceived) => {
      const existing = this.networkRequests.get(params.requestId);
      if (existing) {
        existing.status = params.response.status;
        existing.statusText = params.response.statusText;
        existing.mimeType = params.response.mimeType;
        existing.responseHeaders = params.response.headers;
        existing.responseTimestamp = params.timestamp * 1000;
        existing.type = this.normalizeResourceType(params.type || 'Other');
      }

      this.broadcast({
        type: 'network:response',
        requestId: params.requestId,
        status: params.response.status,
        statusText: params.response.statusText,
        mimeType: params.response.mimeType,
        responseHeaders: params.response.headers
      });
    });

    this.cdp.on('Network.loadingFinished', async (params: CDPLoadingFinished) => {
      const existing = this.networkRequests.get(params.requestId);
      
      if (existing) {
        try {
          const bodyResult = await this.cdp!.send('Network.getResponseBody', {
            requestId: params.requestId
          }) as { body: string; base64Encoded: boolean };
          
          existing.responseBody = bodyResult.body;
          existing.responseBase64Encoded = bodyResult.base64Encoded;

          if (!existing.requestPostData) {
            try {
              const postDataResult = await this.cdp!.send('Network.getRequestPostData', {
                requestId: params.requestId
              }) as { postData: string };
              existing.requestPostData = postDataResult.postData;
            } catch {
              // Ignore
            }
          }

          this.checkCapturePatterns(existing);
        } catch {
          existing.responseBody = undefined;
        }
      }

      this.broadcast({ 
        type: 'network:finished', 
        requestId: params.requestId,
        capturedByKey: existing?.capturedByKey 
      });
    });

    this.cdp.on('Network.loadingFailed', (params: CDPLoadingFailed) => {
      const existing = this.networkRequests.get(params.requestId);
      if (existing) {
        existing.errorText = params.errorText;
      }

      this.broadcast({
        type: 'network:failed',
        requestId: params.requestId,
        errorText: params.errorText
      });
    });
  }

  private normalizeResourceType(cdpType: string): ResourceType {
    const typeMap: Record<string, ResourceType> = {
      'Document': 'Document',
      'Stylesheet': 'Stylesheet',
      'Image': 'Image',
      'Media': 'Media',
      'Font': 'Font',
      'Script': 'Script',
      'TextTrack': 'TextTrack',
      'XHR': 'XHR',
      'Fetch': 'Fetch',
      'Prefetch': 'Prefetch',
      'EventSource': 'EventSource',
      'WebSocket': 'WebSocket',
      'Manifest': 'Manifest',
      'SignedExchange': 'SignedExchange',
      'Ping': 'Ping',
      'CSPViolationReport': 'CSPViolationReport',
      'Preflight': 'Preflight'
    };
    return typeMap[cdpType] || 'Other';
  }

  private async handleViewportResize(requestedWidth: number, requestedHeight: number): Promise<void> {
    const ASPECT_RATIO = 4 / 3;
    const MIN_WIDTH = 320;
    const MIN_HEIGHT = 240;
    const MAX_WIDTH = 1920;
    const MAX_HEIGHT = 1440;

    let width = Math.floor(requestedWidth);
    let height = Math.round(width / ASPECT_RATIO);

    if (height > requestedHeight) {
      height = Math.floor(requestedHeight);
      width = Math.round(height * ASPECT_RATIO);
    }

    width = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, width));
    height = Math.max(MIN_HEIGHT, Math.min(MAX_HEIGHT, height));
    height = Math.round(width / ASPECT_RATIO);

    if (width === this.viewportWidth && height === this.viewportHeight) {
      return;
    }

    console.log(`[handleViewportResize] Resizing from ${this.viewportWidth}x${this.viewportHeight} to ${width}x${height}`);

    this.viewportWidth = width;
    this.viewportHeight = height;

    if (this.page) {
      await this.page.setViewportSize({ width, height });
    }

    if (this.cdp) {
      try {
        await this.cdp.send('Page.stopScreencast');
      } catch {
      }
      await this.startScreencast();
    }

    this.broadcast({ type: 'viewport:ack', width, height });
  }

  private checkCapturePatterns(request: StoredNetworkRequest): void {
    for (const [key, patterns] of this.capturePatterns.entries()) {
      let match = true;

      if (patterns.url && !patterns.url.test(request.url)) {
        match = false;
      }

      if (match && patterns.body) {
        // For body matching, we need the request body (for POST) or response body?
        // The script uses bodyPattern to match GraphQL query names in POST body
        if (!request.requestPostData || !patterns.body.test(request.requestPostData)) {
          match = false;
        }
      }

      if (match) {
        let responseData: unknown;
        
        if (request.responseBody) {
          if (request.responseBase64Encoded) {
            responseData = request.responseBody;
          } else {
            try {
              responseData = JSON.parse(request.responseBody);
            } catch {
              responseData = request.responseBody;
            }
          }
        }

        this.capturedResponses.set(key, {
          url: request.url,
          status: request.status,
          headers: request.responseHeaders,
          body: responseData,
          data: responseData // Alias for easier access if script expects .data
        });

        request.capturedByKey = key;
      }
    }
  }

  private waitForCapturedResponse(key: string, timeout: number): Promise<unknown> {
    return new Promise((resolve, reject) => {
      if (this.capturedResponses.has(key)) {
        resolve(this.capturedResponses.get(key));
        return;
      }

      const startTime = Date.now();
      const checkInterval = setInterval(() => {
        if (this.capturedResponses.has(key)) {
          clearInterval(checkInterval);
          resolve(this.capturedResponses.get(key));
        } else if (Date.now() - startTime > timeout) {
          clearInterval(checkInterval);
          reject(new Error(`Timeout waiting for network capture: ${key}`));
        }
      }, 100);
    });
  }

  private async runScript(_code: string): Promise<void> {
    this.status = 'running';
    this.error = '';
    this.result = null;
    this.broadcast({ type: 'status', status: 'running' });
  }

  async scriptFinished(result: unknown): Promise<void> {
    this.status = 'done';
    this.result = result;
    this.broadcast({ type: 'status', status: 'done' });
    this.broadcast({ type: 'result', data: result });
  }

  private async executeCommand(commandId: string, method: string, args: unknown[]): Promise<void> {
    const page = this.page;
    if (!page) {
      this.broadcast({
        type: 'commandResult',
        commandId,
        result: null,
        error: 'Browser not initialized'
      });
      return;
    }

    try {
      let result: unknown;

      if (method === 'captureNetwork') {
        const arg0 = args[0];
        let key: string;
        let urlPattern: string | undefined;
        let bodyPattern: string | undefined;

        if (typeof arg0 === 'object' && arg0 !== null) {
          const opts = arg0 as { key: string; urlPattern?: string; bodyPattern?: string };
          key = opts.key;
          urlPattern = opts.urlPattern;
          bodyPattern = opts.bodyPattern;
        } else {
          key = args[0] as string;
          urlPattern = args[1] as string;
        }

        if (typeof key !== 'string') {
          throw new Error('captureNetwork requires a string key');
        }

        this.capturePatterns.set(key, {
          url: urlPattern ? new RegExp(urlPattern) : undefined,
          body: bodyPattern ? new RegExp(bodyPattern) : undefined
        });
        result = null;
      } else if (method === 'clearNetworkCaptures') {
        this.capturePatterns.clear();
        this.capturedResponses.clear();
        result = null;
      } else if (method === 'getCapturedResponse') {
        const key = args[0];
        if (typeof key !== 'string') {
          throw new Error('getCapturedResponse(key) requires a string');
        }
        result = this.capturedResponses.get(key) || null;
      } else if (method === 'waitForNetworkCapture') {
        const key = args[0];
        const timeout = typeof args[1] === 'number' ? args[1] : 30000;
        if (typeof key !== 'string') {
          throw new Error('waitForNetworkCapture(key) requires a string');
        }
        result = await this.waitForCapturedResponse(key, timeout);
      } else if (method === 'setData') {
        const key = args[0];
        const value = args[1];
        if (typeof key !== 'string') {
          throw new Error('setData(key, value) requires a string key');
        }
        this.automationData[key] = value;
        this.broadcast({ type: 'automation:data', key, value });
        result = null;
      } else if (method === 'getData') {
        result = { ...this.automationData };
      } else if (method === 'clearData') {
        this.automationData = {};
        result = null;
      } else if (method === 'scrapeText') {
        const selector = args[0];
        if (typeof selector !== 'string') {
          throw new Error('scrapeText(selector) requires a string');
        }
        const element = await page.$(selector);
        result = element ? await element.textContent() : null;
      } else if (method === 'scrapeAttribute') {
        const selector = args[0];
        const attribute = args[1];
        if (typeof selector !== 'string' || typeof attribute !== 'string') {
          throw new Error('scrapeAttribute(selector, attribute) requires strings');
        }
        const element = await page.$(selector);
        result = element ? await element.getAttribute(attribute) : null;
      } else if (method === 'scrapeAll') {
        const selector = args[0];
        const options = args[1] as { text?: boolean; attribute?: string } | undefined;
        if (typeof selector !== 'string') {
          throw new Error('scrapeAll(selector, options?) requires a string selector');
        }
        const elements = await page.$$(selector);
        const results: unknown[] = [];
        for (const el of elements) {
          if (options?.attribute) {
            results.push(await el.getAttribute(options.attribute));
          } else {
            results.push(await el.textContent());
          }
        }
        result = results;
      } else if (method === 'exists') {
        const selector = args[0];
        if (typeof selector !== 'string') throw new Error('exists(selector) requires a string');
        const element = await page.$(selector);
        result = !!element;
      } else if (method === 'count') {
        const selector = args[0];
        if (typeof selector !== 'string') throw new Error('count(selector) requires a string');
        const elements = await page.$$(selector);
        result = elements.length;
      } else if (method === 'promptUser') {
        const message = args[0];
        const options = args[1] as { continueWhen?: string; timeout?: number; nonBlocking?: boolean } | undefined;
        if (typeof message !== 'string') {
          throw new Error('promptUser(message, options?) requires a string message');
        }
        
        if (options?.nonBlocking) {
          this.status = 'takeover';
          this.takeoverMessage = message;
          this.broadcast({ 
            type: 'status', 
            status: 'takeover', 
            message, 
            
          });
          result = null;
        } else {
          await this.requestTakeover(message);
          if (options?.continueWhen) {
            const timeout = options.timeout || 300000;
            await page.waitForSelector(options.continueWhen, { timeout });
          }
        }
      } else if (method === 'completeTakeover') {
        this.completeTakeover();
        result = null;
      } else if (method === 'getInput') {
        const inputSchema = args[0] as InputSchema;
        const requestId = `input_${Date.now()}_${Math.random().toString(36).slice(2)}`;
        
        this.broadcast({ type: 'input:request', requestId, input: inputSchema });
        
        result = await new Promise((resolve, reject) => {
          this.inputResolvers.set(requestId, { resolve, reject });
        });
      } else if (method === 'solveCaptcha') {
        const options = (args[0] || {}) as any;
        result = await this.handleSolveCaptcha(page, options);
      } else if (method.includes('.')) {
        result = await this.executeLocatorCommand(page, method, args);
      } else {
        result = await this.executePageCommand(page, method, args);
      }

      this.broadcast({ type: 'commandResult', commandId, result });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.broadcast({ type: 'commandResult', commandId, result: null, error: message });
    }
  }

  private async executePageCommand(page: Page, method: string, args: unknown[]): Promise<unknown> {
    switch (method) {
      case 'goto': {
        const url = args[0];
        if (typeof url !== 'string') throw new Error('goto(url) requires a string');
        const options = (args[1] ?? undefined) as Parameters<Page['goto']>[1];
        return await page.goto(url, options);
      }
      case 'click': {
        const selector = args[0];
        if (typeof selector !== 'string') throw new Error('click(selector) requires a string');
        const options = (args[1] ?? undefined) as Parameters<Page['click']>[1];
        await this.broadcastCursorForSelector(page, selector, 'click');
        return await page.click(selector, options);
      }
      case 'fill': {
        const selector = args[0];
        const value = args[1];
        if (typeof selector !== 'string' || typeof value !== 'string') {
          throw new Error('fill(selector, value) requires strings');
        }
        const options = (args[2] ?? undefined) as Parameters<Page['fill']>[2];
        await this.broadcastCursorForSelector(page, selector, 'click');
        return await page.fill(selector, value, options);
      }
      case 'type': {
        const selector = args[0];
        const text = args[1];
        if (typeof selector !== 'string' || typeof text !== 'string') {
          throw new Error('type(selector, text) requires strings');
        }
        const options = (args[2] ?? undefined) as Parameters<Page['type']>[2];
        await this.broadcastCursorForSelector(page, selector, 'click');
        return await page.type(selector, text, options);
      }
      case 'press': {
        const selector = args[0];
        const key = args[1];
        if (typeof selector !== 'string' || typeof key !== 'string') {
          throw new Error('press(selector, key) requires strings');
        }
        const options = (args[2] ?? undefined) as Parameters<Page['press']>[2];
        await this.broadcastCursorForSelector(page, selector, 'click');
        return await page.press(selector, key, options);
      }
      case 'waitForSelector': {
        const selector = args[0];
        if (typeof selector !== 'string') throw new Error('waitForSelector(selector) requires a string');
        const options = (args[1] ?? undefined) as Parameters<Page['waitForSelector']>[1];
        return await page.waitForSelector(selector, options);
      }
      case 'waitForTimeout': {
        const timeout = args[0];
        if (typeof timeout !== 'number') throw new Error('waitForTimeout(ms) requires a number');
        await page.waitForTimeout(timeout);
        return null;
      }
      case 'screenshot': {
        const options = (args[0] ?? undefined) as Parameters<Page['screenshot']>[0];
        return await page.screenshot(options);
      }
      case 'title': {
        return await page.title();
      }
      case 'url': {
        return page.url();
      }
      case 'evaluate': {
        const expression = args[0];
        if (typeof expression !== 'string') throw new Error('evaluate(expression) requires a string');
        const arg = args.length >= 2 ? args[1] : undefined;
        return await page.evaluate(expression, arg as never);
      }
      default:
        throw new Error(`Unsupported page method: ${method}`);
    }
  }

  private async broadcastCursorForSelector(page: Page, selector: string, action: 'move' | 'click'): Promise<void> {
    try {
      const element = await page.$(selector);
      if (element) {
        const box = await element.boundingBox();
        if (box) {
          const x = Math.round(box.x + box.width / 2);
          const y = Math.round(box.y + box.height / 2);
          this.broadcast({ type: 'automation:cursor', x, y, action });
        }
      }
    } catch {
    }
  }

  private async executeLocatorCommand(page: Page, method: string, args: unknown[]): Promise<unknown> {
    const parts = method.split('.');
    if (parts.length !== 2) {
      throw new Error(`Unsupported locator method: ${method}`);
    }
    const [locatorFactory, action] = parts;

    if (locatorFactory === 'getByPlaceholder') {
      const text = args[0];
      if (typeof text !== 'string') throw new Error('getByPlaceholder(text) requires a string');
      const locator = page.getByPlaceholder(text);
      return await this.executeLocatorAction(locator, action, args.slice(1));
    }

    if (locatorFactory === 'getByText') {
      const text = args[0];
      if (typeof text !== 'string') throw new Error('getByText(text) requires a string');
      const locator = page.getByText(text);
      return await this.executeLocatorAction(locator, action, args.slice(1));
    }

    if (locatorFactory === 'getByRole') {
      const role = args[0];
      if (typeof role !== 'string') throw new Error('getByRole(role) requires a string');

      if (action === 'click') {
        const maybeOptions = args.length >= 2 ? args[1] : undefined;
        const locator = page.getByRole(role as never, (maybeOptions ?? undefined) as never);
        return await this.executeLocatorAction(locator, action, []);
      }

      if (action === 'fill' || action === 'press') {
        if (args.length === 2) {
          const locator = page.getByRole(role as never);
          return await this.executeLocatorAction(locator, action, [args[1]]);
        }
        if (args.length === 3) {
          const locator = page.getByRole(role as never, (args[1] ?? undefined) as never);
          return await this.executeLocatorAction(locator, action, [args[2]]);
        }
      }

      throw new Error(`Unsupported getByRole usage for ${method}`);
    }

    throw new Error(`Unsupported locator factory: ${locatorFactory}`);
  }

  private async executeLocatorAction(locator: ReturnType<Page['getByText']>, action: string, actionArgs: unknown[]): Promise<unknown> {
    await this.broadcastCursorForLocator(locator, action === 'click' ? 'click' : 'move');
    
    if (action === 'fill') {
      const value = actionArgs[0];
      if (typeof value !== 'string') throw new Error('locator.fill(value) requires a string');
      await locator.fill(value);
      return null;
    }

    if (action === 'press') {
      const key = actionArgs[0];
      if (typeof key !== 'string') throw new Error('locator.press(key) requires a string');
      await locator.press(key);
      return null;
    }

    if (action === 'click') {
      await locator.click();
      return null;
    }

    throw new Error(`Unsupported locator action: ${action}`);
  }

  private async broadcastCursorForLocator(locator: ReturnType<Page['getByText']>, action: 'move' | 'click'): Promise<void> {
    try {
      const box = await locator.boundingBox();
      if (box) {
        const x = Math.round(box.x + box.width / 2);
        const y = Math.round(box.y + box.height / 2);
        this.broadcast({ type: 'automation:cursor', x, y, action });
      }
    } catch {
    }
  }

  private requestTakeover(message: string): Promise<void> {
    return new Promise((resolve) => {
      this.status = 'takeover';
      this.takeoverMessage = message;
      this.takeoverResolver = resolve;
      this.broadcast({ type: 'status', status: 'takeover', message });
    });
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

  private handleWebSocket(): Response {
    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair);

    server.accept();
    this.wsConnections.add(server);

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
    // Reset idle timer on any client activity
    await this.resetIdleTimer();

    if (message.type === 'command') {
      await this.executeCommand(message.commandId, message.method, message.args);
      return;
    }

    if (message.type === 'viewport') {
      await this.handleViewportResize(message.width, message.height);
      return;
    }

    if (message.type === 'input:response') {
      const resolver = this.inputResolvers.get(message.requestId);
      if (resolver) {
        this.inputResolvers.delete(message.requestId);
        resolver.resolve(message.values);
      }
      return;
    }

    if (message.type === 'input:cancel') {
      const resolver = this.inputResolvers.get(message.requestId);
      if (resolver) {
        this.inputResolvers.delete(message.requestId);
        resolver.reject(new Error('User cancelled input'));
      }
      this.broadcast({ type: 'input:cancelled', requestId: message.requestId });
      return;
    }

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
          text: message.text,
          code: message.code,
          windowsVirtualKeyCode: message.keyCode,
          nativeVirtualKeyCode: message.keyCode,
        });
        break;

      case 'paste':
        if (typeof message.text === 'string') {
          await this.page?.keyboard.insertText(message.text);
        }
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
      } catch {
        this.wsConnections.delete(ws);
      }
    }
  }

  private handleError(error: unknown): void {
    const message = error instanceof Error ? error.message : String(error);
    const stack = error instanceof Error ? error.stack : undefined;
    
    console.error(`Session Error [${this.status}]:`, message, stack);
    
    this.status = 'error';
    this.error = stack ? `${message}\n\nStack:\n${stack}` : message;
    
    // Send the error message first
    this.broadcast({ type: 'error', message: this.error });
    // Then send status with the error message included
    this.broadcast({ type: 'status', status: 'error', message: this.error });
  }

  // Inspired by https://github.com/berstend/puppeteer-extra/tree/master/packages/puppeteer-extra-plugin-recaptcha
  private async handleSolveCaptcha(page: Page, options: any): Promise<any> {
    console.log('[solveCaptcha] ========== START ==========');
    console.log('[solveCaptcha] Options:', JSON.stringify(options));
    
    try {
      const currentUrl = page.url();
      console.log('[solveCaptcha] Current URL:', currentUrl);
      
      console.log('[solveCaptcha] Detecting captcha...');
      const captchaInfo = await this.detectCaptcha(page, options);
      console.log('[solveCaptcha] Detection result:', JSON.stringify(captchaInfo));
      
      if (!captchaInfo) {
        console.log('[solveCaptcha] No captcha detected, returning skipped');
        return { success: true, skipped: true };
      }

      this.automationData['status'] = 'Solving security check...';
      this.broadcast({ type: 'automation:data', key: 'status', value: 'Solving security check...' });
      this.broadcast({ type: 'automation:data', key: 'captcha_debug', value: `Detected: ${captchaInfo.type}, sitekey: ${captchaInfo.sitekey?.substring(0, 20)}...` });

      console.log('[solveCaptcha] Checking for TWOCAPTCHA_API_KEY...');
      if (!this.env.TWOCAPTCHA_API_KEY) {
        console.warn('[solveCaptcha] TWOCAPTCHA_API_KEY not set!');
        this.broadcast({ type: 'automation:data', key: 'captcha_debug', value: 'ERROR: TWOCAPTCHA_API_KEY not configured' });
        return await this.fallbackToPromptUser('Security check detected. Please solve the captcha manually.');
      }
      console.log('[solveCaptcha] API key is set (length:', this.env.TWOCAPTCHA_API_KEY.length, ')');

      console.log('[solveCaptcha] Calling 2captcha API with:', JSON.stringify({
        type: captchaInfo.type,
        sitekey: captchaInfo.sitekey?.substring(0, 20) + '...',
        pageurl: captchaInfo.pageurl,
        enterprise: captchaInfo.enterprise
      }));
      this.broadcast({ type: 'automation:data', key: 'captcha_debug', value: 'Calling 2captcha API...' });
      
      const token = await this.callTwoCaptcha(captchaInfo);
      console.log('[solveCaptcha] Got token (length:', token?.length, ')');
      this.broadcast({ type: 'automation:data', key: 'captcha_debug', value: `Got token (${token?.length} chars)` });
      
      console.log('[solveCaptcha] Injecting token...');
      this.broadcast({ type: 'automation:data', key: 'captcha_debug', value: 'Injecting token...' });
      await this.injectCaptchaToken(page, token, captchaInfo);
      console.log('[solveCaptcha] Token injected successfully');
      
      this.automationData['status'] = 'Security check solved';
      this.broadcast({ type: 'automation:data', key: 'status', value: 'Security check solved' });
      this.broadcast({ type: 'automation:data', key: 'captcha_debug', value: 'SUCCESS: Token injected' });
      
      console.log('[solveCaptcha] ========== SUCCESS ==========');
      return { success: true, token: token?.substring(0, 20) + '...' };
    } catch (error) {
      console.error('[solveCaptcha] ========== ERROR ==========');
      console.error('[solveCaptcha] Error type:', error?.constructor?.name);
      console.error('[solveCaptcha] Error message:', error instanceof Error ? error.message : String(error));
      console.error('[solveCaptcha] Error stack:', error instanceof Error ? error.stack : 'N/A');
      
      const errorMsg = error instanceof Error ? error.message : String(error);
      this.broadcast({ type: 'automation:data', key: 'captcha_debug', value: `ERROR: ${errorMsg}` });
      
      return await this.fallbackToPromptUser('Security check detected. Automatic solving failed. Please solve it manually.');
    }
  }

  private async detectCaptcha(page: Page, options: any): Promise<any | null> {
    console.log('[detectCaptcha] Starting detection with options:', JSON.stringify(options));
    
    // This script will be run in every frame
    const findCaptchaInFrame = (opts: any) => {
      const pageurl = window.location.href;
      
      // Simpler, more reliable search for keys in nested objects
      const findValueByKey = (obj: any, targetKey: string, maxDepth = 5): any => {
        if (!obj || maxDepth <= 0) return null;
        if (Object.prototype.hasOwnProperty.call(obj, targetKey)) {
          const val = obj[targetKey];
          if (val !== undefined && val !== null) return val;
        }
        
        for (const key in obj) {
          if (typeof obj[key] === 'object' && obj[key] !== null) {
            try {
              const found = findValueByKey(obj[key], targetKey, maxDepth - 1);
              if (found !== undefined && found !== null) return found;
            } catch (e) {}
          }
        }
        return null;
      };

      // 1. Google reCAPTCHA (The "Source of Truth")
      const cfg = (window as any).___grecaptcha_cfg;
      const clients = cfg?.clients;
      if (clients) {
        for (const id in clients) {
          const client = clients[id];
          const sitekey = opts.sitekey || findValueByKey(client, 'sitekey') || findValueByKey(client, 'googlekey');
          
          if (sitekey) {
            const isEnterprise = opts.enterprise || 
                               !!document.querySelector('script[src*="enterprise"]') || 
                               !!document.querySelector('iframe[src*="/enterprise/"]') ||
                               pageurl.includes('enterprise');

            let callback = findValueByKey(client, 'callback');
            let callbackName = opts.callbackName;
            if (!callbackName && callback) {
              callbackName = typeof callback === 'function' ? (callback.name || 'anonymous') : callback;
            }

            return {
              type: 'recaptcha_v2',
              sitekey,
              pageurl,
              enterprise: isEnterprise,
              callbackName,
              widgetId: id,
              action: findValueByKey(client, 'action'),
              s: findValueByKey(client, 's'),
              frameUrl: pageurl
            };
          }
        }
      }

      // 2. Fallback to DOM-based detection (for hCaptcha, Turnstile, or missed reCAPTCHA)
      const findSiteKey = () => {
        const el = document.querySelector('.g-recaptcha, [data-sitekey], [sitekey], .h-captcha, .cf-turnstile');
        if (el) return el.getAttribute('data-sitekey') || el.getAttribute('sitekey');
        
        const iframe = document.querySelector('iframe[src*="recaptcha"], iframe[src*="hcaptcha"], iframe[src*="turnstile"]');
        if (iframe) {
          try {
            const url = new URL((iframe as HTMLIFrameElement).src);
            return url.searchParams.get('k') || url.searchParams.get('sitekey');
          } catch (e) {}
        }
        return null;
      };

      const sitekey = opts.sitekey || findSiteKey();
      if (sitekey) {
        const isHcaptcha = !!document.querySelector('.h-captcha') || opts.type === 'hcaptcha';
        const isTurnstile = !!document.querySelector('.cf-turnstile') || opts.type === 'turnstile';
        
        let type = 'recaptcha_v2';
        if (isHcaptcha) type = 'hcaptcha';
        else if (isTurnstile) type = 'turnstile';

        return {
          type,
          sitekey,
          pageurl,
          enterprise: opts.enterprise || !!document.querySelector('script[src*="enterprise"]'),
          frameUrl: pageurl
        };
      }

      return null;
    };

    try {
      // Iterate through all frames
      const frames = page.frames();
      console.log(`[detectCaptcha] Scanning ${frames.length} frames...`);
      
      for (const frame of frames) {
        try {
          const result = await frame.evaluate(findCaptchaInFrame, options);
          if (result) {
            console.log(`[detectCaptcha] Found captcha in frame: ${frame.url()}`);
            return result;
          }
        } catch (e) {
          // Some frames might be cross-origin or inaccessible
          console.log(`[detectCaptcha] Frame scan failed for ${frame.url()}:`, (e as Error).message);
        }
      }
      
      console.log('[detectCaptcha] No captcha detected in any frame');
      return null;
    } catch (error) {
      console.error('[detectCaptcha] Error during detection:', error);
      throw error;
    }
  }

  private async callTwoCaptcha(info: any): Promise<string> {
    console.log('[callTwoCaptcha] ========== START ==========');
    console.log('[callTwoCaptcha] Creating solver with API key (length:', this.env.TWOCAPTCHA_API_KEY?.length, ')');
    
    const solver = new Solver(this.env.TWOCAPTCHA_API_KEY!);
    
    console.log('[callTwoCaptcha] Captcha type:', info.type);
    console.log('[callTwoCaptcha] Sitekey:', info.sitekey);
    console.log('[callTwoCaptcha] Page URL:', info.pageurl);
    console.log('[callTwoCaptcha] Enterprise:', info.enterprise);
    
    try {
      if (info.type === 'recaptcha_v2') {
        console.log('[callTwoCaptcha] Calling solver.recaptcha() with enterprise =', info.enterprise);
        const startTime = Date.now();
        
        const result = await solver.recaptcha({
          googlekey: info.sitekey,
          pageurl: info.pageurl,
          enterprise: info.enterprise
        });
        
        const elapsed = Date.now() - startTime;
        console.log('[callTwoCaptcha] solver.recaptcha() completed in', elapsed, 'ms');
        console.log('[callTwoCaptcha] Result received:', {
          hasData: !!result.data,
          dataLength: result.data?.length,
          id: result.id
        });
        
        if (!result.data) {
          throw new Error('2captcha returned empty token');
        }
        
        console.log('[callTwoCaptcha] ========== SUCCESS ==========');
        return result.data;
      } else if (info.type === 'hcaptcha') {
        console.log('[callTwoCaptcha] Calling solver.hcaptcha()...');
        const startTime = Date.now();
        
        const result = await solver.hcaptcha({
          sitekey: info.sitekey,
          pageurl: info.pageurl
        });
        
        const elapsed = Date.now() - startTime;
        console.log('[callTwoCaptcha] solver.hcaptcha() completed in', elapsed, 'ms');
        console.log('[callTwoCaptcha] Result received, data length:', result.data?.length);
        
        if (!result.data) {
          throw new Error('2captcha returned empty token');
        }
        
        console.log('[callTwoCaptcha] ========== SUCCESS ==========');
        return result.data;
      } else if (info.type === 'turnstile') {
        console.log('[callTwoCaptcha] Calling solver.cloudflareTurnstile()...');
        const startTime = Date.now();
        
        const result = await solver.cloudflareTurnstile({
          sitekey: info.sitekey,
          pageurl: info.pageurl
        });
        
        const elapsed = Date.now() - startTime;
        console.log('[callTwoCaptcha] solver.cloudflareTurnstile() completed in', elapsed, 'ms');
        console.log('[callTwoCaptcha] Result received, data length:', result.data?.length);
        
        if (!result.data) {
          throw new Error('2captcha returned empty token');
        }
        
        console.log('[callTwoCaptcha] ========== SUCCESS ==========');
        return result.data;
      }
      
      throw new Error(`Unsupported captcha type: ${info.type}`);
    } catch (error) {
      console.error('[callTwoCaptcha] ========== ERROR ==========');
      console.error('[callTwoCaptcha] Error type:', error?.constructor?.name);
      console.error('[callTwoCaptcha] Error message:', error instanceof Error ? error.message : String(error));
      console.error('[callTwoCaptcha] Error stack:', error instanceof Error ? error.stack : 'N/A');
      throw error;
    }
  }

  private async injectCaptchaToken(page: Page, token: string, info: any): Promise<void> {
    console.log('[injectCaptchaToken] ========== START ==========');
    console.log('[injectCaptchaToken] Token length:', token?.length);
    console.log('[injectCaptchaToken] Captcha type:', info.type);
    console.log('[injectCaptchaToken] Callback name:', info.callbackName);
    console.log('[injectCaptchaToken] Widget ID:', info.widgetId);
    
    // Injection script to be run in the target frame
    const injectInFrame = ({ token, info }: { token: string, info: any }) => {
      const results = {
        textareasFound: [] as string[],
        textareasFilled: [] as string[],
        callbackInvoked: false,
        callbackName: null as string | null,
        clientCallbacksInvoked: 0,
        visualFeedbackApplied: false
      };
      
      const selectors = [
        '[name="g-recaptcha-response"]',
        '#g-recaptcha-response',
        '[name="h-captcha-response"]',
        '#h-captcha-response',
        '[name="cf-turnstile-response"]',
        '#cf-turnstile-response'
      ];
      
      // 1. Fill Textareas
      selectors.forEach(sel => {
        const el = document.querySelector(sel) as HTMLTextAreaElement;
        if (el) {
          results.textareasFound.push(sel);
          el.innerHTML = token;
          el.value = token;
          el.dispatchEvent(new Event('input', { bubbles: true }));
          el.dispatchEvent(new Event('change', { bubbles: true }));
          results.textareasFilled.push(sel);
        }
      });
      
      // 2. Trigger Callbacks
      const triggerCallback = (cb: any) => {
        try {
          if (typeof cb === 'function') {
            cb(token);
            return true;
          } else if (typeof cb === 'string') {
            if ((window as any)[cb]) {
              (window as any)[cb](token);
              return true;
            } else {
              // Try eval if it's a string like "myFunction" or "obj.cb"
              try {
                const fn = eval(cb);
                if (typeof fn === 'function') {
                  fn(token);
                  return true;
                }
              } catch (e) {}
            }
          }
        } catch (e) {
          console.error('[inject:browser] Callback execution failed:', e);
        }
        return false;
      };

      // 3. Try to use the grecaptcha object directly if available
      const grecaptcha = (window as any).grecaptcha || (window as any).grecaptcha?.enterprise;
      
      // If we have a widgetId, try to find the specific callback in ___grecaptcha_cfg
      if (info.widgetId !== null) {
        const cfg = (window as any).___grecaptcha_cfg;
        if (cfg?.clients && cfg.clients[info.widgetId]) {
          const client = cfg.clients[info.widgetId];
          for (const key in client) {
            if (client[key]?.callback) {
              if (triggerCallback(client[key].callback)) {
                results.clientCallbacksInvoked++;
              }
            }
          }
        }
      }

      // Fallback to explicit callback name
      if (results.clientCallbacksInvoked === 0 && info.callbackName) {
        if (triggerCallback(info.callbackName)) {
          results.callbackInvoked = true;
          results.callbackName = info.callbackName;
        }
      }

      // 3. Visual Feedback (Inspired by PEPR)
      const applyFeedback = (iframe: HTMLIFrameElement) => {
        iframe.style.filter = 'opacity(60%) hue-rotate(230deg)'; // Green-ish
        results.visualFeedbackApplied = true;
      };

      const iframes = document.querySelectorAll('iframe');
      for (let i = 0; i < iframes.length; i++) {
        const iframe = iframes[i];
        if (iframe.src.includes('recaptcha') || iframe.src.includes('hcaptcha') || iframe.src.includes('turnstile')) {
          applyFeedback(iframe);
        }
      }
      
      return results;
    };

    try {
      const frames = page.frames();
      let success = false;
      
      console.log(`[injectCaptchaToken] Attempting injection in ${frames.length} frames...`);
      
      for (const frame of frames) {
        // If we have a frameUrl hint, prioritize that frame
        if (info.frameUrl && frame.url() !== info.frameUrl) continue;

        try {
          const result = await frame.evaluate(injectInFrame, { token, info });
          if (result.textareasFilled.length > 0 || result.clientCallbacksInvoked > 0 || result.callbackInvoked) {
            console.log(`[injectCaptchaToken] Success in frame: ${frame.url()}`, JSON.stringify(result));
            success = true;
            break; 
          }
        } catch (e) {
          console.log(`[injectCaptchaToken] Injection failed for frame ${frame.url()}:`, (e as Error).message);
        }
      }

      // If we prioritized a frame and failed, try all frames
      if (!success && info.frameUrl) {
        console.log('[injectCaptchaToken] Prioritized frame failed, trying all frames...');
        for (const frame of frames) {
          if (frame.url() === info.frameUrl) continue;
          try {
            const result = await frame.evaluate(injectInFrame, { token, info });
            if (result.textareasFilled.length > 0 || result.clientCallbacksInvoked > 0 || result.callbackInvoked) {
              console.log(`[injectCaptchaToken] Success in frame (fallback): ${frame.url()}`, JSON.stringify(result));
              success = true;
              break;
            }
          } catch (e) {}
        }
      }
      
      if (!success) {
        console.warn('[injectCaptchaToken] No suitable frame found for injection');
      }
      
      console.log('[injectCaptchaToken] ========== SUCCESS ==========');
    } catch (error) {
      console.error('[injectCaptchaToken] ========== ERROR ==========');
      console.error('[injectCaptchaToken] Error:', error instanceof Error ? error.message : String(error));
      throw error;
    }
  }

  private async fallbackToPromptUser(message: string): Promise<any> {
    await this.requestTakeover(message);
    return { success: true, fallback: true };
  }

  private async cleanup(): Promise<void> {
    if (this.cdp) {
      try {
        await this.cdp.send('Page.stopScreencast');
      } catch { /* ignore */ }
      try {
        await this.cdp.send('Network.disable');
      } catch { /* ignore */ }
    }
    if (this.browser) {
      try {
        await this.browser.close();
      } catch { /* ignore */ }
    }
    this.browser = null;
    this.page = null;
    this.cdp = null;
    this.status = 'idle';
    
    this.networkRequests.clear();
    this.capturePatterns.clear();
    this.capturedResponses.clear();
    this.automationData = {};
    
    for (const resolver of this.inputResolvers.values()) {
      resolver.reject(new Error('Session closed'));
    }
    this.inputResolvers.clear();
    
    for (const ws of this.wsConnections) {
      try {
        ws.close();
      } catch { /* ignore */ }
    }
    this.wsConnections.clear();
  }
}
