"use client";

import React, { useEffect, useState, useRef } from "react";
import { BrowserFrame } from "@/components/browser-frame";
import { NetworkPanel } from "@/components/network-panel";
import { ControlPanel } from "@/components/control-panel";
import { NetworkRequest, Status, WebSocketMessage } from "@/lib/types";

const WS_URL = "wss://docker-chrome-432753364585.us-central1.run.app/ws";
const API_BASE = "https://docker-chrome-432753364585.us-central1.run.app";

export default function Home() {
  const [requests, setRequests] = useState<NetworkRequest[]>([]);
  const [status, setStatus] = useState<Status | null>(null);
  const [wsConnected, setWsConnected] = useState(false);
  const activeSessionIdRef = useRef<string | null>(null);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout>(null);
  const wsRef = useRef<WebSocket | null>(null);

  const fetchStatus = async () => {
    try {
      const res = await fetch(`${API_BASE}/api/status`);
      if (res.ok) {
        setStatus(await res.json());
      }
    } catch (e) {
      console.error("Failed to fetch status", e);
    }
  };

  const handleReset = () => {
    setRequests([]);
    activeSessionIdRef.current = null;
  };

  const handleClearNetworkActivity = async () => {
    // Clear local state
    setRequests([]);

    // Clear server-side network cache
    try {
      await fetch(`${API_BASE}/api/network/clear`, { method: 'POST' });
    } catch (e) {
      console.error('Failed to clear network cache on server', e);
    }
  };

  useEffect(() => {
    fetchStatus();

    const connectWs = () => {
      if (wsRef.current?.readyState === WebSocket.OPEN) return;

      const ws = new WebSocket(WS_URL);
      wsRef.current = ws;

      ws.onopen = () => {
        setWsConnected(true);
        console.log("WebSocket Connected");
      };

      ws.onclose = () => {
        setWsConnected(false);
        console.log("WebSocket Disconnected");
        reconnectTimeoutRef.current = setTimeout(connectWs, 3000);
      };

      ws.onmessage = (event) => {
        try {
          const msg: WebSocketMessage = JSON.parse(event.data);

          if (msg.type === "SESSION_RESET") {
            setRequests([]);
            activeSessionIdRef.current = msg.payload.sessionId;
            fetchStatus();
            return;
          }

          if (msg.type === "NETWORK_REQUEST" || msg.type === "NETWORK_RESPONSE" || msg.type === "NETWORK_FAILED") {
            const payload = msg.payload as NetworkRequest;

            if (activeSessionIdRef.current && payload.sessionId && activeSessionIdRef.current !== payload.sessionId) {
              return;
            }

            setRequests((prev) => {
              const existsIndex = prev.findIndex(r => r.requestId === payload.requestId);

              if (existsIndex !== -1) {
                const newReqs = [...prev];
                newReqs[existsIndex] = { ...newReqs[existsIndex], ...payload };
                return newReqs;
              }

              const next = [...prev, payload];
              if (next.length > 200) next.shift();
              return next;
            });
          }
        } catch (e) {
          console.error("Failed to parse WS message", e);
        }
      };
    };

    connectWs();

    return () => {
      if (wsRef.current) wsRef.current.close();
      if (reconnectTimeoutRef.current) clearTimeout(reconnectTimeoutRef.current);
    };
  }, []);

  return (
    <div className="min-h-screen bg-background p-6">
      <div className="container mx-auto max-w-[1400px] space-y-6">
        {/* Header */}
        <header className="text-center lg:text-left">
          <h1 className="text-2xl font-semibold text-foreground tracking-tight mb-1">
            Docker Chrome
          </h1>
          <p className="text-sm text-text-secondary">
            Remote browser control with network inspection
          </p>
        </header>

        {/* Top Row: Browser + Controls */}
        <div className="grid lg:grid-cols-2 gap-6">
          {/* Left: Browser Viewer */}
          <div className="flex items-start">
            <BrowserFrame url={API_BASE} />
          </div>

          {/* Right: Controls */}
          <div className="space-y-4">
            {/* Connection Status with Refresh */}
            <div className="bg-surface border border-border rounded-lg p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2.5">
                  <div className={`w-2 h-2 rounded-full ${wsConnected ? 'bg-success' : 'bg-error'}`} />
                  <span className="text-sm font-medium text-foreground">
                    {wsConnected ? 'Connected' : 'Disconnected'}
                  </span>
                </div>
                <button
                  onClick={fetchStatus}
                  className="p-2 hover:bg-background rounded-lg transition-colors"
                  title="Refresh Status"
                >
                  <svg className="w-4 h-4 text-text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                  </svg>
                </button>
              </div>
            </div>

            {/* Control Panel */}
            <ControlPanel status={status} onRefreshStatus={fetchStatus} onReset={handleReset} />
          </div>
        </div>

        {/* Bottom Row: Network Activity (full width) */}
        <div>
          <NetworkPanel requests={requests} onClearActivity={handleClearNetworkActivity} />
        </div>
      </div>
    </div>
  );
}
