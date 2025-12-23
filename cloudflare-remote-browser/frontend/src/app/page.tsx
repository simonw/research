"use client";

import React, { useEffect, useState, useRef } from "react";
import { BrowserViewer } from "@/components/BrowserViewer";
import { NetworkPanel } from "@/components/network-panel";
import { ControlPanel } from "@/components/control-panel";
import { DataPanel } from "@/components/data-panel";
import { ChevronDown, Cloud, Server, Trash2, Play } from "lucide-react";
import { useSession } from '@/hooks/useSession';
import { NetworkRequest } from '@/lib/types';
import * as api from '@/lib/api';

const DEFAULT_SCRIPT = `// Remote Browser Automation Script
// 
// Goal:
// - Open the login page
// - Wait for you to log in (takeover)
// - Finish when the next page says "Logged In Successfully"

await page.goto('https://practicetestautomation.com/practice-test-login/');
await page.waitForSelector('#username');

await page.promptUser(
  "Please log in in the remote browser, then click I'm Done.\\n\\nTest creds (optional): student / Password123"
);

await page.waitForSelector('text=Logged In Successfully', { timeout: 5 * 60 * 1000 });

const url = await page.url();
return { success: true, url };
`;

export default function Home() {
  const [script, setScript] = useState(DEFAULT_SCRIPT);
  const session = useSession();
  const [showTargetMenu, setShowTargetMenu] = useState(false);
  const [activeSessions, setActiveSessions] = useState<api.ListedSession[]>([]);

  const loadSessions = async () => {
    try {
      const data = await session.listSessions();
      setActiveSessions(data.sessions || []);
    } catch (e) {
      console.error('Failed to list sessions', e);
    }
  };

  useEffect(() => {
    loadSessions();
  }, []);

  useEffect(() => {
    if (showTargetMenu) {
      loadSessions();
    }
  }, [showTargetMenu]);

  useEffect(() => {
    if (session.connected && session.status === 'idle' && session.sessionId) {
      const timer = setTimeout(() => {
        session.runScript(script).catch(console.error);
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [session.connected, session.status, session.sessionId]); 

  const handleKillSession = async (sessionId: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    if (!confirm("Are you sure you want to kill this session?")) {
      return;
    }
    
    if (sessionId === session.sessionId) {
      await session.destroySession();
    } else {
      await api.destroySession(sessionId);
    }
    await loadSessions();
  };

  const isInteractive = session.status === 'takeover';

  return (
    <div className="min-h-screen bg-background p-6">
      <div className="container mx-auto max-w-[1400px] space-y-6">
        <header className="text-center lg:text-left">
          <div className="flex items-center gap-3 justify-center lg:justify-start mb-1">
            <h1 className="text-2xl font-semibold text-foreground tracking-tight">
              Cloudflare Remote Browser
            </h1>
            <div className="relative">
              <button
                onClick={() => setShowTargetMenu(!showTargetMenu)}
                className="flex items-center gap-1.5 text-xs px-2.5 py-1 bg-surface border border-border rounded-full text-text-secondary hover:bg-background hover:text-foreground transition-colors"
              >
                <Cloud size={12} />
                <span>{session.sessionId ? `Session ${session.sessionId.slice(0, 8)}...` : "No Active Session"}</span>
                {session.connected && <div className="w-1.5 h-1.5 rounded-full bg-success" />}
                <ChevronDown size={12} />
              </button>

              {showTargetMenu && (
                <div className="absolute top-full left-0 mt-1 w-72 bg-surface border border-border rounded-lg shadow-lg z-50">
                  <div className="p-2">
                    <button
                      onClick={() => {
                        session.createSession();
                        setShowTargetMenu(false);
                      }}
                      className="w-full flex items-center gap-2 px-3 py-2 text-sm text-left hover:bg-background transition-colors rounded-lg text-accent"
                    >
                      <Play size={14} />
                      <div className="font-medium">Start New Session</div>
                    </button>
                  </div>

                  {activeSessions.length > 0 && (
                    <>
                      <div className="border-t border-border my-1" />
                      <div className="px-3 py-1.5 text-xs font-semibold text-text-tertiary uppercase tracking-wider">
                        Active Sessions
                      </div>
                      {activeSessions.map((s) => (
                        <div
                          key={s.sessionId}
                          className={`flex items-center gap-2 px-3 py-2.5 text-sm hover:bg-background transition-colors cursor-pointer group ${
                            session.sessionId === s.sessionId ? "bg-background" : ""
                          }`}
                          onClick={() => {
                            session.attachSession(s.sessionId);
                            setShowTargetMenu(false);
                          }}
                        >
                          <Server size={14} className={session.sessionId === s.sessionId ? "text-accent" : "text-foreground"} />
                          <div className="flex-1 truncate">
                            <div className="flex items-center gap-2">
                              <div className={`font-medium truncate ${session.sessionId === s.sessionId ? "text-accent" : "text-foreground"}`}>
                                {s.sessionId.slice(0, 8)}...
                              </div>
                              {session.sessionId === s.sessionId && session.connected && (
                                <div className="w-1.5 h-1.5 rounded-full bg-success shrink-0" />
                              )}
                            </div>
                            <div className="text-xs text-text-secondary">
                              {new Date(s.createdAt).toLocaleTimeString()}
                            </div>
                          </div>
                          <button
                            onClick={(e) => handleKillSession(s.sessionId, e)}
                            className="p-1.5 text-text-tertiary hover:text-error hover:bg-error/10 rounded transition-colors opacity-0 group-hover:opacity-100"
                            title="Kill Session"
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                      ))}
                    </>
                  )}
                </div>
              )}
            </div>
          </div>
          <p className="text-sm text-text-secondary">
            Remote browser automation with user takeover via Cloudflare Workers
          </p>
        </header>

        <div className="grid lg:grid-cols-2 gap-6">
          <div className="flex items-start justify-center lg:justify-start">
            <BrowserViewer
              onFrame={session.setOnFrame}
              sendInput={session.sendInput}
              isInteractive={isInteractive}
              viewport={session.viewport}
              takeoverMessage={session.takeoverMessage}
            />
          </div>

          <div className="space-y-4">
            <ControlPanel
              session={session}
              script={script}
              setScript={setScript}
            />

            <DataPanel data={session.automationData} />
          </div>
        </div>

        <div>
          <NetworkPanel 
            requests={session.networkRequests} 
            sessionId={session.sessionId}
            onClearRequests={session.clearNetworkRequests}
          />
        </div>
      </div>
    </div>
  );
}
