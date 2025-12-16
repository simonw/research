'use client';

import { useState, useEffect, useCallback } from 'react';
import { PhoneFrame } from '@/components/phone-frame';
import { VMProgress } from '@/lib/types';

interface GCPSession {
  vmName: string;
  status: 'starting' | 'running';
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

  // Get the currently selected session
  const selectedSession = activeSessions.find(s => s.vmName === selectedVm) || activeSessions[0] || null;

  // Fetch all active sessions from GCP
  const fetchActiveSessions = useCallback(async () => {
    try {
      const response = await fetch('/api/sessions');
      if (!response.ok) return;

      const data = await response.json();
      const sessions: GCPSession[] = data.sessions || [];

      // Fetch detailed progress for each session
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

      // Auto-select first session if none selected
      if (!selectedVm && sessionsWithProgress.length > 0) {
        setSelectedVm(sessionsWithProgress[0].vmName);
      }
    } catch (err) {
      console.error('Error fetching active sessions:', err);
    } finally {
      setIsRefreshing(false);
    }
  }, [selectedVm]);

  // Fetch on mount and poll every 3 seconds for progress updates
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

      // Clear selection if killed session was selected
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
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-gray-800 to-gray-900">
      {/* Background decoration */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-blue-500/10 rounded-full blur-3xl" />
        <div className="absolute bottom-1/4 right-1/4 w-96 h-96 bg-purple-500/10 rounded-full blur-3xl" />
      </div>

      <div className="relative z-10 container mx-auto px-4 py-8 max-w-4xl">
        {/* Header */}
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-gradient-to-br from-green-500 to-emerald-600 shadow-lg shadow-green-900/30 mb-4">
            <svg className="w-8 h-8 text-white" fill="currentColor" viewBox="0 0 24 24">
              <path d="M17.6 9.48l1.84-3.18c.16-.31.04-.69-.26-.85a.637.637 0 0 0-.83.22l-1.88 3.24a11.463 11.463 0 0 0-8.94 0L5.65 5.67a.643.643 0 0 0-.87-.2c-.28.18-.37.54-.22.83L6.4 9.48A10.78 10.78 0 0 0 1 18h22a10.78 10.78 0 0 0-5.4-8.52zM7 15.25a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5zm10 0a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5z" />
            </svg>
          </div>
          <h1 className="text-3xl font-bold text-white mb-2">ReDroid Cloud</h1>
          <p className="text-gray-400">Ephemeral Android emulators in the cloud</p>
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

        {/* Main Phone Frame - Shows selected session */}
        <div className="flex justify-center mb-8">
          {selectedSession ? (
            <PhoneFrame
              status={selectedSession.status === 'running' ? 'ready' : 'loading'}
              progress={selectedSession.progress}
              streamUrl={selectedSession.streamUrl}
              vmName={selectedSession.vmName}
            />
          ) : (
            <div className="flex flex-col items-center">
              <div
                className="relative bg-gray-900 rounded-[3rem] p-2 shadow-2xl shadow-black/50 flex items-center justify-center"
                style={{ width: 'min(100%, 320px)', aspectRatio: '9/19.5' }}
              >
                <div className="absolute inset-2 bg-gray-800 rounded-[2.5rem]" />
                <div className="relative w-full h-full bg-black rounded-[2.25rem] flex flex-col items-center justify-center p-6">
                  <svg className="w-16 h-16 text-gray-700 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z" />
                  </svg>
                  <p className="text-gray-500 text-sm text-center">No active session</p>
                  <p className="text-gray-600 text-xs text-center mt-1">Start a session to view the emulator</p>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Start New Session Button */}
        <button
          onClick={handleStart}
          disabled={isLoading}
          className={`
            w-full py-3 px-6 rounded-xl font-semibold text-lg transition-all duration-200 mb-6
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

        {/* Sessions List */}
        <div>
          <h2 className="text-lg font-semibold text-white mb-3">Active Sessions</h2>

          {isRefreshing ? (
            <div className="bg-gray-800/50 rounded-xl p-6 border border-gray-700 text-center">
              <div className="animate-spin h-6 w-6 mx-auto mb-3 border-2 border-gray-500 border-t-white rounded-full" />
              <p className="text-gray-400 text-sm">Loading sessions...</p>
            </div>
          ) : activeSessions.length === 0 ? (
            <div className="bg-gray-800/50 rounded-xl p-6 border border-gray-700 text-center">
              <p className="text-gray-400 text-sm">No active sessions</p>
            </div>
          ) : (
            <div className="space-y-2">
              {activeSessions.map((session) => (
                <div
                  key={session.vmName}
                  className={`bg-gray-800/50 rounded-xl p-4 border transition-all ${selectedVm === session.vmName
                      ? 'border-blue-500 bg-blue-900/20'
                      : 'border-gray-700 hover:border-gray-600'
                    }`}
                >
                  <div className="flex items-center justify-between gap-4">
                    {/* Session info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        {session.status === 'running' ? (
                          <div className="h-2 w-2 rounded-full bg-green-400" />
                        ) : (
                          <div className="animate-spin rounded-full h-2 w-2 border border-yellow-400 border-t-transparent" />
                        )}
                        <span className="font-mono text-white text-sm truncate">{session.vmName}</span>
                      </div>
                      <p className="text-xs text-gray-400 truncate">
                        {session.progress?.message || (session.status === 'running' ? 'Ready' : 'Starting...')}
                      </p>
                    </div>

                    {/* Action buttons */}
                    <div className="flex gap-2 flex-shrink-0">
                      <button
                        onClick={() => setSelectedVm(session.vmName)}
                        className={`py-1.5 px-3 rounded-lg font-medium text-sm transition-all ${selectedVm === session.vmName
                            ? 'bg-blue-600 text-white'
                            : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
                          }`}
                      >
                        View
                      </button>
                      <button
                        onClick={() => handleKill(session.vmName)}
                        disabled={killingVm === session.vmName}
                        className={`py-1.5 px-3 rounded-lg font-medium text-sm transition-all ${killingVm === session.vmName
                            ? 'bg-gray-700 text-gray-400 cursor-not-allowed'
                            : 'bg-red-600/80 text-white hover:bg-red-600'
                          }`}
                      >
                        {killingVm === session.vmName ? 'Killing...' : 'Kill'}
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Info Footer */}
        <div className="mt-8 text-center text-gray-500 text-xs">
          <p>Sessions auto-terminate after 60 minutes • ~$0.02 per 15-minute session</p>
        </div>
      </div>
    </div>
  );
}
