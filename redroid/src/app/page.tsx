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
    <div className="min-h-screen bg-[#FAF9F7] overflow-x-hidden">
      <div className="container mx-auto px-4 py-8 sm:px-6 sm:py-12 lg:px-8 lg:py-16 max-w-[1280px]">
        {/* Two-column layout on desktop, single column on mobile/tablet */}
        <div className="lg:grid lg:grid-cols-[minmax(0,55%)_minmax(0,45%)] lg:gap-6 xl:gap-10">
          {/* Left Column: Phone Display */}
          <div className="hidden lg:block lg:sticky lg:top-8 lg:self-start">
            <div className="flex justify-center py-4">
              <PhoneFrame
                status={selectedSession ? (selectedSession.status === 'running' ? 'ready' : 'loading') : 'loading'}
                progress={selectedSession?.progress || null}
                streamUrl={selectedSession?.streamUrl || null}
                vmName={selectedSession?.vmName || 'No session'}
              />
            </div>
          </div>

          {/* Right Column: Header, Controls, Sessions */}
          <div className="space-y-6 lg:space-y-8 min-w-0">
            {/* Header */}
            <div className="text-center lg:text-left">
              <h1 className="text-2xl font-semibold text-[#292827] tracking-tight mb-1 break-words">
                ReDroid Cloud
              </h1>
              <p className="text-sm text-[#6B6763] break-words">Ephemeral Android emulators in the cloud</p>
            </div>

            {/* Mobile Phone Display */}
            <div className="lg:hidden flex justify-center">
              <PhoneFrame
                status={selectedSession ? (selectedSession.status === 'running' ? 'ready' : 'loading') : 'loading'}
                progress={selectedSession?.progress || null}
                streamUrl={selectedSession?.streamUrl || null}
                vmName={selectedSession?.vmName || 'No session'}
              />
            </div>

            {/* Error Alert */}
            {error && (
              <div className="p-4 bg-white border border-[#C65D4F] rounded-lg">
                <div className="flex items-center gap-2 text-[#C65D4F] text-sm">
                  <svg className="w-5 h-5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <span>{error}</span>
                </div>
              </div>
            )}

            {/* Start New Session Button */}
            <button
              onClick={handleStart}
              disabled={isLoading}
              className={`
                w-full py-3 px-6 rounded-lg text-sm font-medium transition-all duration-250 overflow-hidden
                ${!isLoading
                  ? 'bg-[#C07855] hover:bg-[#A86745] text-white shadow-sm'
                  : 'bg-[#E7E5E0] text-[#948F89] cursor-not-allowed'
                }
              `}
            >
              {isLoading ? (
                <span className="flex items-center justify-center gap-2">
                  <svg className="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                  </svg>
                  Starting...
                </span>
              ) : (
                <span className="flex items-center justify-center gap-2">
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                  </svg>
                  Start New Session
                </span>
              )}
            </button>

            {/* Sessions List */}
            <div className="min-w-0">
              <h2 className="text-lg font-semibold text-[#292827] mb-4 break-words">Active Sessions</h2>

              {isRefreshing ? (
                <div className="bg-white rounded-lg p-6 border border-[#E7E5E0] text-center">
                  <div className="animate-spin h-6 w-6 mx-auto mb-3 border-2 border-[#E7E5E0] border-t-[#C07855] rounded-full" />
                  <p className="text-[#6B6763] text-sm">Loading sessions...</p>
                </div>
              ) : activeSessions.length === 0 ? (
                <div className="bg-white rounded-lg p-6 border border-[#E7E5E0] text-center">
                  <p className="text-[#6B6763] text-sm">No active sessions</p>
                </div>
              ) : (
                <div className="space-y-3">
                  {activeSessions.map((session) => (
                    <div
                      key={session.vmName}
                      className={`bg-white rounded-lg p-4 border transition-all duration-250 ${
                        selectedVm === session.vmName
                          ? 'border-[#C07855] shadow-sm'
                          : 'border-[#E7E5E0] hover:border-[#D4D0C8]'
                      }`}
                    >
                      <div className="flex items-center justify-between gap-4">
                        {/* Session info */}
                        <div className="flex-1 min-w-0 overflow-hidden">
                          <div className="flex items-center gap-2 mb-1">
                            {session.status === 'running' ? (
                              <div className="h-2 w-2 rounded-full bg-[#6B8456] flex-shrink-0" />
                            ) : (
                              <div className="animate-spin rounded-full h-2 w-2 border border-[#C07855] border-t-transparent flex-shrink-0" />
                            )}
                            <span className="font-mono text-[#292827] text-xs font-medium truncate">
                              {session.vmName}
                            </span>
                          </div>
                          <p className="text-xs text-[#6B6763] truncate">
                            {session.progress?.message || (session.status === 'running' ? 'Ready' : 'Starting...')}
                          </p>
                        </div>

                        {/* Action buttons */}
                        <div className="flex gap-2 flex-shrink-0">
                          <button
                            onClick={() => setSelectedVm(session.vmName)}
                            className={`py-1.5 px-3 rounded-lg text-xs font-medium transition-all duration-250 ${
                              selectedVm === session.vmName
                                ? 'bg-[#C07855] text-white'
                                : 'bg-white border border-[#E7E5E0] text-[#292827] hover:border-[#D4D0C8]'
                            }`}
                          >
                            View
                          </button>
                          <button
                            onClick={() => handleKill(session.vmName)}
                            disabled={killingVm === session.vmName}
                            className={`py-1.5 px-3 rounded-lg text-xs font-medium transition-all duration-250 ${
                              killingVm === session.vmName
                                ? 'bg-white border border-[#E7E5E0] text-[#948F89] cursor-not-allowed'
                                : 'bg-white border border-[#E7E5E0] text-[#C65D4F] hover:bg-[#C65D4F] hover:text-white hover:border-[#C65D4F]'
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
            <div className="text-center lg:text-left text-[#948F89] text-xs pt-4">
              <p className="leading-relaxed break-words">Sessions auto-terminate after 60 minutes • ~$0.02 per 15-minute session</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
