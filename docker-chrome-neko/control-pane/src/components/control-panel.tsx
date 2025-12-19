'use client';

import React, { useState, useEffect } from 'react';
import { CDPStatus, PersistentScript } from '@/lib/types';

interface ControlPanelProps {
    cdpAgentUrl: string;
    status: CDPStatus | null;
    onRefreshStatus: () => void;
}

export function ControlPanel({ cdpAgentUrl, status, onRefreshStatus }: ControlPanelProps) {
    const [url, setUrl] = useState('https://google.com');
    const [script, setScript] = useState('');
    const [persistScript, setPersistScript] = useState('');
    const [loading, setLoading] = useState(false);
    const [persistentScripts, setPersistentScripts] = useState<PersistentScript[]>([]);
    const [lastResult, setLastResult] = useState<string | null>(null);

    useEffect(() => {
        fetchPersistentScripts();
    }, [cdpAgentUrl]);

    const fetchPersistentScripts = async () => {
        try {
            const res = await fetch(`${cdpAgentUrl}/api/inject/persist`);
            if (res.ok) {
                const data = await res.json();
                setPersistentScripts(data.scripts || []);
            }
        } catch (e) {
            console.error('Failed to fetch persistent scripts', e);
        }
    };

    const handleNavigate = async () => {
        if (!url) return;
        setLoading(true);
        setLastResult(null);
        try {
            const res = await fetch(`${cdpAgentUrl}/api/navigate`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url }),
            });
            const data = await res.json();
            if (res.ok) {
                setLastResult(`Navigated to ${url}`);
            } else {
                setLastResult(`Error: ${data.error}`);
            }
        } catch (e) {
            setLastResult(`Error: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    const handleInject = async () => {
        if (!script) return;
        setLoading(true);
        setLastResult(null);
        try {
            const res = await fetch(`${cdpAgentUrl}/api/inject`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ code: script }),
            });
            const data = await res.json();
            if (res.ok) {
                setLastResult(`Result: ${JSON.stringify(data.result, null, 2)}`);
            } else {
                setLastResult(`Error: ${data.error}`);
            }
        } catch (e) {
            setLastResult(`Error: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    const handleAddPersist = async () => {
        if (!persistScript) return;
        setLoading(true);
        try {
            const res = await fetch(`${cdpAgentUrl}/api/inject/persist`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ code: persistScript }),
            });
            if (res.ok) {
                setPersistScript('');
                fetchPersistentScripts();
                onRefreshStatus();
                setLastResult('Persistent script added');
            }
        } catch (e) {
            setLastResult(`Error: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    const handleClearPersist = async () => {
        if (!confirm('Clear all persistent scripts?')) return;
        setLoading(true);
        try {
            await fetch(`${cdpAgentUrl}/api/inject/persist`, { method: 'DELETE' });
            fetchPersistentScripts();
            onRefreshStatus();
            setLastResult('Persistent scripts cleared');
        } catch (e) {
            setLastResult(`Error: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    const handleResetSession = async () => {
        if (!confirm('Reset session? This will clear persistent scripts and reconnect.')) return;
        setLoading(true);
        try {
            await fetch(`${cdpAgentUrl}/api/session/reset`, { method: 'POST' });
            onRefreshStatus();
            setLastResult('Session reset');
        } catch (e) {
            setLastResult(`Error: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="flex flex-col gap-4 p-4 bg-[var(--surface)] border border-[var(--border)] rounded-lg h-full overflow-y-auto">
            {/* Status */}
            <div className="bg-zinc-900/50 p-3 rounded-lg border border-[var(--border)]">
                <div className="flex items-center justify-between mb-2">
                    <h2 className="text-xs font-medium text-zinc-400 uppercase tracking-wider">Status</h2>
                    <div className="flex gap-1">
                        <button
                            onClick={handleResetSession}
                            disabled={loading}
                            title="Reset Session"
                            className="p-1 hover:bg-red-900/50 text-red-500 rounded transition-colors"
                        >
                            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                            </svg>
                        </button>
                        <button onClick={onRefreshStatus} className="p-1 hover:bg-zinc-800 rounded transition-colors">
                            <svg className="w-3.5 h-3.5 text-zinc-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                            </svg>
                        </button>
                    </div>
                </div>
                <div className="flex items-center gap-2">
                    <span className={`w-2 h-2 rounded-full ${status?.cdpConnected ? 'bg-green-500' : 'bg-red-500'}`} />
                    <span className="text-sm font-mono">
                        {status?.cdpConnected ? 'Connected' : 'Disconnected'}
                    </span>
                    {status?.viewport && (
                        <span className="text-xs text-zinc-500 ml-auto">
                            {status.viewport.width}x{status.viewport.height}
                        </span>
                    )}
                </div>
            </div>

            {/* Navigation */}
            <div className="space-y-2">
                <h2 className="text-sm font-medium flex items-center gap-2">
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9" />
                    </svg>
                    Navigation
                </h2>
                <div className="flex gap-2">
                    <input
                        type="text"
                        value={url}
                        onChange={(e) => setUrl(e.target.value)}
                        onKeyDown={(e) => e.key === 'Enter' && handleNavigate()}
                        className="flex-1 bg-zinc-900 border border-[var(--border)] rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-[var(--accent)] transition-colors"
                        placeholder="https://example.com"
                    />
                    <button
                        onClick={handleNavigate}
                        disabled={loading || !url}
                        className="bg-[var(--accent)] hover:bg-blue-600 text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
                    >
                        Go
                    </button>
                </div>
            </div>

            {/* Quick Inject */}
            <div className="space-y-2">
                <h2 className="text-sm font-medium flex items-center gap-2">
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    Quick Inject
                </h2>
                <textarea
                    value={script}
                    onChange={(e) => setScript(e.target.value)}
                    className="w-full h-20 bg-zinc-900 border border-[var(--border)] rounded-lg p-2 text-sm font-mono focus:outline-none focus:border-[var(--accent)] transition-colors resize-none"
                    placeholder="document.body.style.background = 'red';"
                />
                <button
                    onClick={handleInject}
                    disabled={loading || !script}
                    className="w-full bg-zinc-800 hover:bg-zinc-700 border border-[var(--border)] px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
                >
                    Execute
                </button>
            </div>

            {/* Persistent Scripts */}
            <div className="space-y-2 flex-1">
                <div className="flex items-center justify-between">
                    <h2 className="text-sm font-medium flex items-center gap-2">
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
                        </svg>
                        Persistent Scripts
                    </h2>
                    {persistentScripts.length > 0 && (
                        <button
                            onClick={handleClearPersist}
                            className="text-xs text-red-400 hover:text-red-300"
                        >
                            Clear All
                        </button>
                    )}
                </div>
                <textarea
                    value={persistScript}
                    onChange={(e) => setPersistScript(e.target.value)}
                    className="w-full h-20 bg-zinc-900 border border-[var(--border)] rounded-lg p-2 text-sm font-mono focus:outline-none focus:border-[var(--accent)] transition-colors resize-none"
                    placeholder="// This script runs on every navigation"
                />
                <button
                    onClick={handleAddPersist}
                    disabled={loading || !persistScript}
                    className="w-full bg-zinc-800 hover:bg-zinc-700 border border-[var(--border)] px-4 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
                >
                    Add Permanent
                </button>

                {persistentScripts.length > 0 && (
                    <div className="mt-2 space-y-1 max-h-24 overflow-y-auto">
                        <span className="text-xs text-zinc-500">Active ({persistentScripts.length})</span>
                        {persistentScripts.map((s, i) => (
                            <div
                                key={i}
                                className="bg-zinc-900/50 border border-[var(--border)]/50 rounded p-2 text-xs font-mono text-zinc-400 truncate"
                            >
                                {s.code}
                            </div>
                        ))}
                    </div>
                )}
            </div>

            {/* Last Result */}
            {lastResult && (
                <div className="bg-zinc-900/50 border border-[var(--border)] rounded-lg p-2 text-xs font-mono text-zinc-400 max-h-24 overflow-auto">
                    {lastResult}
                </div>
            )}
        </div>
    );
}
