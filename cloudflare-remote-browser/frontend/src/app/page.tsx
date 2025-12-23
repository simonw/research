'use client';

import { useState, useEffect } from 'react';
import { useSession } from '@/hooks/useSession';
import { BrowserViewer } from '@/components/BrowserViewer';
import { SessionStatus } from '@/lib/types';
import * as api from '@/lib/api';
import dynamic from 'next/dynamic';

const MonacoEditor = dynamic(() => import('@monaco-editor/react').then(mod => mod.default), {
  ssr: false,
  loading: () => <div className="h-full bg-gray-800 flex items-center justify-center text-gray-400">Loading editor...</div>
});

interface SessionManagerProps {
  session: ReturnType<typeof useSession>;
}

function SessionManager({ session }: SessionManagerProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [sessions, setSessions] = useState<api.ListedSession[]>([]);
  const [loading, setLoading] = useState(false);

  const loadSessions = async () => {
    setLoading(true);
    try {
      const data = await session.listSessions();
      setSessions(data.sessions || []);
    } catch (e) {
      console.error('Failed to list sessions:', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      loadSessions();
    }
  }, [isOpen]);

  const handleKill = async (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm('Are you sure you want to kill this session?')) return;

    try {
      if (session.sessionId === sessionId) {
        await session.destroySession();
      } else {
        await api.destroySession(sessionId);
      }
      await loadSessions();
    } catch {
      alert('Failed to kill session');
    }
  };

  return (
    <div className="relative inline-block text-left mr-2">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="px-3 py-2 bg-white border border-gray-300 text-gray-700 rounded-lg hover:bg-gray-50 font-medium text-sm flex items-center gap-2"
      >
        <span>Sessions</span>
        <span className="text-xs bg-gray-100 px-1.5 py-0.5 rounded-full border border-gray-200">
          {sessions.length}
        </span>
      </button>

      {isOpen && (
        <div className="absolute right-0 mt-2 w-80 bg-white rounded-lg shadow-lg border border-gray-200 z-50 overflow-hidden">
          <div className="p-3 border-b border-gray-100 flex items-center justify-between bg-gray-50">
            <h3 className="font-semibold text-gray-700 text-sm">Active Sessions</h3>
            <button
              onClick={loadSessions}
              disabled={loading}
              className="px-2 py-1 text-xs text-gray-700 border border-gray-200 rounded hover:bg-gray-100 disabled:opacity-60"
              title="Refresh"
            >
              Refresh
            </button>
          </div>
          
          <div className="max-h-64 overflow-y-auto">
            {loading && sessions.length === 0 ? (
              <div className="p-4 text-center text-gray-500 text-sm">Loading...</div>
            ) : sessions.length === 0 ? (
              <div className="p-4 text-center text-gray-500 text-sm">No active sessions</div>
            ) : (
              <ul className="divide-y divide-gray-100">
                {sessions.map((s) => {
                  const isCurrent = session.sessionId === s.sessionId;
                  return (
                    <li key={s.sessionId} className={`p-3 hover:bg-gray-50 ${isCurrent ? 'bg-blue-50' : ''}`}>
                      <div className="flex items-center justify-between mb-2">
                        <span className="font-mono text-xs text-gray-600 truncate w-32" title={s.sessionId}>
                          {s.sessionId}
                        </span>
                        <span className="text-xs text-gray-400">
                          {new Date(s.createdAt).toLocaleTimeString()}
                        </span>
                      </div>
                      <div className="flex items-center justify-end gap-2">
                        {isCurrent ? (
                          <span className="text-xs font-medium text-blue-600 px-2 py-1 bg-blue-100 rounded">
                            Active
                          </span>
                        ) : (
                          <button
                            onClick={() => {
                              session.attachSession(s.sessionId);
                              setIsOpen(false);
                            }}
                            className="text-xs px-2 py-1 text-blue-600 hover:bg-blue-50 rounded border border-blue-200"
                          >
                            Attach
                          </button>
                        )}
                        <button
                          onClick={(e) => handleKill(s.sessionId, e)}
                          className="text-xs px-2 py-1 text-red-600 hover:bg-red-50 rounded border border-red-200"
                        >
                          Kill
                        </button>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

const DEFAULT_SCRIPT = `// Remote Browser Automation Script
//
// Goal:
// - Open the login page
// - Wait for you to log in (takeover)
// - Finish when the next page says "Logged In Successfully"
//
// Available:
//   page - Playwright Page object (proxied)
//   requestTakeover(message) - Pause for user interaction

await page.goto('https://practicetestautomation.com/practice-test-login/');
await page.waitForSelector('#username');

await requestTakeover(
  "Please log in in the remote browser, then click I'm Done.\\n\\nTest creds (optional): student / Password123"
);

await page.waitForSelector('text=Logged In Successfully', { timeout: 5 * 60 * 1000 });

const url = await page.url();
return { success: true, url };
`;

const STATUS_CONFIG: Record<SessionStatus, { label: string; color: string }> = {
  idle: { label: 'Ready', color: 'bg-gray-500' },
  starting: { label: 'Starting browser...', color: 'bg-yellow-500' },
  running: { label: 'Script running...', color: 'bg-blue-500' },
  takeover: { label: 'Waiting for you', color: 'bg-orange-500' },
  done: { label: 'Completed', color: 'bg-green-500' },
  error: { label: 'Error', color: 'bg-red-500' }
};

export default function Home() {
  const [script, setScript] = useState(DEFAULT_SCRIPT);
  const session = useSession();

  const isInteractive = session.status === 'takeover';
  const isRunning = ['starting', 'running', 'takeover'].includes(session.status);
  
  const canCreate = !session.sessionId || session.status === 'error' || session.status === 'done';
  const canRun = session.sessionId && (session.status === 'idle' || session.status === 'done');
  const canStop = session.sessionId && isRunning;

  const statusConfig = STATUS_CONFIG[session.status];

  return (
    <main className="min-h-screen p-6 bg-gray-100">
      <div className="max-w-7xl mx-auto space-y-6">
        <header className="flex items-center justify-between">
          <h1 className="text-2xl font-bold text-gray-800">
            Remote Browser Automation
          </h1>
          <div className="flex items-center gap-2">
            <SessionManager session={session} />
            {canCreate && (
              <button
                onClick={session.createSession}
                className="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 font-medium"
              >
                {session.sessionId ? 'New Session' : 'Start Session'}
              </button>
            )}
            {canRun && (
              <button
                onClick={() => session.runScript(script)}
                className="px-4 py-2 bg-green-500 text-white rounded-lg hover:bg-green-600 font-medium flex items-center gap-2"
              >
                <span>▶</span> Run Script
              </button>
            )}
            {canStop && (
              <button
                onClick={session.destroySession}
                className="px-4 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 font-medium flex items-center gap-2"
              >
                <span>⏹</span> Stop
              </button>
            )}
          </div>
        </header>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <div className="bg-white rounded-lg shadow-sm p-4">
            <h2 className="text-lg font-semibold mb-3 text-gray-700">Script</h2>
            <div className="h-[500px] border rounded-lg overflow-hidden">
              <MonacoEditor
                height="100%"
                defaultLanguage="typescript"
                value={script}
                onChange={(v) => setScript(v || '')}
                theme="vs-dark"
                options={{
                  minimap: { enabled: false },
                  fontSize: 14,
                  lineNumbers: 'on',
                  scrollBeyondLastLine: false,
                  wordWrap: 'on',
                  readOnly: isRunning,
                  automaticLayout: true
                }}
              />
            </div>
          </div>

          <div className="bg-white rounded-lg shadow-sm p-4">
            <h2 className="text-lg font-semibold mb-3 text-gray-700">Browser</h2>
            <BrowserViewer
              onFrame={session.setOnFrame}
              sendInput={session.sendInput}
              isInteractive={isInteractive}
            />
          </div>
        </div>

        <div className="bg-white rounded-lg shadow-sm p-4 space-y-3">
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <div className={`w-2 h-2 rounded-full ${session.connected ? 'bg-green-500' : 'bg-red-500'}`} />
              <span className="text-sm text-gray-600">
                {session.connected ? 'Connected' : 'Disconnected'}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <div className={`w-2 h-2 rounded-full ${statusConfig.color}`} />
              <span className="text-sm font-medium">{statusConfig.label}</span>
            </div>
            {session.sessionId && (
              <span className="text-xs text-gray-400">Session: {session.sessionId.slice(0, 8)}...</span>
            )}
          </div>

          {session.status === 'takeover' && session.takeoverMessage && (
            <div className="flex items-center gap-4 p-3 bg-orange-50 border border-orange-200 rounded-lg">
              <span className="flex-1 text-orange-800">{session.takeoverMessage}</span>
              <button
                onClick={session.completeTakeover}
                className="px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 font-medium"
              >
                I&apos;m Done
              </button>
            </div>
          )}

          {session.error && (
            <div className="p-3 bg-red-50 border border-red-200 rounded-lg">
              <p className="text-red-800 font-medium">Error</p>
              <p className="text-red-600 text-sm mt-1">{session.error}</p>
            </div>
          )}

          {session.status === 'done' && session.result !== null && (
            <div className="p-3 bg-green-50 border border-green-200 rounded-lg">
              <p className="text-green-800 font-medium">Result</p>
              <pre className="text-green-600 text-sm mt-1 overflow-auto max-h-32">
                {JSON.stringify(session.result, null, 2)}
              </pre>
            </div>
          )}
        </div>
      </div>
    </main>
  );
}
