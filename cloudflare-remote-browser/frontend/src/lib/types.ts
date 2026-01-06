export type SessionStatus = 'idle' | 'starting' | 'running' | 'takeover' | 'done' | 'error';

export type AutomationMode = 'idle' | 'automation' | 'user-input';

export interface InputFieldSchema {
  type: 'string' | 'boolean' | 'number';
  title: string;
  description?: string;
  enum?: string[];
  enumNames?: string[];
  default?: string | boolean | number;
  minLength?: number;
  maxLength?: number;
  pattern?: string;
}

export interface InputSchema {
  schema: {
    type: 'object';
    required?: string[];
    properties: Record<string, InputFieldSchema>;
  };
  uiSchema?: Record<string, {
    'ui:widget'?: 'text' | 'password' | 'textarea' | 'radio' | 'select' | 'checkbox';
    'ui:placeholder'?: string;
    'ui:autofocus'?: boolean;
    'ui:help'?: string;
  }>;
  title?: string;
  description?: string;
  submitLabel?: string;
  cancelLabel?: string;
  error?: string;
  errors?: Record<string, string>;
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
  | { type: 'viewport:ack'; width: number; height: number }
  | { type: 'input:request'; requestId: string; input: InputSchema }
  | { type: 'input:cancelled'; requestId: string };

export type ClientMessage =
  | { type: 'mouse'; action: string; x: number; y: number; button?: string }
  | { type: 'keyboard'; action: string; key: string; text?: string; code?: string; keyCode?: number }
  | { type: 'paste'; text: string }
  | { type: 'done' }
  | { type: 'scroll'; deltaX: number; deltaY: number; x: number; y: number }
  | { type: 'command'; commandId: string; method: string; args: unknown[] }
  | { type: 'viewport'; width: number; height: number }
  | { type: 'input:response'; requestId: string; values: Record<string, unknown> }
  | { type: 'input:cancel'; requestId: string };
