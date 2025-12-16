export interface Session {
  id: string;
  userId: string;
  vmName: string;
  zone: string;
  ip: string | null;
  status: 'starting' | 'running' | 'ended' | 'error';
  createdAt: number;
  expiresAt: number;
  kasmReady: boolean;
}

// Progress tracking for VM startup stages
export interface VMProgress {
  stage: string;
  step: number;
  totalSteps: number;
  message: string;
  percent: number;
  timestamp?: number;
  tunnelUrl?: string;  // Cloudflare tunnel URL for HTTPS access
  elapsedSeconds?: number;  // Time elapsed since startup began
}

export interface StartSessionRequest {
  userId?: string;
}

export interface StartSessionResponse {
  sessionId: string;
  status: 'starting';
}

export interface SessionStatusResponse {
  session: Session;
  url: string | null;
  progress: VMProgress | null;
}

export interface EndSessionRequest {
  sessionId: string;
}

export interface EndSessionResponse {
  success: boolean;
  message: string;
}

// VM Configuration
export interface VMConfig {
  projectId: string;
  zone: string;
  machineType: string;
  diskSizeGb: number;
  ttlMinutes: number;
}

