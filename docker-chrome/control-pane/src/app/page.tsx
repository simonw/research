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
    <main className="h-screen w-full flex flex-col overflow-hidden p-6 gap-6">
      <header className="flex items-center justify-between pb-4 border-b border-border">
        <h1 className="text-2xl font-bold tracking-tight text-foreground flex items-center gap-2">
          <span className="text-accent">Docker</span>Chrome
        </h1>
        <div className="flex items-center gap-4 text-xs font-mono">
          <div className="flex items-center gap-2">
            <span className={`w-2 h-2 rounded-full ${wsConnected ? 'bg-green-500' : 'bg-red-500'}`} />
            <span className="text-zinc-500">WS {wsConnected ? 'LIVE' : 'OFFLINE'}</span>
          </div>
        </div>
      </header>

      <div className="flex-1 grid grid-cols-1 lg:grid-cols-12 gap-6 min-h-0">
        <div className="lg:col-span-7 flex flex-col gap-6 h-full min-h-0">
          <div className="flex-none flex justify-center py-4 bg-zinc-900/30 rounded-xl border border-border/50">
            <BrowserFrame url={API_BASE} />
          </div>
          
          <div className="flex-1 min-h-0">
            <NetworkPanel requests={requests} />
          </div>
        </div>

        <div className="lg:col-span-5 h-full min-h-0">
          <ControlPanel status={status} onRefreshStatus={fetchStatus} onReset={handleReset} />
        </div>
      </div>
    </main>
  );
}
