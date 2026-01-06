import { ClientMessage, ServerMessage, InputSchema } from './types';

type CommandCallback = (commandId: string, result: unknown, error?: string) => void;

function trimTrailingUndefined(args: unknown[]): unknown[] {
  let end = args.length;
  while (end > 0 && args[end - 1] === undefined) {
    end -= 1;
  }
  return end === args.length ? args : args.slice(0, end);
}

function getCommandTimeoutMs(method: string, args: unknown[]): number {
  if (method === 'promptUser' || method === 'getInput' || method === 'solveCaptcha') {
    return 10 * 60 * 1000;
  }

  if (method === 'waitForNetworkCapture') {
    const timeout = args[1];
    return typeof timeout === 'number' ? timeout + 5_000 : 35_000;
  }

  if (method === 'waitForTimeout') {
    const ms = args[0];
    return typeof ms === 'number' ? ms + 5_000 : 30_000;
  }

  if (method === 'waitForSelector' || method === 'goto') {
    const options = args[1];
    if (options && typeof options === 'object' && 'timeout' in options) {
      const timeout = (options as { timeout?: unknown }).timeout;
      if (typeof timeout === 'number') {
        return timeout + 5_000;
      }
    }
  }

  return 60_000;
}

export class PlaywrightProxy {
  private commandId = 0;
  private pendingCommands = new Map<string, CommandCallback>();
  private sendMessage: (message: ClientMessage) => void;

  constructor(sendMessage: (message: ClientMessage) => void) {
    this.sendMessage = sendMessage;
  }

  handleServerMessage(message: ServerMessage): void {
    if (message.type === 'commandResult') {
      const callback = this.pendingCommands.get(message.commandId);
      if (callback) {
        callback(message.commandId, message.result, message.error);
        this.pendingCommands.delete(message.commandId);
      }
    }
  }

  createPageProxy(): PlaywrightPageProxy {
    return new PlaywrightPageProxy((method, args) => this.executeCommand(method, args));
  }

  private executeCommand(method: string, args: unknown[]): Promise<unknown> {
    return new Promise((resolve, reject) => {
      const commandId = `cmd_${++this.commandId}`;

      const trimmedArgs = trimTrailingUndefined(args);
      
      this.pendingCommands.set(commandId, (_id, result, error) => {
        if (error) {
          reject(new Error(error));
        } else {
          resolve(result);
        }
      });

      this.sendMessage({
        type: 'command',
        commandId,
        method,
        args: trimmedArgs
      });

      const timeoutMs = getCommandTimeoutMs(method, trimmedArgs);

      setTimeout(() => {
        if (this.pendingCommands.has(commandId)) {
          this.pendingCommands.delete(commandId);
          reject(new Error(`Command timeout: ${method}`));
        }
      }, timeoutMs);
    });
  }
}

export class PlaywrightPageProxy {
  private execute: (method: string, args: unknown[]) => Promise<unknown>;

  constructor(execute: (method: string, args: unknown[]) => Promise<unknown>) {
    this.execute = execute;
  }

  async goto(url: string, options?: unknown): Promise<unknown> {
    return this.execute('goto', [url, options]);
  }

  async click(selector: string, options?: unknown): Promise<unknown> {
    return this.execute('click', [selector, options]);
  }

  async fill(selector: string, value: string, options?: unknown): Promise<unknown> {
    return this.execute('fill', [selector, value, options]);
  }

  async type(selector: string, text: string, options?: unknown): Promise<unknown> {
    return this.execute('type', [selector, text, options]);
  }

  async press(selector: string, key: string, options?: unknown): Promise<unknown> {
    return this.execute('press', [selector, key, options]);
  }

  async waitForSelector(selector: string, options?: unknown): Promise<unknown> {
    return this.execute('waitForSelector', [selector, options]);
  }

  async waitForTimeout(timeout: number): Promise<unknown> {
    return this.execute('waitForTimeout', [timeout]);
  }

  async sleep(timeout: number): Promise<unknown> {
    return this.execute('waitForTimeout', [timeout]);
  }

  async screenshot(options?: unknown): Promise<unknown> {
    return this.execute('screenshot', [options]);
  }

  async title(): Promise<unknown> {
    return this.execute('title', []);
  }

  async url(): Promise<string> {
    const value = await this.execute('url', []);
    return typeof value === 'string' ? value : String(value);
  }

  async evaluate(pageFunction: string | Function, ...args: unknown[]): Promise<unknown> {
    const fnString = typeof pageFunction === 'function' 
      ? pageFunction.toString() 
      : pageFunction;
    return this.execute('evaluate', [fnString, ...args]);
  }

  getByPlaceholder(text: string): PlaywrightLocatorProxy {
    return new PlaywrightLocatorProxy(this.execute, 'getByPlaceholder', [text]);
  }

  getByText(text: string): PlaywrightLocatorProxy {
    return new PlaywrightLocatorProxy(this.execute, 'getByText', [text]);
  }

  getByRole(role: string, options?: unknown): PlaywrightLocatorProxy {
    return new PlaywrightLocatorProxy(this.execute, 'getByRole', trimTrailingUndefined([role, options]));
  }

  async captureNetwork(arg1: string | { key: string; urlPattern?: string; bodyPattern?: string }, arg2?: string): Promise<void> {
    if (typeof arg1 === 'string') {
      await this.execute('captureNetwork', [arg1, arg2]);
    } else {
      await this.execute('captureNetwork', [arg1]);
    }
  }

