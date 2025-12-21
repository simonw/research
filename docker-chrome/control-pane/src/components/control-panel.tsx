"use client";

import React, { useState } from "react";
import { Status } from "@/lib/types";
import { Globe, Play, Square, Loader2 } from "lucide-react";

interface ControlPanelProps {
  status: Status | null;
  onRefreshStatus: () => void;
  apiBase: string;
  automationMode?: "idle" | "automation" | "user-input";
  onAutomationStart?: (scriptId: string) => void;
}

const SAMPLE_SCRIPT = `// Example automation script
await playwright.goto('https://example.com');
await playwright.scrapeText('h1', 'pageTitle');

// Wait for user - auto-continue after 5 seconds
const start = Date.now();
await playwright.promptUser('Please review the page', async () => {
  return Date.now() - start > 5000;
});

await playwright.scrapeText('p', 'description');
return playwright.data;`;

export function ControlPanel({
  status,
  onRefreshStatus,
  apiBase,
  automationMode = "idle",
  onAutomationStart,
}: ControlPanelProps) {
  const [url, setUrl] = useState("https://google.com");
  const [script, setScript] = useState(SAMPLE_SCRIPT);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isRunning = automationMode !== "idle";

  const handleNavigate = async () => {
    setLoading(true);
    setError(null);
    try {
      await fetch(`${apiBase}/api/navigate`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url }),
      });
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const handleStartAutomation = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(`${apiBase}/api/automation/start`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ script }),
      });
      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error || "Failed to start automation");
      }
      onAutomationStart?.(data.scriptId);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const handleStopAutomation = async () => {
    setLoading(true);
    try {
      await fetch(`${apiBase}/api/automation/stop`, { method: "POST" });
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Error Display */}
      {error && (
        <div className="bg-error/10 border border-error/30 rounded-lg px-3 py-2 text-sm text-error">
          {error}
        </div>
      )}

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
            onKeyDown={(e) => {
              if (e.key === "Enter" && !loading && !isRunning) {
                handleNavigate();
              }
            }}
            className="flex-1 bg-background border border-border rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-accent transition-colors text-foreground placeholder:text-text-tertiary"
            placeholder="https://example.com"
            disabled={isRunning}
          />
          <button
            onClick={handleNavigate}
            disabled={loading || isRunning}
            className="bg-accent hover:bg-accent-hover text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
          >
            Go
          </button>
        </div>
      </div>

      {/* Playwright Script Section */}
      <div className="bg-surface border border-border rounded-lg p-4">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-sm font-semibold text-foreground flex items-center gap-2">
            <Play size={16} className="text-text-secondary" />
            <span>Playwright Script</span>
          </h2>
          {isRunning && (
            <span className="flex items-center gap-1.5 text-xs text-accent">
              <Loader2 size={12} className="animate-spin" />
              {automationMode === "user-input" ? "Waiting for user" : "Running"}
            </span>
          )}
        </div>

        <textarea
          value={script}
          onChange={(e) => setScript(e.target.value)}
          className="w-full h-40 bg-background border border-border rounded-lg p-3 text-sm font-mono text-foreground focus:outline-none focus:border-accent transition-colors placeholder:text-text-tertiary resize-none"
          placeholder="// Enter your Playwright automation script..."
          disabled={isRunning}
        />

        <div className="flex gap-2 mt-3">
          <button
            onClick={handleStartAutomation}
            disabled={loading || !script.trim() || isRunning}
            className="flex-1 bg-accent hover:bg-accent-hover text-white px-4 py-2.5 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
          >
            <Play size={14} />
            Run Automation
          </button>
          <button
            onClick={handleStopAutomation}
            disabled={loading}
            className="bg-error hover:bg-error/80 text-white px-4 py-2.5 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
          >
            <Square size={14} />
            Stop
          </button>
        </div>

        {/* API Reference Hint */}
        <details className="mt-3">
          <summary className="text-xs text-text-tertiary cursor-pointer hover:text-text-secondary">
            API Reference
          </summary>
          <div className="mt-2 text-xs text-text-secondary bg-background rounded p-2 font-mono space-y-1">
            <div>playwright.goto(url)</div>
            <div>playwright.click(selector)</div>
            <div>playwright.type(selector, text)</div>
            <div>playwright.fill(selector, text)</div>
            <div>playwright.waitFor(selector)</div>
            <div>playwright.sleep(ms)</div>
            <div>playwright.evaluate(code)</div>
            <div>playwright.scrapeText(selector, key)</div>
            <div>playwright.scrapeAll(selector, key)</div>
            <div>playwright.promptUser(msg, conditionFn)</div>
            <div>playwright.captureNetwork({`{urlPattern, bodyPattern, key}`})</div>
            <div>playwright.waitForNetworkCapture(key, {`{timeout}`})</div>
            <div>playwright.getCapturedResponse(key)</div>
            <div>playwright.data - collected data object</div>
            <div>playwright.setData(key, value)</div>
          </div>
        </details>
      </div>
    </div>
  );
}
