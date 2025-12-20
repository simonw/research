"use client";

import React, { useEffect, useState, useRef, useCallback } from "react";
import { BrowserFrame } from "@/components/browser-frame";
import { NetworkPanel } from "@/components/network-panel";
import { ControlPanel } from "@/components/control-panel";
import { DataPanel } from "@/components/data-panel";
import { NetworkRequest, Status, WebSocketMessage, AutomationMode, AutomationState } from "@/lib/types";
import { ChevronDown, Cloud, Server } from "lucide-react";

// Deployment targets configuration
const CLOUD_RUN_BASE = "docker-chrome-432753364585.us-central1.run.app";

// Predefined targets that users can switch between
const PREDEFINED_TARGETS = [
  {
    id: "cloudrun",
    name: "Cloud Run",
    icon: "cloud",
    host: CLOUD_RUN_BASE,
    isSecure: true,
  },
  {
    id: "vm",
    name: "VM (Cloudflare)",
    icon: "server",
    host: "", // Will be filled from URL or input
    isSecure: true, // trycloudflare.com uses HTTPS
  },
];

interface DeploymentTarget {
  wsUrl: string;
  apiBase: string;
  name: string;
  id: string;
}

function buildTarget(id: string, host: string, isSecure: boolean): DeploymentTarget {
  const protocol = isSecure ? "https" : "http";
  const wsProtocol = isSecure ? "wss" : "ws";
  const portSuffix = !isSecure && !host.includes(":") ? ":8080" : "";

  return {
    id,
    name: id === "cloudrun" ? "Cloud Run" : `VM (${host.substring(0, 20)}${host.length > 20 ? '...' : ''})`,
    wsUrl: `${wsProtocol}://${host}${portSuffix}/ws`,
    apiBase: `${protocol}://${host}${portSuffix}`,
  };
}

function getDeploymentTarget(): DeploymentTarget {
  if (typeof window === "undefined") {
    return buildTarget("cloudrun", CLOUD_RUN_BASE, true);
  }

  const params = new URLSearchParams(window.location.search);
  const target = params.get("target");
  const vmHost = params.get("ip");

  if (target === "vm" && vmHost) {
    const isSecure = vmHost.includes("trycloudflare.com") || vmHost.startsWith("localhost");
    return buildTarget("vm", vmHost, isSecure);
  }

  return buildTarget("cloudrun", CLOUD_RUN_BASE, true);
}

