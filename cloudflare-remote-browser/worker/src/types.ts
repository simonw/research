export interface Env {
  BROWSER: Fetcher;
  BROWSER_SESSION: DurableObjectNamespace;
  API_KEY: string;
}

export type SessionStatus = 'idle' | 'starting' | 'running' | 'takeover' | 'done' | 'error';

export interface SessionState {
  status: SessionStatus;
  takeoverMessage?: string;
  error?: string;
  result?: unknown;
}

export type ServerMessage =
  | { type: 'frame'; data: string; timestamp: number }
  | { type: 'status'; status: SessionStatus; message?: string }
  | { type: 'result'; data: unknown }
  | { type: 'error'; message: string }
  | { type: 'commandResult'; commandId: string; result: unknown; error?: string };

export type ClientMessage =
  | { type: 'mouse'; action: string; x: number; y: number; button?: string }
  | { type: 'keyboard'; action: string; key: string; text?: string }
  | { type: 'done' }
  | { type: 'scroll'; deltaX: number; deltaY: number; x: number; y: number }
  | { type: 'command'; commandId: string; method: string; args: unknown[] };
