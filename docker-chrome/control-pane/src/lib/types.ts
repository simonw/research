export interface NetworkRequest {
  requestId: string;
  url: string;
  method: string;
  status?: number;
  type: string;
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
