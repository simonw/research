"use client";

import React, { useState } from "react";
import { Status } from "@/lib/types";
import { RefreshCw, RotateCcw } from "lucide-react";

const API_BASE = "https://docker-chrome-432753364585.us-central1.run.app";

interface ControlPanelProps {
  status: Status | null;
  onRefreshStatus: () => void;
  onReset?: () => void;
}

export function ControlPanel({ status, onRefreshStatus, onReset }: ControlPanelProps) {
  const [loading, setLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);

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

  const handleRefresh = async () => {
    setIsRefreshing(true);
    await onRefreshStatus();
    setTimeout(() => setIsRefreshing(false), 600);
  };

  return (
    <div className="space-y-4">
      {/* System Status Card */}
      <div className="bg-surface border border-border rounded-lg p-5 shadow-sm relative overflow-hidden group">
        {/* Subtle texture overlay */}
        <div className="absolute inset-0 opacity-[0.03] pointer-events-none"
             style={{
               backgroundImage: `url("data:image/svg+xml,%3Csvg width='60' height='60' viewBox='0 0 60 60' xmlns='http://www.w3.org/2000/svg'%3E%3Cg fill='none' fill-rule='evenodd'%3E%3Cg fill='%23000000' fill-opacity='1'%3E%3Cpath d='M36 34v-4h-2v4h-4v2h4v4h2v-4h4v-2h-4zm0-30V0h-2v4h-4v2h4v4h2V6h4V4h-4zM6 34v-4H4v4H0v2h4v4h2v-4h4v-2H6zM6 4V0H4v4H0v2h4v4h2V6h4V4H6z'/%3E%3C/g%3E%3C/g%3E%3C/svg%3E")`
             }}
        />

        <div className="flex items-center justify-between mb-5 relative">
          <h2 className="text-base font-semibold text-foreground tracking-tight">
            System Status
          </h2>
          <button
            onClick={handleRefresh}
            disabled={isRefreshing}
            className="p-2 hover:bg-background rounded-lg transition-all duration-250 group/btn disabled:opacity-50"
            title="Refresh Status"
          >
            <RefreshCw
              size={16}
              className={`text-text-secondary group-hover/btn:text-foreground transition-colors ${isRefreshing ? 'animate-spin' : ''}`}
            />
          </button>
        </div>

        <div className="space-y-4 relative">
          {/* CDP Connection */}
          <div className="flex items-center justify-between">
            <span className="text-sm text-text-secondary font-medium">CDP Connection</span>
            <div className="flex items-center gap-2.5">
              <div className="relative">
                <div className={`w-2 h-2 rounded-full transition-all duration-300 ${
                  status?.cdpConnected ? 'bg-success' : 'bg-error'
                }`} />
                {status?.cdpConnected && (
                  <div className="absolute inset-0 w-2 h-2 rounded-full bg-success animate-ping opacity-75" />
                )}
              </div>
              <span className={`text-sm font-medium transition-colors duration-300 ${
                status?.cdpConnected ? 'text-success' : 'text-error'
              }`}>
                {status?.cdpConnected ? 'Connected' : 'Disconnected'}
              </span>
            </div>
          </div>

          {/* CDP Port */}
          <div className="flex items-center justify-between pt-1">
            <span className="text-sm text-text-secondary font-medium">CDP Port</span>
            <span className="text-sm font-mono font-semibold text-foreground bg-background px-3 py-1 rounded border border-border">
              {status?.cdpPort || '—'}
            </span>
          </div>
        </div>
      </div>

      {/* Reset Session Button */}
      <button
        onClick={handleResetSession}
        disabled={loading}
        className="w-full group relative overflow-hidden bg-surface hover:bg-error border border-border hover:border-error rounded-lg px-5 py-3.5 transition-all duration-250 disabled:opacity-50 disabled:cursor-not-allowed shadow-sm hover:shadow"
      >
        <div className="flex items-center justify-center gap-2.5 relative z-10">
          <RotateCcw
            size={16}
            className={`text-error group-hover:text-white transition-colors duration-250 ${loading ? 'animate-spin' : ''}`}
          />
          <span className="text-sm font-semibold text-error group-hover:text-white transition-colors duration-250">
            {loading ? 'Resetting Session...' : 'Reset Session'}
          </span>
        </div>

        {/* Hover background fill effect */}
        <div className="absolute inset-0 bg-error transform scale-x-0 group-hover:scale-x-100 transition-transform duration-250 origin-left" />
      </button>

      {/* Info Text */}
      <p className="text-xs text-text-tertiary leading-relaxed px-1">
        The reset session action will restart the Chrome instance with a fresh profile and clear all configurations.
      </p>
    </div>
  );
}
