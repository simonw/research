"use client";

import React, { useState, useEffect } from "react";
import { Status, PersistentScript } from "@/lib/types";
import { Play, Save, Globe, Trash2, RefreshCw, RotateCcw } from "lucide-react";

const API_BASE = "https://docker-chrome-432753364585.us-central1.run.app";

interface ControlPanelProps {
  status: Status | null;
  onRefreshStatus: () => void;
  onReset?: () => void;
}

export function ControlPanel({ status, onRefreshStatus, onReset }: ControlPanelProps) {
  const [url, setUrl] = useState("https://google.com");
  const [script, setScript] = useState("");
  const [persistScript, setPersistScript] = useState("");
  const [loading, setLoading] = useState(false);
  const [persistentScripts, setPersistentScripts] = useState<PersistentScript[]>([]);

  useEffect(() => {
    fetchPersistentScripts();
  }, []);

  const fetchPersistentScripts = async () => {
    try {
      const res = await fetch(`${API_BASE}/api/inject/persist`);
      if (res.ok) {
        const data = await res.json();
        setPersistentScripts(data.scripts || []);
      }
    } catch (e) {
      console.error("Failed to fetch persistent scripts", e);
    }
  };

  const handleNavigate = async () => {
    setLoading(true);
    try {
      await fetch(`${API_BASE}/api/navigate`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url }),
      });
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const handleInject = async () => {
    setLoading(true);
    try {
      await fetch(`${API_BASE}/api/inject`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code: script }),
      });
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const handleAddPersist = async () => {
    if (!persistScript) return;
    setLoading(true);
    try {
      await fetch(`${API_BASE}/api/inject/persist`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code: persistScript }),
      });
      setPersistScript("");
      fetchPersistentScripts();
      onRefreshStatus();
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const handleClearPersist = async () => {
    if (!confirm("Clear all persistent scripts?")) return;
    setLoading(true);
    try {
      await fetch(`${API_BASE}/api/inject/persist`, {
        method: "DELETE",
      });
      fetchPersistentScripts();
      onRefreshStatus();
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const handleResetSession = async () => {
    if (!confirm("WARNING: This will restart Chrome with a new user-data-dir and clear all persistent scripts. Are you sure?")) return;
    setLoading(true);
    try {
      await fetch(`${API_BASE}/api/session/reset`, { method: "POST" });
      if (onReset) onReset();
      onRefreshStatus();
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-6 p-6 bg-surface border border-border rounded-xl h-full overflow-y-auto">
      <div className="bg-zinc-900/50 p-4 rounded-lg border border-border">
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-sm font-medium text-zinc-400 uppercase tracking-wider">System Status</h2>
          <div className="flex gap-1">
            <button 
              onClick={handleResetSession}
              disabled={loading}
              title="Reset Session"
              className="p-1 hover:bg-red-900/50 text-red-500 rounded transition-colors"
            >
              <RotateCcw size={14} />
            </button>
            <button onClick={onRefreshStatus} className="p-1 hover:bg-zinc-800 rounded transition-colors">
              <RefreshCw size={14} className="text-zinc-500" />
            </button>
          </div>
        </div>
        
        <div className="grid grid-cols-2 gap-4">
          <div className="flex flex-col">
            <span className="text-xs text-zinc-500">CDP Connection</span>
            <div className="flex items-center gap-2 mt-1">
              <span className={`w-2 h-2 rounded-full ${status?.cdpConnected ? 'bg-green-500' : 'bg-red-500'}`} />
              <span className="text-sm font-mono text-foreground">
                {status?.cdpConnected ? 'Connected' : 'Disconnected'}
              </span>
            </div>
          </div>
          <div className="flex flex-col">
            <span className="text-xs text-zinc-500">CDP Port</span>
            <span className="text-sm font-mono text-foreground">{status?.cdpPort || '-'}</span>
          </div>
        </div>
      </div>

      <div className="space-y-3">
        <h2 className="text-lg font-semibold text-foreground flex items-center gap-2">
          <Globe size={18} /> Navigation
        </h2>
        <div className="flex gap-2">
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            className="flex-1 bg-zinc-900 border border-border rounded-lg px-4 py-2 text-sm focus:outline-none focus:border-accent transition-colors text-foreground placeholder:text-zinc-600"
            placeholder="https://example.com"
          />
          <button
            onClick={handleNavigate}
            disabled={loading}
            className="bg-accent hover:bg-blue-600 text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Go
          </button>
        </div>
      </div>

      <div className="space-y-3">
        <h2 className="text-lg font-semibold text-foreground flex items-center gap-2">
          <Play size={18} /> Quick Inject
        </h2>
        <textarea
          value={script}
          onChange={(e) => setScript(e.target.value)}
          className="w-full h-24 bg-zinc-900 border border-border rounded-lg p-3 text-sm font-mono text-foreground focus:outline-none focus:border-accent transition-colors placeholder:text-zinc-600 resize-none"
          placeholder="document.body.style.background = 'red';"
        />
        <button
          onClick={handleInject}
          disabled={loading || !script}
          className="w-full bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-border px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
        >
          Execute Script
        </button>
      </div>

      <div className="space-y-3 flex-1">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-foreground flex items-center gap-2">
            <Save size={18} /> Persistent Scripts
          </h2>
          {persistentScripts.length > 0 && (
            <button 
              onClick={handleClearPersist}
              className="text-xs text-red-400 hover:text-red-300 flex items-center gap-1"
            >
              <Trash2 size={12} /> Clear All
            </button>
          )}
        </div>
        
        <div className="flex flex-col gap-2">
          <textarea
            value={persistScript}
            onChange={(e) => setPersistScript(e.target.value)}
            className="w-full h-24 bg-zinc-900 border border-border rounded-lg p-3 text-sm font-mono text-foreground focus:outline-none focus:border-accent transition-colors placeholder:text-zinc-600 resize-none"
            placeholder="// This script runs on every navigation"
          />
          <button
            onClick={handleAddPersist}
            disabled={loading || !persistScript}
            className="w-full bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-border px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
          >
            Add Permanent Script
          </button>
        </div>

        {persistentScripts.length > 0 && (
          <div className="mt-4 space-y-2">
            <span className="text-xs font-medium text-zinc-500 uppercase tracking-wider">Active Scripts ({persistentScripts.length})</span>
            <div className="flex flex-col gap-2 max-h-[200px] overflow-y-auto">
              {persistentScripts.map((s, i) => (
                <div key={i} className="bg-zinc-900/50 border border-border/50 rounded p-2 text-xs font-mono text-zinc-400 truncate">
                  {s.code}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
