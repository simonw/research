"use client";

import React, { useState } from "react";
import { Globe, Play, Square, Loader2 } from "lucide-react";
import { useSession } from "@/hooks/useSession";

interface ControlPanelProps {
  session: ReturnType<typeof useSession>;
  script: string;
  setScript: (script: string) => void;
}

export function ControlPanel({
  session,
  script,
  setScript
}: ControlPanelProps) {
  const [url, setUrl] = useState("https://google.com");
  const [loading, setLoading] = useState(false);

  const isConnected = !!session.sessionId && session.connected;
  const isRunning = ['starting', 'running', 'takeover'].includes(session.status);

  const handleNavigate = async () => {
    if (!isConnected) return;
    setLoading(true);
    try {
      await session.runScript(`await page.goto('${url}');`);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className={`bg-surface border border-border rounded-lg p-4 transition-opacity ${!isConnected ? 'opacity-50 pointer-events-none' : ''}`}>
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
              if (e.key === "Enter" && !loading) {
                handleNavigate();
              }
            }}
            className="flex-1 bg-background border border-border rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-accent transition-colors text-foreground placeholder:text-text-tertiary"
            placeholder="https://example.com"
            disabled={!isConnected}
          />
          <button
            onClick={handleNavigate}
            disabled={loading || !isConnected}
            className="bg-accent hover:bg-accent-hover text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
          >
            Go
          </button>
        </div>
      </div>

      <div className="bg-surface border border-border rounded-lg p-4">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-sm font-semibold text-foreground flex items-center gap-2">
            <Play size={16} className="text-text-secondary" />
            <span>Playwright Script</span>
          </h2>
          {isRunning && (
            <span className="flex items-center gap-1.5 text-xs text-accent">
              <Loader2 size={12} className="animate-spin" />
              {session.status === "takeover" ? "Waiting for user" : "Running"}
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

        <details className="mt-3">
          <summary className="text-xs text-text-tertiary cursor-pointer hover:text-text-secondary">
            API Reference
          </summary>
          <div className="mt-2 text-xs text-text-secondary bg-background rounded p-2 font-mono space-y-1">
            <div>page.goto(url)</div>
            <div>page.click(selector)</div>
            <div>page.type(selector, text)</div>
            <div>page.fill(selector, text)</div>
            <div>page.waitForSelector(selector)</div>
            <div>page.sleep(ms)</div>
            <div>page.evaluate(fn)</div>
            <div>page.scrapeText(selector)</div>
            <div>page.scrapeAll(selector)</div>
            <div>page.promptUser(msg, options?)</div>
            <div>page.captureNetwork({`{urlPattern, bodyPattern, key}`})</div>
            <div>page.waitForNetworkCapture(key, timeout?)</div>
            <div>page.getCapturedResponse(key)</div>
            <div>page.setData(key, value)</div>
            <div>page.getData()</div>
          </div>
        </details>
      </div>
    </div>
  );
}
