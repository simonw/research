import React, { useRef, useEffect, useState, useMemo } from "react";
import { NetworkRequest } from "@/lib/types";
import { NetworkFilter, FilterGroup, FILTER_GROUPS } from "./network-filter";

interface NetworkPanelProps {
  requests: NetworkRequest[];
}

export function NetworkPanel({ requests }: NetworkPanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [selectedFilters, setSelectedFilters] = useState<Set<FilterGroup>>(
    new Set()
  );

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

  const API_BASE = "https://docker-chrome-432753364585.us-central1.run.app";

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
      const res = await fetch(`${API_BASE}/api/network/${req.requestId}/body`);
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
        try {
          const parsed = JSON.parse(body);
          body = JSON.stringify(parsed, null, 2);
        } catch {}
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

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [filteredRequests]);

  return (
    <div className="flex flex-col h-full bg-surface border border-border rounded-lg overflow-hidden">
      <div className="px-4 py-2 border-b border-border bg-surface flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-sm font-medium text-foreground">
            Network Activity
          </h2>
          <span className="text-xs text-zinc-500">
            {filteredRequests.length !== requests.length
              ? `${filteredRequests.length}/${requests.length} requests`
              : `${requests.length} requests`}
          </span>
        </div>
        <NetworkFilter
          selectedGroups={selectedFilters}
          onToggleGroup={toggleFilter}
        />
      </div>

      <div className="grid grid-cols-12 gap-2 px-4 py-2 text-xs font-medium text-zinc-500 border-b border-border bg-zinc-900/50">
        <div className="col-span-1">Stat</div>
        <div className="col-span-1">Meth</div>
        <div className="col-span-7">URL</div>
        <div className="col-span-2">Type</div>
        <div className="col-span-1">Time</div>
      </div>

      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto min-h-[200px] max-h-[400px]"
      >
        {filteredRequests.length === 0 ? (
          <div className="flex items-center justify-center h-full text-zinc-600 text-sm">
            {requests.length === 0
              ? "No network activity recorded"
              : "No requests match filter"}
          </div>
        ) : (
          filteredRequests.map((req, i) => (
            <div
              key={`${req.requestId}-${i}`}
              onClick={() => handleRequestClick(req)}
              className={`grid grid-cols-12 gap-2 px-4 py-2 text-xs border-b border-border/50 hover:bg-zinc-800/50 transition-colors cursor-pointer ${
                selectedRequest?.requestId === req.requestId ? "bg-zinc-700/50" : ""
              }`}
            >
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
                {req.method || '-'}
              </div>
              <div className="col-span-7 truncate text-zinc-300" title={req.url || ''}>
                {req.url || ''}
              </div>
              <div className="col-span-2 text-zinc-500 truncate">
                {req.type || '-'}
              </div>
              <div className="col-span-1 text-zinc-600 text-right">
                {new Date(req.timestamp).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute:'2-digit', second:'2-digit' })}
              </div>
            </div>
          ))
        )}
      </div>

      {selectedRequest && (
        <div className="border-t border-border p-4 bg-zinc-900/50 max-h-[300px] overflow-hidden flex flex-col">
          <div className="flex items-center justify-between mb-2 flex-shrink-0">
            <span className="text-xs font-medium text-zinc-400">Response Details</span>
            <button 
              onClick={() => { setSelectedRequest(null); setResponseBody(null); }} 
              className="text-xs text-zinc-500 hover:text-zinc-300 px-2"
            >
              ✕
            </button>
          </div>
          <div className="text-xs text-zinc-300 break-all truncate mb-2 flex-shrink-0">
            <span className="text-zinc-500">URL:</span> {selectedRequest.url}
          </div>
          <div className="text-xs text-zinc-500 mb-2 flex-shrink-0">
            <span className="mr-3">Status: <span className="text-zinc-300">{selectedRequest.status || 'pending'}</span></span>
            <span className="mr-3">Type: <span className="text-zinc-300">{selectedRequest.type || '-'}</span></span>
            <span>MIME: <span className="text-zinc-300">{selectedRequest.mimeType || '-'}</span></span>
          </div>
          <div className="flex-1 overflow-auto min-h-0">
            {loadingBody ? (
              <div className="text-zinc-500 text-sm">Loading...</div>
            ) : (
              <pre className="text-xs font-mono text-zinc-300 whitespace-pre-wrap bg-black/30 p-2 rounded overflow-auto max-h-[180px]">
                {responseBody || '(empty)'}
              </pre>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
