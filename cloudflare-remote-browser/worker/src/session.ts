import { launch, Browser, Page, CDPSession } from '@cloudflare/playwright';
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
  AutomationMode
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
  
  private browser: Browser | null = null;
  private page: Page | null = null;
  private cdp: CDPSession | null = null;
  
  private status: SessionStatus = 'idle';
  private takeoverMessage = '';
  private error = '';
  private result: unknown = null;
  
  private wsConnections: Set<WebSocket> = new Set();
  private takeoverResolver: (() => void) | null = null;
  
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
    if (message.type === 'command') {
      await this.executeCommand(message.commandId, message.method, message.args);
      return;
    }

    if (message.type === 'viewport') {
      await this.handleViewportResize(message.width, message.height);
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
    
    for (const ws of this.wsConnections) {
      try {
        ws.close();
      } catch { /* ignore */ }
    }
    this.wsConnections.clear();
  }
}
