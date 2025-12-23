const WORKER_URL = process.env.NEXT_PUBLIC_WORKER_URL || 'https://remote-browser.vana.workers.dev';
const API_KEY = process.env.NEXT_PUBLIC_API_KEY || 'dev-api-key-12345';

const headers = {
  'Content-Type': 'application/json',
  'Authorization': `Bearer ${API_KEY}`
};

async function handleResponse(res: Response, fallbackMessage: string) {
  if (!res.ok) {
    let errorMessage = fallbackMessage;
    try {
      const errorData = await res.json();
      errorMessage = errorData.error || fallbackMessage;
      if (errorData.stack) {
        errorMessage += `\n\nStack:\n${errorData.stack}`;
      }
    } catch {
      errorMessage = `${fallbackMessage}: ${await res.text()}`;
    }
    throw new Error(errorMessage);
  }
}

export async function createSession(): Promise<{ sessionId: string }> {
  const res = await fetch(`${WORKER_URL}/sessions`, {
    method: 'POST',
    headers
  });
  await handleResponse(res, 'Failed to create session');
  return res.json();
}

export interface ListedSession {
  sessionId: string;
  createdAt: number;
}

export async function listSessions(): Promise<{ sessions: ListedSession[] }> {
  const res = await fetch(`${WORKER_URL}/sessions`, {
    method: 'GET',
    headers
  });
  await handleResponse(res, 'Failed to list sessions');
  return res.json();
}

export async function runScript(sessionId: string, code: string): Promise<void> {
  const res = await fetch(`${WORKER_URL}/sessions/${sessionId}/script`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ code })
  });
  await handleResponse(res, 'Failed to run script');
}

export async function completeTakeover(sessionId: string): Promise<void> {
  const res = await fetch(`${WORKER_URL}/sessions/${sessionId}/done`, {
    method: 'POST',
    headers
  });
  await handleResponse(res, 'Failed to complete takeover');
}

export async function destroySession(sessionId: string): Promise<void> {
  const res = await fetch(`${WORKER_URL}/sessions/${sessionId}`, {
    method: 'DELETE',
    headers
  });
  await handleResponse(res, 'Failed to destroy session');
}

export function getWebSocketUrl(sessionId: string): string {
  const wsUrl = WORKER_URL.replace('https://', 'wss://').replace('http://', 'ws://');
  return `${wsUrl}/sessions/${sessionId}/ws`;
}
