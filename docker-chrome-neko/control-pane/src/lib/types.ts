// Session and progress tracking for VM deployment
export interface Session {
  id: string;
  vmName: string;
  zone: string;
  ip: string | null;
  status: 'starting' | 'running' | 'ended' | 'error';
  createdAt: number;
  expiresAt: number;
}

export interface VMProgress {
  stage: string;
  step: number;
  totalSteps: number;
  message: string;
  percent: number;
  timestamp?: number;
  tunnelUrl?: string;
  cdpAgentUrl?: string;  // CDP agent Cloudflare tunnel URL
  elapsedSeconds?: number;
}

// VM Configuration
export interface VMConfig {
  projectId: string;
  zone: string;
  machineType: string;
  diskSizeGb: number;
  ttlMinutes: number;
}

// CDP Agent types
export interface CDPStatus {
  cdpConnected: boolean;
  cdpPort: number;
  cdpHost: string;
  wsPath: string;
  viewport: { width: number; height: number };
  persistentScriptCount: number;
  sessionId: string | null;
}

export interface NetworkRequest {
  requestId: string;
  url: string;
  method: string;
  type: string;
  status?: number;
  mimeType?: string;
  timestamp: number;
}

export interface PersistentScript {
  id: number;
  code: string;
}
