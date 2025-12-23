'use client';

import { useState, useCallback, useRef, useEffect } from 'react';
import { SessionState, ServerMessage, ClientMessage } from '@/lib/types';
import { PlaywrightProxy } from '@/lib/playwright-proxy';
import * as api from '@/lib/api';

const INITIAL_STATE: SessionState = {
  sessionId: null,
  status: 'idle',
  takeoverMessage: '',
  error: '',
  result: null,
  connected: false
};

export function useSession() {
  const [state, setState] = useState<SessionState>(INITIAL_STATE);
  const wsRef = useRef<WebSocket | null>(null);
  const onFrameRef = useRef<((data: string) => void) | null>(null);
  const playwrightProxyRef = useRef<PlaywrightProxy | null>(null);

  const sendMessage = useCallback((message: ClientMessage) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message));
    }
  }, []);

  const connect = useCallback((sessionId: string) => {
    const ws = new WebSocket(api.getWebSocketUrl(sessionId));
    wsRef.current = ws;

    playwrightProxyRef.current = new PlaywrightProxy(sendMessage);

    ws.onopen = () => {
      setState(s => ({ ...s, connected: true }));
    };

    ws.onmessage = (event) => {
      const message: ServerMessage = JSON.parse(event.data);
      
      playwrightProxyRef.current?.handleServerMessage(message);
      
      switch (message.type) {
        case 'frame':
          onFrameRef.current?.(message.data);
          break;
        case 'status':
          setState(s => ({
            ...s,
            status: message.status,
            takeoverMessage: message.message || ''
          }));
          break;
        case 'result':
          setState(s => ({ ...s, result: message.data }));
          break;
        case 'error':
          setState(s => ({ ...s, error: message.message }));
          break;
      }
    };

    ws.onclose = () => {
      setState(s => ({ ...s, connected: false }));
    };

    ws.onerror = () => {
      setState(s => ({ ...s, error: 'WebSocket connection error' }));
    };
  }, [sendMessage]);

  const createSession = useCallback(async () => {
    try {
      setState(s => ({ ...s, status: 'starting', error: '' }));
      const { sessionId } = await api.createSession();
      setState(s => ({ ...s, sessionId }));
      connect(sessionId);
    } catch (e) {
      setState(s => ({ ...s, error: (e as Error).message, status: 'error' }));
    }
  }, [connect]);

  const attachSession = useCallback((sessionId: string) => {
    wsRef.current?.close();
    wsRef.current = null;
    playwrightProxyRef.current = null;
    setState(s => ({ ...s, sessionId, connected: false, error: '', status: 'idle', takeoverMessage: '', result: null }));
    connect(sessionId);
  }, [connect]);

  const listSessions = useCallback(async () => {
    return api.listSessions();
  }, []);

  const runScript = useCallback(async (code: string) => {
    if (!state.sessionId || !playwrightProxyRef.current) return;
    
    try {
      setState(s => ({ ...s, error: '', status: 'running' }));
      await api.runScript(state.sessionId, code);

      const page = playwrightProxyRef.current.createPageProxy();
      
      const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
      const scriptFn = new AsyncFunction('page', 'requestTakeover', code);
      
      const requestTakeover = async (message: string) => {
        await page.requestTakeover(message);
      };

      const result = await scriptFn(page, requestTakeover);
      
      setState(s => ({ ...s, status: 'done', result }));
    } catch (e) {
      const errorMessage = (e as Error).message;
      setState(s => ({ ...s, error: errorMessage, status: 'error' }));
    }
  }, [state.sessionId]);

  const completeTakeover = useCallback(async () => {
    if (!state.sessionId) return;
    
    try {
      await api.completeTakeover(state.sessionId);
    } catch (e) {
      setState(s => ({ ...s, error: (e as Error).message }));
    }
  }, [state.sessionId]);

  const destroySession = useCallback(async () => {
    if (state.sessionId) {
      try {
        await api.destroySession(state.sessionId);
      } catch { /* cleanup */ }
    }
    wsRef.current?.close();
    wsRef.current = null;
    playwrightProxyRef.current = null;
    setState(INITIAL_STATE);
  }, [state.sessionId]);

  const sendInput = useCallback((message: ClientMessage) => {
    sendMessage(message);
  }, [sendMessage]);

  const setOnFrame = useCallback((callback: (data: string) => void) => {
    onFrameRef.current = callback;
  }, []);

  useEffect(() => {
    return () => {
      wsRef.current?.close();
    };
  }, []);

  return {
    ...state,
    createSession,
    attachSession,
    listSessions,
    runScript,
    completeTakeover,
    destroySession,
    sendInput,
    setOnFrame
  };
}
