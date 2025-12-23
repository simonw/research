import { ClientMessage, ServerMessage } from './types';

type CommandCallback = (commandId: string, result: unknown, error?: string) => void;

function trimTrailingUndefined(args: unknown[]): unknown[] {
  let end = args.length;
  while (end > 0 && args[end - 1] === undefined) {
    end -= 1;
  }
  return end === args.length ? args : args.slice(0, end);
}

function getCommandTimeoutMs(method: string, args: unknown[]): number {
  if (method === 'requestTakeover') {
    return 10 * 60 * 1000;
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

  return 30_000;
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

  async requestTakeover(message: string): Promise<void> {
    await this.execute('requestTakeover', [message]);
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
