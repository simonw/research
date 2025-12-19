'use client';

import { useState, useEffect, useRef, useCallback } from 'react';
import { BrowserFrame } from '@/components/browser-frame';
import { ControlPanel } from '@/components/control-panel';
import { NetworkPanel } from '@/components/network-panel';
import { CDPStatus, NetworkRequest, VMProgress } from '@/lib/types';

// Default URLs for local development
const DEFAULT_NEKO_URL = 'http://localhost:8080';
const DEFAULT_CDP_AGENT_URL = 'http://localhost:3001';

interface GCPSession {
    vmName: string;
    status: string;
    ip: string | null;
    createdAt: number;
}

interface SessionWithProgress extends GCPSession {
    progress: VMProgress | null;
    streamUrl: string | null;
    cdpAgentUrl: string | null;
}

export default function Home() {
    // Mode: 'local' or 'gcp'
    const [mode, setMode] = useState<'local' | 'gcp'>('local');

    // Local mode URLs
    const [localNekoUrl, setLocalNekoUrl] = useState(DEFAULT_NEKO_URL);
    const [localCdpUrl, setLocalCdpUrl] = useState(DEFAULT_CDP_AGENT_URL);

    // GCP session management
    const [sessions, setSessions] = useState<SessionWithProgress[]>([]);
    const [selectedVm, setSelectedVm] = useState<string | null>(null);
    const [isStarting, setIsStarting] = useState(false);
    const [killingVm, setKillingVm] = useState<string | null>(null);

    // Active URLs (depend on mode and selected session)
    const selectedSession = sessions.find(s => s.vmName === selectedVm);
    const nekoUrl = mode === 'local' ? localNekoUrl : (selectedSession?.streamUrl || '');
    const cdpAgentUrl = mode === 'local' ? localCdpUrl : (selectedSession?.cdpAgentUrl || '');

    // CDP state
    const [status, setStatus] = useState<CDPStatus | null>(null);
    const [requests, setRequests] = useState<NetworkRequest[]>([]);
    const wsRef = useRef<WebSocket | null>(null);
    const [wsConnected, setWsConnected] = useState(false);

    // Fetch GCP sessions
    const fetchSessions = useCallback(async () => {
        if (mode !== 'gcp') return;

        try {
            const res = await fetch('/api/sessions');
            if (!res.ok) return;

            const data = await res.json();
            const gcpSessions: GCPSession[] = data.sessions || [];

            // Fetch detailed progress for each session
            const sessionsWithProgress = await Promise.all(
                gcpSessions.map(async (session): Promise<SessionWithProgress> => {
                    try {
                        const statusRes = await fetch(`/api/session/${session.vmName}`);
                        if (statusRes.ok) {
                            const statusData = await statusRes.json();
                            return {
                                ...session,
                                status: statusData.session?.status || session.status,
                                progress: statusData.progress || null,
                                streamUrl: statusData.url || null,
                                cdpAgentUrl: statusData.cdpAgentUrl || null,
                            };
                        }
                    } catch (e) {
                        console.error('Error fetching session status:', e);
                    }
                    return { ...session, progress: null, streamUrl: null, cdpAgentUrl: null };
                })
            );

            setSessions(sessionsWithProgress);

            // Auto-select first session if none selected
            if (!selectedVm && sessionsWithProgress.length > 0) {
                setSelectedVm(sessionsWithProgress[0].vmName);
            }
        } catch (err) {
            console.error('Error fetching sessions:', err);
        }
    }, [mode, selectedVm]);

    // Fetch CDP status
    const fetchStatus = useCallback(async () => {
        if (!cdpAgentUrl) return;

        try {
            const res = await fetch(`${cdpAgentUrl}/api/status`);
            if (res.ok) {
                const data = await res.json();
                setStatus(data);
            }
        } catch (e) {
            console.error('Failed to fetch CDP status:', e);
            setStatus(null);
        }
    }, [cdpAgentUrl]);

    // Setup WebSocket for network events
    const connectWebSocket = useCallback(() => {
        if (!cdpAgentUrl) return;
        if (wsRef.current?.readyState === WebSocket.OPEN) return;

        // Close existing connection
        if (wsRef.current) {
            wsRef.current.close();
            wsRef.current = null;
        }

        const wsUrl = `${cdpAgentUrl.replace(/^http/, 'ws')}/ws`;
        console.log('Connecting WebSocket to:', wsUrl);

        try {
            const ws = new WebSocket(wsUrl);
            wsRef.current = ws;

            ws.onopen = () => {
                console.log('WebSocket connected');
                setWsConnected(true);
            };

            ws.onmessage = (event) => {
                try {
                    const msg = JSON.parse(event.data);
                    if (msg.type === 'NETWORK_REQUEST') {
                        setRequests((prev) => [...prev.slice(-499), { ...msg.payload }]);
                    } else if (msg.type === 'NETWORK_RESPONSE') {
                        setRequests((prev) =>
                            prev.map((r) =>
                                r.requestId === msg.payload.requestId
                                    ? { ...r, status: msg.payload.status, mimeType: msg.payload.mimeType }
                                    : r
                            )
                        );
                    } else if (msg.type === 'VIEWPORT_CHANGED') {
                        setStatus((prev) => prev ? { ...prev, viewport: msg.payload } : null);
                    }
                } catch (e) {
                    console.error('Failed to parse WebSocket message:', e);
                }
            };

            ws.onclose = () => {
                console.log('WebSocket disconnected');
                setWsConnected(false);
            };

            ws.onerror = (err) => {
                console.error('WebSocket error:', err);
                ws.close();
            };
        } catch (e) {
            console.error('Failed to create WebSocket:', e);
        }
    }, [cdpAgentUrl]);

    // Handle starting a new GCP session
    const handleStartSession = async () => {
        setIsStarting(true);
        try {
            const res = await fetch('/api/session/start', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({}),
            });

            if (!res.ok) {
                const data = await res.json();
                throw new Error(data.error || 'Failed to start session');
            }

            const data = await res.json();
            setSelectedVm(data.vmName);

            // Refresh sessions
            await fetchSessions();
        } catch (err) {
            console.error('Failed to start session:', err);
            alert(err instanceof Error ? err.message : 'Failed to start session');
        } finally {
            setIsStarting(false);
        }
    };

    // Handle killing a session
    const handleKillSession = async (vmName: string) => {
        setKillingVm(vmName);
        try {
            const res = await fetch('/api/session/end', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ vmName }),
            });

            if (!res.ok) {
                const data = await res.json();
                throw new Error(data.error || 'Failed to kill session');
            }

            setSessions(prev => prev.filter(s => s.vmName !== vmName));
            if (selectedVm === vmName) {
                setSelectedVm(null);
            }
        } catch (err) {
            console.error('Failed to kill session:', err);
        } finally {
            setKillingVm(null);
        }
    };

    // Effects
    useEffect(() => {
        if (mode === 'gcp') {
            fetchSessions();
            const interval = setInterval(fetchSessions, 3000);
            return () => clearInterval(interval);
        }
    }, [mode, fetchSessions]);

    useEffect(() => {
        if (cdpAgentUrl) {
            fetchStatus();
            connectWebSocket();
            const interval = setInterval(fetchStatus, 5000);
            return () => {
                clearInterval(interval);
                if (wsRef.current) {
                    wsRef.current.close();
                }
            };
        }
    }, [cdpAgentUrl, fetchStatus, connectWebSocket]);

    const handleClearRequests = () => setRequests([]);

    return (
        <div className="min-h-screen bg-[var(--background)] p-4">
            <div className="max-w-[1920px] mx-auto">
                {/* Header */}
                <div className="flex items-center justify-between mb-4">
                    <div>
                        <h1 className="text-xl font-semibold">Docker Chrome Neko</h1>
                        <p className="text-sm text-zinc-500">Remote browser control via WebRTC + CDP</p>
                    </div>
                    <div className="flex items-center gap-3">
                        {/* Mode Toggle */}
                        <div className="flex bg-zinc-800 rounded-lg p-0.5">
                            <button
                                onClick={() => setMode('local')}
                                className={`px-3 py-1 text-xs rounded-md transition-colors ${mode === 'local' ? 'bg-zinc-700 text-white' : 'text-zinc-400'}`}
                            >
                                Local
                            </button>
                            <button
                                onClick={() => setMode('gcp')}
                                className={`px-3 py-1 text-xs rounded-md transition-colors ${mode === 'gcp' ? 'bg-zinc-700 text-white' : 'text-zinc-400'}`}
                            >
                                GCP
                            </button>
                        </div>

                        <div className="flex items-center gap-2 text-sm">
                            <span className={`w-2 h-2 rounded-full ${wsConnected ? 'bg-green-500' : 'bg-red-500'}`} />
                            <span className="text-zinc-500">WS</span>
                        </div>
                        <button
                            onClick={handleClearRequests}
                            className="text-xs text-zinc-500 hover:text-zinc-300 px-2 py-1 rounded bg-zinc-800"
                        >
                            Clear
                        </button>
                    </div>
                </div>

                {/* Mode-specific controls */}
                {mode === 'local' ? (
                    <div className="flex gap-4 mb-4 text-sm">
                        <div className="flex items-center gap-2">
                            <label className="text-zinc-500">Neko:</label>
                            <input
                                type="text"
                                value={localNekoUrl}
                                onChange={(e) => setLocalNekoUrl(e.target.value)}
                                className="bg-zinc-900 border border-[var(--border)] rounded px-2 py-1 w-64 text-xs font-mono"
                            />
                        </div>
                        <div className="flex items-center gap-2">
                            <label className="text-zinc-500">CDP Agent:</label>
                            <input
                                type="text"
                                value={localCdpUrl}
                                onChange={(e) => setLocalCdpUrl(e.target.value)}
                                className="bg-zinc-900 border border-[var(--border)] rounded px-2 py-1 w-64 text-xs font-mono"
                            />
                        </div>
                    </div>
                ) : (
                    <div className="mb-4 space-y-3">
                        <div className="flex items-center gap-3">
                            <button
                                onClick={handleStartSession}
                                disabled={isStarting}
                                className="bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
                            >
                                {isStarting ? 'Starting...' : 'Start New Session'}
                            </button>
                            <span className="text-xs text-zinc-500">{sessions.length} active sessions</span>
                        </div>

                        {sessions.length > 0 && (
                            <div className="flex flex-wrap gap-2">
                                {sessions.map((session) => (
                                    <div
                                        key={session.vmName}
                                        onClick={() => setSelectedVm(session.vmName)}
                                        className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs cursor-pointer transition-colors ${selectedVm === session.vmName
                                                ? 'bg-blue-600 text-white'
                                                : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700'
                                            }`}
                                    >
                                        <span className={`w-2 h-2 rounded-full ${session.progress?.stage === 'ready' ? 'bg-green-500' : 'bg-yellow-500 animate-pulse'
                                            }`} />
                                        <span className="font-mono">{session.vmName.slice(-8)}</span>
                                        {session.progress && (
                                            <span className="text-zinc-400">{session.progress.percent}%</span>
                                        )}
                                        <button
                                            onClick={(e) => { e.stopPropagation(); handleKillSession(session.vmName); }}
                                            disabled={killingVm === session.vmName}
                                            className="ml-1 text-red-400 hover:text-red-300"
                                        >
                                            ✕
                                        </button>
                                    </div>
                                ))}
                            </div>
                        )}

                        {selectedSession?.progress && selectedSession.progress.stage !== 'ready' && (
                            <div className="bg-zinc-800/50 rounded-lg p-3 text-sm">
                                <div className="flex items-center gap-2 mb-1">
                                    <div className="animate-spin h-4 w-4 border-2 border-zinc-600 border-t-blue-500 rounded-full" />
                                    <span>{selectedSession.progress.message}</span>
                                </div>
                                <div className="w-full bg-zinc-700 rounded-full h-1.5">
                                    <div
                                        className="bg-blue-500 h-1.5 rounded-full transition-all"
                                        style={{ width: `${selectedSession.progress.percent}%` }}
                                    />
                                </div>
                            </div>
                        )}
                    </div>
                )}

                {/* Main Layout */}
                <div className="grid grid-cols-1 lg:grid-cols-[1fr_360px] gap-4">
                    <div className="space-y-4">
                        <div className="h-[600px]">
                            <BrowserFrame
                                nekoUrl={nekoUrl}
                                cdpAgentUrl={cdpAgentUrl}
                                onViewportChange={(w, h) => {
                                    setStatus((prev) => (prev ? { ...prev, viewport: { width: w, height: h } } : null));
                                }}
                            />
                        </div>
                        <NetworkPanel requests={requests} cdpAgentUrl={cdpAgentUrl} />
                    </div>

                    <div className="h-[600px]">
                        <ControlPanel
                            cdpAgentUrl={cdpAgentUrl}
                            status={status}
                            onRefreshStatus={fetchStatus}
                        />
                    </div>
                </div>
            </div>
        </div>
    );
}
