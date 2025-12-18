// OPTIMIZED NetworkPanel with Virtualization & Performance Improvements
// This is a conceptual implementation showing the key changes needed

import React, { useRef, useEffect, useState, useMemo, useCallback } from "react";
import { useVirtualizer } from '@tanstack/react-virtual';
import { NetworkRequest } from "@/lib/types";
import { X, FileText, Search, Filter } from "lucide-react";

const API_BASE = "https://docker-chrome-432753364585.us-central1.run.app";

// Response caching store
const responseStore = new Map<string, string>();

interface NetworkPanelProps {
  requests: NetworkRequest[];
}

interface Filters {
  url: string;
  method: string;
  status: string;
}

export function NetworkPanel({ requests }: NetworkPanelProps) {
  const [selectedReq, setSelectedReq] = useState<NetworkRequest | null>(null);
  const [responseBody, setResponseBody] = useState<string | null>(null);
  const [loadingBody, setLoadingBody] = useState(false);
  const [filters, setFilters] = useState<Filters>({ url: '', method: '', status: '' });
  
  const scrollRef = useRef<HTMLDivElement>(null);

  // FILTERED REQUESTS - Memoized filtering
  const filteredRequests = useMemo(() => {
    return requests.filter(req => {
      const matchesUrl = !filters.url || req.url.toLowerCase().includes(filters.url.toLowerCase());
      const matchesMethod = !filters.method || req.method === filters.method;
      const matchesStatus = !filters.status || (req.status?.toString() === filters.status);
      return matchesUrl && matchesMethod && matchesStatus;
    });
  }, [requests, filters]);

  // VIRTUALIZATION SETUP
  const rowVirtualizer = useVirtualizer({
    count: filteredRequests.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 48, // Height per row
    overscan: 5, // Render extra items for smooth scrolling
  });

  // MEMOIZED RESPONSE BODY FORMATTING
  const formattedBody = useMemo(() => {
    if (!responseBody) return <span className="text-zinc-600 italic">No content or failed to load</span>;
    
    try {
      const json = JSON.parse(responseBody);
      return <pre className="text-xs font-mono text-green-300 overflow-auto whitespace-pre-wrap break-all">
        {JSON.stringify(json, null, 2)}
      </pre>;
    } catch {
      return <pre className="text-xs font-mono text-zinc-300 overflow-auto whitespace-pre-wrap break-all">
        {responseBody}
      </pre>;
    }
  }, [responseBody]);

  // RESPONSE LOADING WITH CACHING
  useEffect(() => {
    if (!selectedReq) return;

    const cacheKey = selectedReq.requestId;
    const cached = responseStore.get(cacheKey);
    
    if (cached) {
      setResponseBody(cached);
      return;
    }

    setResponseBody(null);
    setLoadingBody(true);
    
    fetch(`${API_BASE}/api/network/${selectedReq.requestId}/body`)
      .then(async res => {
        if (!res.ok) throw new Error('Failed');
        const data = await res.json();
        responseStore.set(cacheKey, data.body); // Cache the response
        setResponseBody(data.body);
      })
      .catch(() => setResponseBody(null))
      .finally(() => setLoadingBody(false));
  }, [selectedReq]);

  // MEMOIZED ROW RENDERER
  const renderRow = useCallback((virtualItem: any) => {
    const req = filteredRequests[virtualItem.index];
    return (
      <div 
        key={`${req.requestId}-${virtualItem.index}`}
        onClick={() => setSelectedReq(req)}
        className={`grid grid-cols-12 gap-2 px-4 py-2 text-xs border-b border-border/50 cursor-pointer transition-colors ${
          selectedReq?.requestId === req.requestId ? 'bg-accent/10 border-accent/20' : 'hover:bg-zinc-800/50'
        }`}
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          width: '100%',
          height: `${virtualItem.size}px`,
          transform: `translateY(${virtualItem.start}px)`,
        }}
      >
        {/* Row content same as original */}
        <div className={`col-span-1 font-mono ${
          !req.status ? 'text-zinc-500' :
          req.status >= 400 ? 'text-red-400' : 
          req.status >= 300 ? 'text-yellow-400' : 
          'text-green-400'
        }`}>
          {req.status || '...'}
        </div>
        <div className={`col-span-1 font-bold ${
          req.method === 'GET' ? 'text-blue-400' :
          req.method === 'POST' ? 'text-green-400' :
          req.method === 'PUT' ? 'text-orange-400' :
          req.method === 'DELETE' ? 'text-red-400' :
          'text-zinc-400'
        }`}>
          {req.method}
        </div>
        <div className="col-span-7 truncate text-zinc-300" title={req.url}>
          {req.url}
        </div>
        <div className="col-span-2 text-zinc-500 truncate">
          {req.type}
        </div>
        <div className="col-span-1 text-zinc-600 text-right">
          {new Date(req.timestamp).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute:'2-digit', second:'2-digit' })}
        </div>
      </div>
    );
  }, [filteredRequests, selectedReq]);

  return (
    <div className="flex flex-col h-full bg-surface border border-border rounded-lg overflow-hidden relative">
      {/* Header with filters */}
      <div className="px-4 py-2 border-b border-border bg-surface flex items-center justify-between shrink-0">
        <h2 className="text-sm font-medium text-foreground">Network Activity</h2>
        <div className="flex items-center gap-2">
          <span className="text-xs text-zinc-500">{filteredRequests.length} requests</span>
          <Filter size={14} className="text-zinc-500" />
        </div>
      </div>
      
      {/* Filters */}
      <div className="px-4 py-2 border-b border-border/50 bg-zinc-900/50 shrink-0">
        <div className="flex gap-2 text-xs">
          <input
            placeholder="Filter URLs..."
            value={filters.url}
            onChange={(e) => setFilters(f => ({ ...f, url: e.target.value }))}
            className="flex-1 bg-zinc-800 border border-border rounded px-2 py-1"
          />
          <select
            value={filters.method}
            onChange={(e) => setFilters(f => ({ ...f, method: e.target.value }))}
            className="bg-zinc-800 border border-border rounded px-2 py-1"
          >
            <option value="">All Methods</option>
            <option value="GET">GET</option>
            <option value="POST">POST</option>
            <option value="PUT">PUT</option>
            <option value="DELETE">DELETE</option>
          </select>
          <select
            value={filters.status}
            onChange={(e) => setFilters(f => ({ ...f, status: e.target.value }))}
            className="bg-zinc-800 border border-border rounded px-2 py-1"
          >
            <option value="">All Status</option>
            <option value="200">200</option>
            <option value="400">4xx</option>
            <option value="500">5xx</option>
          </select>
        </div>
      </div>

      {/* Column headers */}
      <div className="grid grid-cols-12 gap-2 px-4 py-2 text-xs font-medium text-zinc-500 border-b border-border bg-zinc-900/50 shrink-0">
        <div className="col-span-1">Stat</div>
        <div className="col-span-1">Meth</div>
        <div className="col-span-7">URL</div>
        <div className="col-span-2">Type</div>
        <div className="col-span-1">Time</div>
      </div>

      {/* VIRTUALIZED LIST */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto min-h-0">
        {filteredRequests.length === 0 ? (
          <div className="flex items-center justify-center h-full text-zinc-600 text-sm">
            No network activity recorded
          </div>
        ) : (
          <div
            style={{
              height: `${rowVirtualizer.getTotalSize()}px`,
              width: '100%',
              position: 'relative',
            }}
          >
            {rowVirtualizer.getVirtualItems().map(renderRow)}
          </div>
        )}
      </div>

      {/* Response detail panel - unchanged */}
      {selectedReq && (
        <div className="absolute inset-0 z-10 bg-zinc-900/95 backdrop-blur-sm flex flex-col animate-in slide-in-from-right-10 duration-200">
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b border-border bg-zinc-900 shrink-0">
            <div className="flex flex-col overflow-hidden max-w-[80%]">
              <div className="flex items-center gap-2">
                <span className={`text-xs font-bold px-1.5 py-0.5 rounded ${
                  selectedReq.method === 'GET' ? 'bg-blue-500/20 text-blue-400' :
                  selectedReq.method === 'POST' ? 'bg-green-500/20 text-green-400' :
                  'bg-zinc-500/20 text-zinc-400'
                }`}>{selectedReq.method}</span>
                <span className="text-sm font-medium truncate text-foreground" title={selectedReq.url}>
                  {selectedReq.url}
                </span>
              </div>
            </div>
            <button 
              onClick={() => setSelectedReq(null)}
              className="p-2 hover:bg-zinc-800 rounded-lg text-zinc-400 hover:text-white transition-colors"
            >
              <X size={18} />
            </button>
          </div>
          
          {/* Content */}
          <div className="flex-1 overflow-y-auto p-4 space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="p-3 rounded-lg bg-zinc-900 border border-border">
                <span className="text-xs text-zinc-500 block mb-1">Status</span>
                <span className={`text-sm font-mono ${
                  !selectedReq.status ? 'text-zinc-500' :
                  selectedReq.status >= 400 ? 'text-red-400' : 
                  selectedReq.status >= 300 ? 'text-yellow-400' : 
                  'text-green-400'
                }`}>{selectedReq.status || 'Pending'}</span>
              </div>
              <div className="p-3 rounded-lg bg-zinc-900 border border-border">
                <span className="text-xs text-zinc-500 block mb-1">Type</span>
                <span className="text-sm text-zinc-300">{selectedReq.type}</span>
              </div>
            </div>

            <div className="flex flex-col gap-2 h-full min-h-[200px]">
              <div className="flex items-center gap-2 text-xs text-zinc-500 uppercase tracking-wider font-semibold">
                <FileText size={14} /> Response Body
              </div>
              <div className="flex-1 rounded-lg bg-zinc-950 border border-border p-4 overflow-hidden relative group">
                {loadingBody ? (
                  <div className="flex items-center justify-center h-full text-zinc-500 text-sm animate-pulse">
                    Loading response...
                  </div>
                ) : (
                  <div className="h-full overflow-auto">
                    {formattedBody}
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
