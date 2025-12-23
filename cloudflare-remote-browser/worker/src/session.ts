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

      console.log('[initSession] Creating CDP session...');
      this.cdp = await this.page.context().newCDPSession(this.page);
      console.log('[initSession] CDP session created');
      
      console.log('[initSession] Starting screencast...');
      await this.startScreencast();
      console.log('[initSession] Screencast started');

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

  private async runScript(_code: string): Promise<void> {
    this.status = 'running';
    this.error = '';
    this.result = null;
    this.broadcast({ type: 'status', status: 'running' });
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

      if (method === 'requestTakeover') {
        const message = args[0];
        if (typeof message !== 'string') {
          throw new Error('requestTakeover(message) requires a string');
        }
        await this.requestTakeover(message);
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
        return await page.click(selector, options);
      }
      case 'fill': {
        const selector = args[0];
        const value = args[1];
        if (typeof selector !== 'string' || typeof value !== 'string') {
          throw new Error('fill(selector, value) requires strings');
        }
        const options = (args[2] ?? undefined) as Parameters<Page['fill']>[2];
        return await page.fill(selector, value, options);
      }
      case 'type': {
        const selector = args[0];
        const text = args[1];
        if (typeof selector !== 'string' || typeof text !== 'string') {
          throw new Error('type(selector, text) requires strings');
        }
        const options = (args[2] ?? undefined) as Parameters<Page['type']>[2];
        return await page.type(selector, text, options);
      }
      case 'press': {
        const selector = args[0];
        const key = args[1];
        if (typeof selector !== 'string' || typeof key !== 'string') {
          throw new Error('press(selector, key) requires strings');
        }
        const options = (args[2] ?? undefined) as Parameters<Page['press']>[2];
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
    
    for (const ws of this.wsConnections) {
      try {
        ws.close();
      } catch { /* ignore */ }
    }
    this.wsConnections.clear();
  }
}