  async clearNetworkCaptures(): Promise<void> {
    await this.execute('clearNetworkCaptures', []);
  }

  async getCapturedResponse(key: string): Promise<unknown> {
    return this.execute('getCapturedResponse', [key]);
  }

  async waitForNetworkCapture(key: string, timeout?: number): Promise<unknown> {
    return this.execute('waitForNetworkCapture', [key, timeout]);
  }

  async setData(key: string, value: unknown): Promise<void> {
    await this.execute('setData', [key, value]);
  }

  async getData(): Promise<Record<string, unknown>> {
    const result = await this.execute('getData', []);
    return (result as Record<string, unknown>) || {};
  }

  async clearData(): Promise<void> {
    await this.execute('clearData', []);
  }

  async scrapeText(selector: string): Promise<string | null> {
    const result = await this.execute('scrapeText', [selector]);
    return result as string | null;
  }

  async scrapeAttribute(selector: string, attribute: string): Promise<string | null> {
    const result = await this.execute('scrapeAttribute', [selector, attribute]);
    return result as string | null;
  }

  async scrapeAll(selector: string, options?: { text?: boolean; attribute?: string }): Promise<(string | null)[]> {
    const result = await this.execute('scrapeAll', [selector, options]);
    return result as (string | null)[];
  }

  async exists(selector: string): Promise<boolean> {
    const result = await this.execute('exists', [selector]);
    return !!result;
  }

  async count(selector: string): Promise<number> {
    const result = await this.execute('count', [selector]);
    return (result as number) || 0;
  }

  async waitForURL(url: string | RegExp | ((url: string) => boolean), options?: { timeout?: number; waitUntil?: 'load' | 'domcontentloaded' | 'networkidle' | 'commit' }): Promise<void> {
    if (typeof url === 'string') {
      const timeout = options?.timeout || 30000;
      const start = Date.now();
      while (Date.now() - start < timeout) {
        try {
          const currentUrl = await this.url();
          if (currentUrl.includes(url)) return;
        } catch (e) {}
        await this.sleep(1000);
      }
      throw new Error(`Timeout waiting for URL: ${url}`);
    }
    await this.sleep(2000);
  }

  async $(selector: string): Promise<any> {
    const exists = await this.execute('exists', [selector]);
    if (!exists) return null;
    return {
      textContent: () => this.execute('scrapeText', [selector]),
      getAttribute: (name: string) => this.execute('scrapeAttribute', [selector, name]),
      click: (options?: any) => this.execute('click', [selector, options]),
      fill: (value: string, options?: any) => this.execute('fill', [selector, value, options]),
    };
  }

  async $$(selector: string): Promise<any[]> {
    const count = await this.execute('count', [selector]) as number;
    const results = [];
    for (let i = 0; i < count; i++) {
      results.push({
        textContent: () => this.execute('scrapeText', [`(${selector}) >> nth=${i}`]),
        getAttribute: (name: string) => this.execute('scrapeAttribute', [`(${selector}) >> nth=${i}`, name]),
      });
    }
    return results;
  }

  async promptUser(message: string, ...args: unknown[]): Promise<void> {
    let conditionFn: (() => Promise<boolean>) | undefined;
    let pollInterval = 2000;
    let options: any = {};

    if (args.length > 0) {
      if (typeof args[0] === 'function') {
        conditionFn = args[0] as () => Promise<boolean>;
        if (typeof args[1] === 'number') pollInterval = args[1];
      } else if (typeof args[0] === 'object') {
        options = args[0];
      }
    }

    if (conditionFn) {
      await this.execute('promptUser', [message, { ...options, nonBlocking: true }]);
      
      while (true) {
        let result = false;
        try {
          result = await conditionFn();
        } catch (e) {
          console.error('Error in promptUser condition:', e);
        }
        
        if (result) break;
        await new Promise(r => setTimeout(r, pollInterval));
      }
      
      await this.execute('completeTakeover', []);
    } else {
      await this.execute('promptUser', [message, options]);
    }
  }

  async getInput(inputSchema: InputSchema): Promise<Record<string, unknown>> {
    return this.execute('getInput', [inputSchema]) as Promise<Record<string, unknown>>;
  }

  async solveCaptcha(options?: {
    type?: 'recaptcha_v2' | 'recaptcha_v3' | 'hcaptcha' | 'turnstile';
    sitekey?: string;
    enterprise?: boolean;
    action?: string;
  }): Promise<{ success: boolean; fallback?: boolean }> {
    return this.execute('solveCaptcha', [options]) as Promise<{ success: boolean; fallback?: boolean }>;
  }
}

export class PlaywrightLocatorProxy {
  private execute: (method: string, args: unknown[]) => Promise<unknown>;
  private locatorMethod: string;
  private locatorArgs: unknown[];

  constructor(
    execute: (method: string, args: unknown[]) => Promise<unknown>,
    locatorMethod: string,
    locatorArgs: unknown[]
  ) {
    this.execute = execute;
    this.locatorMethod = locatorMethod;
    this.locatorArgs = locatorArgs;
  }

  async fill(value: string): Promise<unknown> {
    return this.execute(`${this.locatorMethod}.fill`, [...this.locatorArgs, value]);
  }

  async press(key: string): Promise<unknown> {
    return this.execute(`${this.locatorMethod}.press`, [...this.locatorArgs, key]);
  }

  async click(options?: unknown): Promise<unknown> {
    return this.execute(`${this.locatorMethod}.click`, [...this.locatorArgs, options]);
  }
}
