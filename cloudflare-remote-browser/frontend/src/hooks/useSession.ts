'use client';

import { useState, useCallback, useRef, useEffect } from 'react';
import { SessionState, ServerMessage, ClientMessage, NetworkRequest, AutomationState } from '@/lib/types';
import { PlaywrightProxy } from '@/lib/playwright-proxy';
import * as api from '@/lib/api';

export interface CursorState {
  x: number;
  y: number;
  action: 'move' | 'click' | 'idle';
  timestamp: number;
}

interface ExtendedSessionState extends SessionState {
  networkRequests: NetworkRequest[];
  automationData: Record<string, unknown>;
  viewport: { width: number; height: number };
  agentCursor: CursorState | null;
}

const INITIAL_STATE: ExtendedSessionState = {
  sessionId: null,
  status: 'idle',
  takeoverMessage: '',
  error: '',
  result: null,
  connected: false,
  networkRequests: [],
  automationData: {},
  viewport: { width: 800, height: 600 },
  agentCursor: null
};

export function useSession() {
  const [state, setState] = useState<ExtendedSessionState>(INITIAL_STATE);
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
            takeoverMessage: message.message || '',
          }));
          break;
        case 'result':
          setState(s => ({ ...s, result: message.data }));
          break;
        case 'error':
          setState(s => ({ ...s, error: message.message }));
          break;
        case 'network:request':
          setState(s => ({
            ...s,
            networkRequests: [...s.networkRequests, message.request]
          }));
          break;
        case 'network:response':
          setState(s => ({
            ...s,
            networkRequests: s.networkRequests.map(req =>
              req.requestId === message.requestId
                ? { 
                    ...req, 
                    status: message.status, 
                    statusText: message.statusText, 
                    mimeType: message.mimeType,
                    responseTimestamp: Date.now()
                  }
                : req
            )
          }));
          break;
        case 'network:finished':
          if (message.capturedByKey) {
            setState(s => ({
              ...s,
              networkRequests: s.networkRequests.map(req =>
                req.requestId === message.requestId
                  ? { ...req, capturedByKey: message.capturedByKey }
                  : req
              )
            }));
          }
          break;
        case 'network:failed':
          setState(s => ({
            ...s,
            networkRequests: s.networkRequests.map(req =>
              req.requestId === message.requestId
                ? { ...req, errorText: message.errorText }
                : req
            )
          }));
          break;
        case 'network:clear':
          setState(s => ({ ...s, networkRequests: [] }));
          break;
        case 'automation:data':
          setState(s => ({
            ...s,
            automationData: { ...s.automationData, [message.key]: message.value }
          }));
          break;
        case 'automation:cursor':
          setState(s => ({
            ...s,
            agentCursor: {
              x: message.x,
              y: message.y,
              action: message.action,
              timestamp: Date.now()
            }
          }));
          break;
        case 'viewport:ack':
          setState(s => ({
            ...s,
            viewport: { width: message.width, height: message.height }
          }));
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

  // Destroy session but keep captured data and network requests
  const destroySessionKeepData = useCallback(async (sessionId: string) => {
    try {
      await api.destroySession(sessionId);
    } catch { /* cleanup */ }
    wsRef.current?.close();
    wsRef.current = null;
    playwrightProxyRef.current = null;
    // Keep networkRequests and automationData, just clear session-specific state
    setState(s => ({
      ...s,
      sessionId: null,
      connected: false,
      takeoverMessage: '',
      // Keep: networkRequests, automationData, result, error, status
    }));
  }, []);

  const createSession = useCallback(async () => {
    try {
      // Clear all data when starting a new session
      setState(s => ({ 
        ...INITIAL_STATE,
        status: 'starting'
      }));
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
    // Clear data when attaching to a different session
    setState({ 
      ...INITIAL_STATE, 
      sessionId, 
      status: 'idle' 
    });
    connect(sessionId);
  }, [connect]);

  const listSessions = useCallback(async () => {
    return api.listSessions();
  }, []);

  const runScript = useCallback(async (code: string) => {
    if (!state.sessionId || !playwrightProxyRef.current) return;
    
    const currentSessionId = state.sessionId;
    
    try {
      setState(s => ({ ...s, error: '', status: 'running' }));
      await api.runScript(currentSessionId, code);

      const page = playwrightProxyRef.current.createPageProxy();
      
      const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
      const scriptFn = new AsyncFunction('page', code);

      const result = await scriptFn(page);
      
      await api.finishScript(currentSessionId, result);
      
      setState(s => ({ ...s, status: 'done', result }));
      
      // Kill the session after successful completion, but keep data
      await destroySessionKeepData(currentSessionId);
    } catch (e) {
      const errorMessage = (e as Error).message;
      setState(s => ({ ...s, error: errorMessage, status: 'error' }));
      
      // Kill the session after error, but keep data
      await destroySessionKeepData(currentSessionId);
    }
  }, [state.sessionId, destroySessionKeepData]);

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

  const clearNetworkRequests = useCallback(() => {
    setState(s => ({ ...s, networkRequests: [] }));
  }, []);

  // Keep sessionId in a ref for polling to avoid stale closures
  const sessionIdRef = useRef<string | null>(null);
  useEffect(() => {
    sessionIdRef.current = state.sessionId;
  }, [state.sessionId]);

  // Poll status every 3 seconds to keep UI synchronized
  useEffect(() => {
    if (!state.sessionId || !state.connected) {
      return;
    }

    const pollStatus = async () => {
      const currentSessionId = sessionIdRef.current;
      if (!currentSessionId) return;
      
      try {
        const data = await api.getStatus(currentSessionId);
        setState(s => ({
          ...s,
          status: data.status,
          takeoverMessage: data.takeoverMessage || s.takeoverMessage,
        }));
      } catch (e) {
        // Don't update error state on poll failures - WebSocket will handle real errors
      }
    };

    // Poll immediately, then every 3 seconds
    pollStatus();
    const interval = setInterval(pollStatus, 3000);

    return () => {
      clearInterval(interval);
    };
  }, [state.sessionId, state.connected]);

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
    setOnFrame,
    clearNetworkRequests
  };
}
