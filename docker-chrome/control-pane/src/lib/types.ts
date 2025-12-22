export type ResourceType =
  | "Document"
  | "Stylesheet"
  | "Image"
  | "Media"
  | "Font"
  | "Script"
  | "TextTrack"
  | "XHR"
  | "Fetch"
  | "Prefetch"
  | "EventSource"
  | "WebSocket"
  | "Manifest"
  | "SignedExchange"
  | "Ping"
  | "CSPViolationReport"
  | "Preflight"
  | "FedCM"
  | "Other";

export interface NetworkRequest {
  requestId: string;
  url?: string;
  method?: string;
  status?: number;
  type?: ResourceType;
  mimeType?: string;
  errorText?: string;
  sessionId?: string;
  timestamp: number;
  capturedByKey?: string;
}

export interface NetworkRequestDetails {
  requestHeaders: Record<string, string>;
  requestBody: string;
  responseHeaders: Record<string, string>;
  responseBody: string;
  base64Encoded: boolean;
}

export interface Status {
  cdpConnected: boolean;
  cdpPort: number;
  wsPath: string;
  persistentScriptCount: number;
}

export interface PersistentScript {
  id: string;
  code: string;
}

export interface WebSocketMessage {
  type: string;
  payload: any;
}

// Automation types
export type AutomationMode = "idle" | "automation" | "user-input";

export interface AutomationState {
  mode: AutomationMode;
  isRunning: boolean;
  scriptId: string | null;
  prompt: string | null;
  error: string | null;
  data: Record<string, any>;
  cursorPosition?: { x: number; y: number } | null;
  cursorAction?: "move" | "click";
}

export interface AutomationCursorEvent {
  x: number;
  y: number;
  action: "move" | "click";
  timestamp: number;
}
