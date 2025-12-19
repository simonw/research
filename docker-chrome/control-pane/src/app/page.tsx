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
    <div className="min-h-screen bg-background overflow-x-hidden">
      <div className="container mx-auto px-4 py-8 sm:px-6 sm:py-12 lg:px-8 lg:py-16 max-w-[1280px]">
        <div className="lg:grid lg:grid-cols-[minmax(0,55%)_minmax(0,45%)] lg:gap-6 xl:gap-10">
          {/* Left Column: Sticky BrowserFrame (Desktop) */}
          <div className="hidden lg:block lg:sticky lg:top-8 lg:self-start">
            <div className="flex justify-center py-4">
              <BrowserFrame url={API_BASE} />
            </div>
          </div>

          {/* Right Column: Header + Mobile Frame + Controls */}
          <div className="space-y-6 lg:space-y-8 min-w-0">
            {/* Header */}
            <div className="text-center lg:text-left">
              <h1 className="text-2xl font-semibold text-foreground tracking-tight mb-1">
                Docker Chrome
              </h1>
              <p className="text-sm text-text-secondary">
                Remote browser control with network inspection
              </p>
            </div>

            {/* Mobile BrowserFrame */}
            <div className="lg:hidden flex justify-center">
              <BrowserFrame url={API_BASE} />
            </div>

            {/* WebSocket Status */}
            <div className="bg-surface border border-border rounded-lg p-4">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium text-foreground">Connection Status</span>
                <div className="flex items-center gap-2">
                  <div className={`w-2 h-2 rounded-full ${wsConnected ? 'bg-success' : 'bg-error'}`} />
                  <span className="text-xs text-text-secondary font-mono">
                    {wsConnected ? 'Connected' : 'Disconnected'}
                  </span>
                </div>
              </div>
            </div>

            {/* Network Panel */}
            <div className="min-h-0">
              <NetworkPanel requests={requests} />
            </div>

            {/* Control Panel */}
            <div className="min-h-0">
              <ControlPanel status={status} onRefreshStatus={fetchStatus} onReset={handleReset} />
            </div>

            {/* Info Footer */}
            <div className="text-center lg:text-left text-text-tertiary text-xs pt-4 border-t border-border">
              <p className="leading-relaxed">
                Real-time network monitoring • CDP-based browser control
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