export default function Home() {
  const [requests, setRequests] = useState<NetworkRequest[]>([]);
  const [status, setStatus] = useState<Status | null>(null);
  const [wsConnected, setWsConnected] = useState(false);
  const [deploymentTarget, setDeploymentTarget] = useState<DeploymentTarget | null>(null);
  const [showTargetMenu, setShowTargetMenu] = useState(false);
  const [vmHostInput, setVmHostInput] = useState("");
  const activeSessionIdRef = useRef<string | null>(null);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout>(null);
  const wsRef = useRef<WebSocket | null>(null);

  // Automation state
  const [automationState, setAutomationState] = useState<AutomationState>({
    mode: "idle",
    isRunning: false,
    scriptId: null,
    prompt: null,
    error: null,
    data: {},
    cursorPosition: null,
    cursorAction: "move",
  });

  // Initialize deployment target on client side
  useEffect(() => {
    const target = getDeploymentTarget();
    setDeploymentTarget(target);
    // Pre-fill VM host input from URL if present
    if (typeof window !== "undefined") {
      const params = new URLSearchParams(window.location.search);
      const ip = params.get("ip");
      if (ip) setVmHostInput(ip);
    }
  }, []);

  const switchTarget = (targetId: string, host?: string) => {
    // Close any existing WebSocket
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    setWsConnected(false);
    setRequests([]);
    setStatus(null);

    // Update URL and reload
    const url = new URL(window.location.href);
    if (targetId === "vm" && host) {
      url.searchParams.set("target", "vm");
      url.searchParams.set("ip", host);
    } else {
      url.searchParams.delete("target");
      url.searchParams.delete("ip");
    }
    window.location.href = url.toString();
  };

  const fetchStatus = async () => {
    if (!deploymentTarget) return;
    try {
      const res = await fetch(`${deploymentTarget.apiBase}/api/status`);
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
    if (!deploymentTarget) return;
    try {
      await fetch(`${deploymentTarget.apiBase}/api/network/clear`, { method: 'POST' });
    } catch (e) {
      console.error('Failed to clear network cache on server', e);
    }
  };

  useEffect(() => {
    if (!deploymentTarget) return;
    fetchStatus();

    const connectWs = () => {
      if (wsRef.current?.readyState === WebSocket.OPEN) return;

      const ws = new WebSocket(deploymentTarget.wsUrl);
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

          // Automation events
          if (msg.type === "AUTOMATION_MODE_CHANGED") {
            setAutomationState(prev => ({
              ...prev,
              mode: msg.payload.mode as AutomationMode,
              isRunning: msg.payload.mode !== "idle",
              prompt: msg.payload.prompt || null,
              scriptId: msg.payload.scriptId || prev.scriptId,
            }));
          }

          if (msg.type === "AUTOMATION_CURSOR") {
            setAutomationState(prev => ({
              ...prev,
              cursorPosition: { x: msg.payload.x, y: msg.payload.y },
              cursorAction: msg.payload.action,
            }));
          }

          if (msg.type === "AUTOMATION_DATA_UPDATED") {
            setAutomationState(prev => ({
              ...prev,
              data: msg.payload.data || prev.data,
            }));
          }

          if (msg.type === "AUTOMATION_COMPLETE") {
            setAutomationState(prev => ({
              ...prev,
              mode: "idle",
              isRunning: false,
              error: msg.payload.error || null,
              data: msg.payload.data || prev.data,
            }));
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
  }, [deploymentTarget]);

  return (
    <div className="min-h-screen bg-background p-6">
      <div className="container mx-auto max-w-[1400px] space-y-6">
        {/* Header */}
        <header className="text-center lg:text-left">
          <div className="flex items-center gap-3 justify-center lg:justify-start mb-1">
            <h1 className="text-2xl font-semibold text-foreground tracking-tight">
              Docker Chrome
            </h1>
            {/* Target Switcher Dropdown */}
            <div className="relative">
              <button
                onClick={() => setShowTargetMenu(!showTargetMenu)}
                className="flex items-center gap-1.5 text-xs px-2.5 py-1 bg-surface border border-border rounded-full text-text-secondary hover:bg-background hover:text-foreground transition-colors"
              >
                {deploymentTarget?.id === "cloudrun" ? (
                  <Cloud size={12} />
                ) : (
                  <Server size={12} />
                )}
                <span>{deploymentTarget?.name || "Loading..."}</span>
                <ChevronDown size={12} />
              </button>

              {showTargetMenu && (
                <div className="absolute top-full left-0 mt-1 w-72 bg-surface border border-border rounded-lg shadow-lg z-50">
                  {/* Cloud Run Option */}
                  <button
                    onClick={() => {
                      switchTarget("cloudrun");
                      setShowTargetMenu(false);
                    }}
                    className={`w-full flex items-center gap-2 px-3 py-2.5 text-sm text-left hover:bg-background transition-colors ${deploymentTarget?.id === "cloudrun" ? "text-accent" : "text-foreground"
                      }`}
                  >
                    <Cloud size={14} />
                    <div>
                      <div className="font-medium">Cloud Run</div>
                      <div className="text-xs text-text-secondary truncate">{CLOUD_RUN_BASE}</div>
                    </div>
                  </button>

                  <div className="border-t border-border" />

                  {/* VM Option with Input */}
                  <div className="p-3">
                    <div className="flex items-center gap-2 mb-2">
                      <Server size={14} className="text-text-secondary" />
                      <span className="text-sm font-medium text-foreground">VM Target</span>
                    </div>
                    <input
                      type="text"
                      value={vmHostInput}
                      onChange={(e) => setVmHostInput(e.target.value)}
                      placeholder="xyz.trycloudflare.com or IP:port"
                      className="w-full bg-background border border-border rounded px-2 py-1.5 text-xs text-foreground placeholder:text-text-tertiary focus:outline-none focus:border-accent"
                    />
                    <button
                      onClick={() => {
                        if (vmHostInput.trim()) {
                          switchTarget("vm", vmHostInput.trim());
                          setShowTargetMenu(false);
                        }
                      }}
                      disabled={!vmHostInput.trim()}
                      className="w-full mt-2 bg-accent hover:bg-accent-hover text-white px-3 py-1.5 rounded text-xs font-medium transition-colors disabled:opacity-50"
                    >
                      Connect to VM
                    </button>
                  </div>
                </div>
              )}
            </div>
          </div>
          <p className="text-sm text-text-secondary">
            Remote browser control with network inspection
          </p>
        </header>

        {/* Top Row: Browser + Controls */}
        <div className="grid lg:grid-cols-2 gap-6">
          {/* Left: Browser Viewer */}
          <div className="flex items-start">
            <BrowserFrame
              url={deploymentTarget?.apiBase || ''}
              automationMode={automationState.mode}
              automationPrompt={automationState.prompt || ''}
              cursorPosition={automationState.cursorPosition}
              cursorAction={automationState.cursorAction}
            />
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
            <ControlPanel
              status={status}
              onRefreshStatus={fetchStatus}
              onReset={handleReset}
              apiBase={deploymentTarget?.apiBase || ''}
              automationMode={automationState.mode}
            />

            {/* Data Panel */}
            <DataPanel data={automationState.data} />
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
