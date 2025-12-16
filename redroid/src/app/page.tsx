'use client';

import { useState, useEffect, useCallback } from 'react';

interface GCPSession {
  vmName: string;
  status: 'starting' | 'running';
  ip: string | null;
  zone: string;
  createdAt: number;
}

export default function Home() {
  const [activeSessions, setActiveSessions] = useState<GCPSession[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(true);
  const [killingVm, setKillingVm] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Fetch all active sessions from GCP
  const fetchActiveSessions = useCallback(async () => {
    try {
      const response = await fetch('/api/sessions');
      if (!response.ok) return;

      const data = await response.json();
      setActiveSessions(data.sessions || []);
    } catch (err) {
      console.error('Error fetching active sessions:', err);
    } finally {
      setIsRefreshing(false);
    }
  }, []);

  // Fetch on mount and poll every 10 seconds
  useEffect(() => {
    fetchActiveSessions();
    const interval = setInterval(fetchActiveSessions, 10000);
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

      // Refresh the sessions list
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

      // Remove from local state immediately
      setActiveSessions(prev => prev.filter(s => s.vmName !== vmName));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred');
    } finally {
      setKillingVm(null);
    }
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-gray-800 to-gray-900">
      {/* Background decoration */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-blue-500/10 rounded-full blur-3xl" />
        <div className="absolute bottom-1/4 right-1/4 w-96 h-96 bg-purple-500/10 rounded-full blur-3xl" />
      </div>

      <div className="relative z-10 container mx-auto px-4 py-12 max-w-2xl">
        {/* Header */}
        <div className="text-center mb-12">
          <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-gradient-to-br from-green-500 to-emerald-600 shadow-lg shadow-green-900/30 mb-6">
            <svg className="w-10 h-10 text-white" fill="currentColor" viewBox="0 0 24 24">
              <path d="M17.6 9.48l1.84-3.18c.16-.31.04-.69-.26-.85a.637.637 0 0 0-.83.22l-1.88 3.24a11.463 11.463 0 0 0-8.94 0L5.65 5.67a.643.643 0 0 0-.87-.2c-.28.18-.37.54-.22.83L6.4 9.48A10.78 10.78 0 0 0 1 18h22a10.78 10.78 0 0 0-5.4-8.52zM7 15.25a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5zm10 0a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5z" />
            </svg>
          </div>
          <h1 className="text-4xl font-bold text-white mb-3">
            ReDroid Cloud
          </h1>
          <p className="text-gray-400 text-lg">
            Ephemeral Android emulators in the cloud
          </p>
        </div>

        {/* Error Alert */}
        {error && (
          <div className="mb-6 p-4 bg-red-900/30 border border-red-700 rounded-xl text-red-300">
            <div className="flex items-center gap-2">
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              {error}
            </div>
          </div>
        )}

        {/* Start New Session Button */}
        <button
          onClick={handleStart}
          disabled={isLoading}
          className={`
            w-full py-4 px-6 rounded-xl font-semibold text-lg transition-all duration-200
            ${!isLoading
              ? 'bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-500 hover:to-purple-500 text-white shadow-lg shadow-blue-900/30 hover:shadow-xl hover:shadow-blue-900/40 hover:-translate-y-0.5'
              : 'bg-gray-700 text-gray-400 cursor-not-allowed'
            }
          `}
        >
          {isLoading ? (
            <span className="flex items-center justify-center gap-2">
              <svg className="animate-spin h-5 w-5" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
              </svg>
              Starting...
            </span>
          ) : (
            <span className="flex items-center justify-center gap-2">
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
              Start New Session
            </span>
          )}
        </button>

        {/* Active Sessions Section */}
        <div className="mt-12">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-xl font-semibold text-white">Active Sessions</h2>
            <button
              onClick={fetchActiveSessions}
              className="text-sm text-gray-400 hover:text-white transition-colors"
            >
              Refresh
            </button>
          </div>

          {isRefreshing ? (
            <div className="bg-gray-800/50 rounded-xl p-8 border border-gray-700 text-center">
              <div className="animate-spin h-8 w-8 mx-auto mb-4 border-2 border-gray-500 border-t-white rounded-full" />
              <p className="text-gray-400">Loading sessions from GCP...</p>
            </div>
          ) : activeSessions.length === 0 ? (
            <div className="bg-gray-800/50 rounded-xl p-8 border border-gray-700 text-center">
              <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-700/50 flex items-center justify-center">
                <svg className="w-8 h-8 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z" />
                </svg>
              </div>
              <p className="text-gray-400">No active sessions</p>
              <p className="text-sm text-gray-500 mt-1">Click &quot;Start New Session&quot; to launch an Android emulator</p>
            </div>
          ) : (
            <div className="space-y-4">
              {activeSessions.map((session) => (
                <div
                  key={session.vmName}
                  className="bg-gray-800/50 rounded-xl p-4 border border-gray-700"
                >
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center gap-3">
                      {session.status === 'running' ? (
                        <div className="h-3 w-3 rounded-full bg-green-400 animate-pulse" />
                      ) : (
                        <div className="animate-spin rounded-full h-3 w-3 border-2 border-yellow-400 border-t-transparent" />
                      )}
                      <span className="font-mono text-white">{session.vmName}</span>
                    </div>
                    <span className={`text-sm capitalize ${session.status === 'running' ? 'text-green-400' : 'text-yellow-400'}`}>
                      {session.status}
                    </span>
                  </div>

                  <div className="grid grid-cols-2 gap-2 text-sm mb-4">
                    <div>
                      <span className="text-gray-500">IP Address</span>
                      <p className="font-mono text-gray-300">{session.ip || 'Pending...'}</p>
                    </div>
                    <div>
                      <span className="text-gray-500">Zone</span>
                      <p className="text-gray-300">{session.zone}</p>
                    </div>
                  </div>

                  <div className="flex gap-2">
                    {session.status === 'running' && session.ip && (
                      <a
                        href={`https://${session.ip}/vnc.html`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="flex-1 py-2 px-4 bg-gradient-to-r from-green-600 to-emerald-600 hover:from-green-500 hover:to-emerald-500 text-white rounded-lg font-medium text-sm text-center transition-all"
                      >
                        Open in Browser
                      </a>
                    )}
                    <button
                      onClick={() => handleKill(session.vmName)}
                      disabled={killingVm === session.vmName}
                      className={`
                        flex-1 py-2 px-4 rounded-lg font-medium text-sm transition-all
                        ${killingVm === session.vmName
                          ? 'bg-gray-700 text-gray-400 cursor-not-allowed'
                          : 'bg-gradient-to-r from-red-600 to-orange-600 hover:from-red-500 hover:to-orange-500 text-white'
                        }
                      `}
                    >
                      {killingVm === session.vmName ? (
                        <span className="flex items-center justify-center gap-2">
                          <svg className="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
                            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                          </svg>
                          Killing...
                        </span>
                      ) : (
                        'Kill Session'
                      )}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Info Footer */}
        <div className="mt-12 text-center text-gray-500 text-sm">
          <p>Sessions auto-terminate after 60 minutes</p>
          <p className="mt-1">~$0.02 per 15-minute session</p>
        </div>
      </div>
    </div>
  );
}
