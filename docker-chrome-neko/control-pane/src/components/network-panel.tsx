'use client';

import React, { useRef, useEffect, useState, useMemo } from 'react';
import { NetworkRequest } from '@/lib/types';

// Filter groups for network request types
export const FILTER_GROUPS: Record<string, string[]> = {
    XHR: ['XHR', 'Fetch'],
    Doc: ['Document'],
    JS: ['Script'],
    CSS: ['Stylesheet'],
    Img: ['Image'],
    Media: ['Media', 'Font'],
    WS: ['WebSocket'],
    Other: ['Other', 'Manifest', 'Preflight'],
};

type FilterGroup = keyof typeof FILTER_GROUPS;

interface NetworkPanelProps {
    requests: NetworkRequest[];
    cdpAgentUrl: string;
}

export function NetworkPanel({ requests, cdpAgentUrl }: NetworkPanelProps) {
    const scrollRef = useRef<HTMLDivElement>(null);
    const [selectedFilters, setSelectedFilters] = useState<Set<FilterGroup>>(new Set());
    const [selectedRequest, setSelectedRequest] = useState<NetworkRequest | null>(null);
    const [responseBody, setResponseBody] = useState<string | null>(null);
    const [loadingBody, setLoadingBody] = useState(false);

    const toggleFilter = (group: FilterGroup) => {
        const newFilters = new Set(selectedFilters);
        if (newFilters.has(group)) {
            newFilters.delete(group);
        } else {
            newFilters.add(group);
        }
        setSelectedFilters(newFilters);
    };

    const handleRequestClick = async (req: NetworkRequest) => {
        if (selectedRequest?.requestId === req.requestId) {
            setSelectedRequest(null);
            setResponseBody(null);
            return;
        }

        setSelectedRequest(req);
        setResponseBody(null);
        setLoadingBody(true);

        try {
            const res = await fetch(`${cdpAgentUrl}/api/network/${req.requestId}/body`);
            if (res.ok) {
                const data = await res.json();
                let body = data.body || '';
                if (data.base64Encoded) {
                    try {
                        body = atob(data.body);
                    } catch {
                        body = '(binary content)';
                    }
                }
                // Try to format JSON
                try {
                    const parsed = JSON.parse(body);
                    body = JSON.stringify(parsed, null, 2);
                } catch { }
                setResponseBody(body);
            } else {
                setResponseBody('(Response body not available)');
            }
        } catch {
            setResponseBody('(Failed to fetch response body)');
        } finally {
            setLoadingBody(false);
        }
    };

    const filteredRequests = useMemo(() => {
        if (selectedFilters.size === 0) return requests;

        const allowedTypes = new Set<string>();
        selectedFilters.forEach((group) => {
            FILTER_GROUPS[group].forEach((type) => allowedTypes.add(type));
        });

        return requests.filter((req) => req.type && allowedTypes.has(req.type));
    }, [requests, selectedFilters]);

    // Auto-scroll to bottom
    useEffect(() => {
        if (scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
        }
    }, [filteredRequests]);

    return (
        <div className="flex flex-col h-full bg-[var(--surface)] border border-[var(--border)] rounded-lg overflow-hidden">
            {/* Header */}
            <div className="px-4 py-2 border-b border-[var(--border)] flex items-center justify-between">
                <div className="flex items-center gap-3">
                    <h2 className="text-sm font-medium">Network</h2>
                    <span className="text-xs text-zinc-500">
                        {filteredRequests.length !== requests.length
                            ? `${filteredRequests.length}/${requests.length}`
                            : requests.length}
                    </span>
                </div>
                <div className="flex gap-1">
                    {Object.keys(FILTER_GROUPS).map((group) => (
                        <button
                            key={group}
                            onClick={() => toggleFilter(group as FilterGroup)}
                            className={`px-2 py-0.5 text-xs rounded transition-colors ${selectedFilters.has(group as FilterGroup)
                                    ? 'bg-[var(--accent)] text-white'
                                    : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'
                                }`}
                        >
                            {group}
                        </button>
                    ))}
                </div>
            </div>

            {/* Column Headers */}
            <div className="grid grid-cols-12 gap-2 px-4 py-1 text-xs font-medium text-zinc-500 border-b border-[var(--border)] bg-zinc-900/50">
                <div className="col-span-1">Status</div>
                <div className="col-span-1">Method</div>
                <div className="col-span-7">URL</div>
                <div className="col-span-2">Type</div>
                <div className="col-span-1">Time</div>
            </div>

            {/* Request List */}
            <div ref={scrollRef} className="flex-1 overflow-y-auto min-h-[150px] max-h-[300px]">
                {filteredRequests.length === 0 ? (
                    <div className="flex items-center justify-center h-full text-zinc-600 text-sm">
                        {requests.length === 0 ? 'No network activity' : 'No requests match filter'}
                    </div>
                ) : (
                    filteredRequests.map((req, i) => (
                        <div
                            key={`${req.requestId}-${i}`}
                            onClick={() => handleRequestClick(req)}
                            className={`grid grid-cols-12 gap-2 px-4 py-1.5 text-xs border-b border-[var(--border)]/50 hover:bg-zinc-800/50 cursor-pointer ${selectedRequest?.requestId === req.requestId ? 'bg-zinc-700/50' : ''
                                }`}
                        >
                            <div
                                className={`col-span-1 font-mono ${!req.status
                                        ? 'text-zinc-500'
                                        : req.status >= 400
                                            ? 'text-red-400'
                                            : req.status >= 300
                                                ? 'text-yellow-400'
                                                : 'text-green-400'
                                    }`}
                            >
                                {req.status || '...'}
                            </div>
                            <div
                                className={`col-span-1 font-bold ${req.method === 'GET'
                                        ? 'text-blue-400'
                                        : req.method === 'POST'
                                            ? 'text-green-400'
                                            : req.method === 'PUT'
                                                ? 'text-orange-400'
                                                : req.method === 'DELETE'
                                                    ? 'text-red-400'
                                                    : 'text-zinc-400'
                                    }`}
                            >
                                {req.method || '-'}
                            </div>
                            <div className="col-span-7 truncate text-zinc-300" title={req.url || ''}>
                                {req.url || ''}
                            </div>
                            <div className="col-span-2 text-zinc-500 truncate">{req.type || '-'}</div>
                            <div className="col-span-1 text-zinc-600 text-right">
                                {new Date(req.timestamp).toLocaleTimeString([], {
                                    hour12: false,
                                    hour: '2-digit',
                                    minute: '2-digit',
                                    second: '2-digit',
                                })}
                            </div>
                        </div>
                    ))
                )}
            </div>

            {/* Response Detail */}
            {selectedRequest && (
                <div className="border-t border-[var(--border)] p-3 bg-zinc-900/50 max-h-[200px] overflow-hidden flex flex-col">
                    <div className="flex items-center justify-between mb-2">
                        <span className="text-xs font-medium text-zinc-400">Response</span>
                        <button
                            onClick={() => {
                                setSelectedRequest(null);
                                setResponseBody(null);
                            }}
                            className="text-xs text-zinc-500 hover:text-zinc-300"
                        >
                            ✕
                        </button>
                    </div>
                    <div className="text-xs text-zinc-500 mb-2">
                        <span className="mr-3">
                            Status: <span className="text-zinc-300">{selectedRequest.status || 'pending'}</span>
                        </span>
                        <span className="mr-3">
                            Type: <span className="text-zinc-300">{selectedRequest.type || '-'}</span>
                        </span>
                        <span>
                            MIME: <span className="text-zinc-300">{selectedRequest.mimeType || '-'}</span>
                        </span>
                    </div>
                    <div className="flex-1 overflow-auto">
                        {loadingBody ? (
                            <div className="text-zinc-500 text-sm">Loading...</div>
                        ) : (
                            <pre className="text-xs font-mono text-zinc-300 whitespace-pre-wrap bg-black/30 p-2 rounded max-h-[120px] overflow-auto">
                                {responseBody || '(empty)'}
                            </pre>
                        )}
                    </div>
                </div>
            )}
        </div>
    );
}
