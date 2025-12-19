"use client";

import React, { useState } from "react";
import { Status } from "@/lib/types";
import { Globe, Play } from "lucide-react";

const API_BASE = "https://docker-chrome-432753364585.us-central1.run.app";

interface ControlPanelProps {
  status: Status | null;
  onRefreshStatus: () => void;
  onReset?: () => void;
}

export function ControlPanel({ status, onRefreshStatus, onReset }: ControlPanelProps) {
  const [url, setUrl] = useState("https://google.com");
  const [script, setScript] = useState("");
  const [loading, setLoading] = useState(false);

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

  return (
    <div className="space-y-4">
      {/* Navigation Section */}
      <div className="bg-surface border border-border rounded-lg p-4">
        <h2 className="text-sm font-semibold text-foreground flex items-center gap-2 mb-3">
          <Globe size={16} className="text-text-secondary" />
          <span>Navigation</span>
        </h2>
        <div className="flex gap-2">
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            className="flex-1 bg-background border border-border rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-accent transition-colors text-foreground placeholder:text-text-tertiary"
            placeholder="https://example.com"
          />
          <button
            onClick={handleNavigate}
            disabled={loading}
            className="bg-accent hover:bg-accent-hover text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
          >
            Go
          </button>
        </div>
      </div>

      {/* Quick Inject Section */}
      <div className="bg-surface border border-border rounded-lg p-4">
        <h2 className="text-sm font-semibold text-foreground flex items-center gap-2 mb-3">
          <Play size={16} className="text-text-secondary" />
          <span>Quick Inject</span>
        </h2>
        <textarea
          value={script}
          onChange={(e) => setScript(e.target.value)}
          className="w-full h-24 bg-background border border-border rounded-lg p-3 text-sm font-mono text-foreground focus:outline-none focus:border-accent transition-colors placeholder:text-text-tertiary resize-none"
          placeholder="document.body.style.background = 'red';"
        />
        <button
          onClick={handleInject}
          disabled={loading || !script}
          className="w-full mt-3 bg-surface hover:bg-background text-foreground border border-border px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
        >
          Execute Script
        </button>
      </div>
    </div>
  );
}
