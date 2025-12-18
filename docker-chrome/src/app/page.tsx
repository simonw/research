'use client';

import { useState, useEffect, useCallback } from 'react';
import { BrowserFrame } from '@/components/browser-frame';
import { VMProgress } from '@/lib/types';

interface GCPSession {
  vmName: string;
  status: 'running' | 'staging' | 'provisioning' | 'stopping' | 'terminated' | 'stopped' | 'suspended' | 'suspending' | string;
  ip: string | null;
  zone: string;
  createdAt: number;
}

interface SessionWithProgress extends GCPSession {
  progress: VMProgress | null;
  streamUrl: string | null;
}

export default function Home() {
  const [activeSessions, setActiveSessions] = useState<SessionWithProgress[]>([]);
  const [selectedVm, setSelectedVm] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(true);
  const [killingVm, setKillingVm] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const selectedSession = activeSessions.find(s => s.vmName === selectedVm) || activeSessions[0] || null;

  const getBrowserStatus = (session: SessionWithProgress | null): 'idle' | 'loading' | 'ready' | 'stopping' => {
    if (!session) return 'idle';
    if (killingVm === session.vmName) return 'stopping';
    if (session.status === 'running') return 'ready';
    if (session.status === 'stopping' || session.status === 'terminated' || session.status === 'stopped' || session.status === 'suspended') return 'stopping';
    return 'loading';
  };

  const getStatusText = (session: SessionWithProgress): string => {
    if (killingVm === session.vmName) return 'Shutting down...';
    if (session.progress?.message) return session.progress.message;
    switch (session.status) {
      case 'running': return 'Ready';
      case 'stopping': return 'Stopping...';
      case 'terminated': return 'Terminated';
      case 'stopped': return 'Stopped';
      case 'suspended': return 'Suspended';
      case 'suspending': return 'Suspending...';
      case 'staging':
      case 'provisioning':
      default: return 'Starting...';
    }
  };

  const fetchActiveSessions = useCallback(async () => {
    try {
      const response = await fetch('/api/sessions');
      if (!response.ok) return;

      const data = await response.json();
      const sessions: GCPSession[] = data.sessions || [];

      const sessionsWithProgress = await Promise.all(
        sessions.map(async (session): Promise<SessionWithProgress> => {
          try {
            const statusRes = await fetch(`/api/session/${session.vmName}`);
            if (statusRes.ok) {
              const statusData = await statusRes.json();
              return {
                ...session,
                status: statusData.session?.status || session.status,
                progress: statusData.progress || null,
                streamUrl: statusData.url || null,
              };
            }
          } catch (e) {
            console.error('Error fetching session status:', e);
          }
          return {
            ...session,
            progress: null,
            streamUrl: null,
          };
        })
      );

      setActiveSessions(sessionsWithProgress);

      if (!selectedVm && sessionsWithProgress.length > 0) {
        setSelectedVm(sessionsWithProgress[0].vmName);
      }
    } catch (err) {
      console.error('Error fetching active sessions:', err);
    } finally {
      setIsRefreshing(false);
    }
  }, [selectedVm]);

  useEffect(() => {
    fetchActiveSessions();
    const interval = setInterval(fetchActiveSessions, 3000);
    return () => clearInterval(interval);
  }, [fetchActiveSessions]);

  const handleStart = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await fetch('/api/session/start', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ userId: 'user-1' }),
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || 'Failed to start session');
      }
      await fetchActiveSessions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred');
    } finally {
      setIsLoading(false);
    }
  };

  const handleKill = async (vmName: string) => {
    setKillingVm(vmName);
    setError(null);
    try {
      const response = await fetch('/api/session/end', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ vmName }),
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error || 'Failed to kill session');
      }

      setActiveSessions(prev => prev.filter(s => s.vmName !== vmName));
      if (selectedVm === vmName) {
        setSelectedVm(null);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred');
    } finally {
      setKillingVm(null);
    }
  };

  return (
    <div className="min-h-screen bg-[#111111] overflow-x-hidden text-white">
      <div className="container mx-auto px-4 py-8 max-w-[1600px]">
        <div className="flex justify-between items-end mb-8 border-b border-gray-800 pb-6">
          <div>
            <h1 className="text-3xl font-bold text-white tracking-tight mb-2">
              Cloud Browser
            </h1>
            <p className="text-gray-400">Secure, ephemeral Chrome sessions with network capture</p>
          </div>
          <button
            onClick={handleStart}
            disabled={isLoading}
            className={`
              py-2.5 px-6 rounded-lg text-sm font-medium transition-all duration-200
              ${!isLoading
                ? 'bg-blue-600 hover:bg-blue-500 text-white shadow-lg shadow-blue-900/20'
                : 'bg-gray-800 text-gray-500 cursor-not-allowed'
              }
            `}
          >
            {isLoading ? 'Starting...' : '+ New Session'}
          </button>
        </div>

        <div className="grid lg:grid-cols-[1fr_320px] gap-8">
          
          <div className="min-w-0">
            {error && (
              <div className="mb-6 p-4 bg-red-900/20 border border-red-500/50 rounded-lg flex items-center gap-3 text-red-400">
                <svg className="w-5 h-5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                {error}
              </div>
            )}
            
            <div className="flex justify-center">
              <BrowserFrame
                status={getBrowserStatus(selectedSession)}
                progress={selectedSession?.progress || null}
                streamUrl={selectedSession?.streamUrl || null}
                vmName={selectedSession?.vmName || ''}
              />
            </div>

            {selectedSession?.status === 'running' && (
              <div className="mt-6 bg-gray-900 rounded-xl border border-gray-800 p-6 h-[300px] flex items-center justify-center text-gray-500 border-dashed">
                Network Capture Panel Coming Soon...
              </div>
            )}
          </div>

          <div className="space-y-6">
            <div className="bg-gray-900 rounded-xl border border-gray-800 p-4">
              <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 px-2">
                Active Sessions
              </h2>
              
              {isRefreshing ? (
                <div className="flex justify-center py-8">
                  <div className="animate-spin h-6 w-6 border-2 border-gray-800 border-t-blue-500 rounded-full" />
                </div>
              ) : activeSessions.length === 0 ? (
                <div className="text-center py-8 text-gray-600 text-sm">
                  No active sessions
                </div>
              ) : (
                <div className="space-y-2">
                  {activeSessions.map((session) => (
                    <div
                      key={session.vmName}
                      onClick={() => setSelectedVm(session.vmName)}
                      className={`
                        group p-3 rounded-lg border cursor-pointer transition-all duration-200
                        ${selectedVm === session.vmName
                          ? 'bg-gray-800 border-blue-500/50 ring-1 ring-blue-500/20'
                          : 'bg-gray-900/50 border-gray-800 hover:border-gray-700 hover:bg-gray-800'
                        }
                      `}
                    >
                      <div className="flex items-center justify-between mb-2">
                        <div className="flex items-center gap-2">
                          <div className={`w-2 h-2 rounded-full ${
                            session.status === 'running' ? 'bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.4)]' :
                            session.status === 'stopping' ? 'bg-red-500' :
                            'bg-yellow-500 animate-pulse'
                          }`} />
                          <span className="font-mono text-xs text-gray-300 font-medium truncate">
                            {session.vmName}
                          </span>
                        </div>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleKill(session.vmName);
                          }}
                          className="opacity-0 group-hover:opacity-100 p-1.5 text-gray-500 hover:text-red-400 hover:bg-red-900/20 rounded transition-all"
                          title="Terminate Session"
                        >
                          <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                          </svg>
                        </button>
                      </div>
                      
                      <div className="flex justify-between items-end">
                        <span className="text-xs text-gray-500">
                          {getStatusText(session)}
                        </span>
                        {session.status === 'running' && (
                          <span className="text-[10px] font-mono text-gray-600">
                            {session.ip}
                          </span>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
            
            <div className="bg-gray-900 rounded-xl border border-gray-800 p-4">
              <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 px-2">
                Quick Actions
              </h2>
              <div className="grid grid-cols-2 gap-2">
                 <button disabled className="p-3 bg-gray-800/50 rounded-lg text-xs text-gray-500 border border-gray-800 cursor-not-allowed text-left">
                    Run Script...
                 </button>
                 <button disabled className="p-3 bg-gray-800/50 rounded-lg text-xs text-gray-500 border border-gray-800 cursor-not-allowed text-left">
                    Inject JS...
                 </button>
              </div>
            </div>

          </div>
        </div>
      </div>
    </div>
  );
}
