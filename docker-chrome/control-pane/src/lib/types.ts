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
