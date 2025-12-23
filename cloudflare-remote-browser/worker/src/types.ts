export interface Env {
  BROWSER: Fetcher;
  BROWSER_SESSION: DurableObjectNamespace;
  SESSION_REGISTRY: DurableObjectNamespace;
  API_KEY: string;
}

export type SessionStatus = 'idle' | 'starting' | 'running' | 'takeover' | 'done' | 'error';

export type AutomationMode = 'idle' | 'automation' | 'user-input';

export interface SessionState {
  status: SessionStatus;
  takeoverMessage?: string;
  error?: string;
  result?: unknown;
}

export type ResourceType =
  | 'Document'
  | 'Stylesheet'
  | 'Image'
  | 'Media'
  | 'Font'
  | 'Script'
  | 'TextTrack'
  | 'XHR'
  | 'Fetch'
  | 'Prefetch'
  | 'EventSource'
  | 'WebSocket'
  | 'Manifest'
  | 'SignedExchange'
  | 'Ping'
  | 'CSPViolationReport'
  | 'Preflight'
  | 'Other';

export interface NetworkRequest {
  requestId: string;
  url: string;
  method: string;
  status?: number;
  statusText?: string;
  type: ResourceType;
  mimeType?: string;
  errorText?: string;
  timestamp: number;
  responseTimestamp?: number;
  capturedByKey?: string;
}

export interface NetworkRequestDetails {
  requestHeaders: Record<string, string>;
  requestBody?: string;
  responseHeaders: Record<string, string>;
  responseBody?: string;
  base64Encoded: boolean;
}

export interface StoredNetworkRequest extends NetworkRequest {
  requestHeaders: Record<string, string>;
  requestPostData?: string;
  responseHeaders?: Record<string, string>;
  responseBody?: string;
  responseBase64Encoded?: boolean;
}

export interface AutomationState {
  mode: AutomationMode;
  isRunning: boolean;
  scriptId: string | null;
  prompt: string | null;
  error: string | null;
  data: Record<string, unknown>;
  cursorPosition?: { x: number; y: number } | null;
  cursorAction?: 'move' | 'click';
}

export type ServerMessage =
  | { type: 'frame'; data: string; timestamp: number }
  | { type: 'status'; status: SessionStatus; message?: string }
  | { type: 'result'; data: unknown }
  | { type: 'error'; message: string }
  | { type: 'commandResult'; commandId: string; result: unknown; error?: string }
  | { type: 'network:request'; request: NetworkRequest }
  | { type: 'network:response'; requestId: string; status: number; statusText: string; mimeType: string; responseHeaders: Record<string, string> }
  | { type: 'network:finished'; requestId: string; capturedByKey?: string }
  | { type: 'network:failed'; requestId: string; errorText: string }
  | { type: 'network:clear' }
  | { type: 'automation:state'; state: AutomationState }
  | { type: 'automation:data'; key: string; value: unknown }
  | { type: 'automation:cursor'; x: number; y: number; action: 'move' | 'click' }
  | { type: 'viewport:ack'; width: number; height: number };

export type ClientMessage =
  | { type: 'mouse'; action: string; x: number; y: number; button?: string }
  | { type: 'keyboard'; action: string; key: string; text?: string; code?: string; keyCode?: number }
  | { type: 'paste'; text: string }
  | { type: 'done' }
  | { type: 'scroll'; deltaX: number; deltaY: number; x: number; y: number }
  | { type: 'command'; commandId: string; method: string; args: unknown[] }
  | { type: 'viewport'; width: number; height: number };

export class LRUCache<K, V> {
  private maxSize: number;
  private cache: Map<K, V>;

  constructor(maxSize: number) {
    this.maxSize = maxSize;
    this.cache = new Map();
  }

  get(key: K): V | undefined {
    const value = this.cache.get(key);
    if (value !== undefined) {
      this.cache.delete(key);
      this.cache.set(key, value);
    }
    return value;
  }

  set(key: K, value: V): void {
    if (this.cache.has(key)) {
      this.cache.delete(key);
    } else if (this.cache.size >= this.maxSize) {
      const firstKey = this.cache.keys().next().value;
      if (firstKey !== undefined) {
        this.cache.delete(firstKey);
      }
    }
    this.cache.set(key, value);
  }

  has(key: K): boolean {
    return this.cache.has(key);
  }

  delete(key: K): boolean {
    return this.cache.delete(key);
  }

  clear(): void {
    this.cache.clear();
  }

  size(): number {
    return this.cache.size;
  }

  values(): IterableIterator<V> {
    return this.cache.values();
  }

  entries(): IterableIterator<[K, V]> {
    return this.cache.entries();
  }
}
