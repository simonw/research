import React, { useRef, useEffect, useState, useMemo } from "react";
import { NetworkRequest } from "@/lib/types";
import { NetworkFilter, FilterGroup, FILTER_GROUPS } from "./network-filter";
import { X, Trash2 } from "lucide-react";

interface NetworkPanelProps {
  requests: NetworkRequest[];
  onClearActivity?: () => void;
}

export function NetworkPanel({ requests, onClearActivity }: NetworkPanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [selectedFilters, setSelectedFilters] = useState<Set<FilterGroup>>(
    new Set()
  );

  // Bug fix: Store requestId separately to maintain panel visibility during async fetch
  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
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
    // Toggle off if clicking the same request
    if (selectedRequestId === req.requestId) {
      setSelectedRequestId(null);
      setResponseBody(null);
      return;
    }

    // Set selected ID immediately to show panel
    setSelectedRequestId(req.requestId);
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

  // Find the currently selected request object
  const selectedRequest = useMemo(
    () => requests.find(r => r.requestId === selectedRequestId) || null,
    [requests, selectedRequestId]
  );

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [filteredRequests]);

  return (
    <div className="flex flex-col h-full bg-surface border border-border rounded-lg overflow-hidden shadow-sm">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border bg-surface flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-base font-semibold text-foreground tracking-tight">
            Network Activity
          </h2>
          <span className="text-xs text-text-secondary font-medium bg-background px-2.5 py-1 rounded-full border border-border">
            {filteredRequests.length !== requests.length
              ? `${filteredRequests.length}/${requests.length}`
              : `${requests.length}`}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {onClearActivity && (
            <button
              onClick={onClearActivity}
              className="flex items-center gap-2 px-3.5 py-2 text-xs font-medium bg-surface border border-border rounded-lg hover:border-error/50 hover:bg-error/5 transition-all duration-200 text-text-secondary hover:text-error shadow-sm"
              title="Clear all network activity"
            >
              <Trash2 className="w-3.5 h-3.5" />
              <span className="font-semibold">Clear</span>
            </button>
          )}
          <NetworkFilter
            selectedGroups={selectedFilters}
            onToggleGroup={toggleFilter}
          />
        </div>
      </div>

      {/* Column Headers */}
      <div className="grid grid-cols-12 gap-3 px-5 py-3 text-xs font-semibold text-text-secondary border-b border-border bg-background/50 uppercase tracking-wider">
        <div className="col-span-1">Status</div>
        <div className="col-span-1">Method</div>
        <div className="col-span-7">URL</div>
        <div className="col-span-2">Type</div>
        <div className="col-span-1 text-right">Time</div>
      </div>

      {/* Request List */}
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto min-h-[200px] max-h-[400px]"
      >
        {filteredRequests.length === 0 ? (
          <div className="flex items-center justify-center h-full text-text-tertiary text-sm py-12">
            {requests.length === 0
              ? "No network activity recorded"
              : "No requests match filter"}
          </div>
        ) : (
          filteredRequests.map((req, i) => (
            <div
              key={`${req.requestId}-${i}`}
              onClick={() => handleRequestClick(req)}
              className={`grid grid-cols-12 gap-3 px-5 py-3 text-sm border-b border-border/50 hover:bg-background/80 transition-all duration-200 cursor-pointer group relative ${selectedRequestId === req.requestId
                ? "bg-accent/5 border-l-2 border-l-accent"
                : "border-l-2 border-l-transparent"
                }`}
            >
              {/* Status Code */}
              <div className={`col-span-1 font-mono font-semibold transition-colors ${!req.status ? 'text-text-tertiary' :
                req.status >= 400 ? 'text-error' :
                  req.status >= 300 ? 'text-accent' :
                    'text-success'
                }`}>
                {req.status || '···'}
              </div>

              {/* HTTP Method */}
              <div className={`col-span-1 font-bold text-xs uppercase tracking-wide transition-colors ${req.method === 'GET' ? 'text-accent' :
                req.method === 'POST' ? 'text-success' :
                  req.method === 'PUT' ? 'text-accent' :
                    req.method === 'DELETE' ? 'text-error' :
                      'text-text-secondary'
                }`}>
                {req.method || '—'}
              </div>

              {/* URL */}
              <div className="col-span-7 truncate text-foreground font-medium group-hover:text-accent transition-colors" title={req.url || ''}>
                {req.url || ''}
              </div>

              {/* Resource Type */}
              <div className="col-span-2 text-text-secondary text-xs truncate uppercase tracking-wide">
                {req.type || '—'}
              </div>

              {/* Timestamp */}
              <div className="col-span-1 text-text-tertiary text-xs text-right font-mono">
                {new Date(req.timestamp).toLocaleTimeString([], {
                  hour12: false,
                  hour: '2-digit',
                  minute: '2-digit',
                  second: '2-digit'
                })}
              </div>
            </div>
          ))
        )}
      </div>

      {/* Response Details Panel - Now properly controlled by selectedRequestId */}
      {selectedRequestId && (
        <div className="border-t border-border bg-background/50 backdrop-blur-sm max-h-[320px] overflow-hidden flex flex-col animate-in slide-in-from-bottom duration-300">
          <div className="px-5 py-4 border-b border-border/50 flex items-center justify-between flex-shrink-0">
            <span className="text-sm font-semibold text-foreground">Response Details</span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                setSelectedRequestId(null);
                setResponseBody(null);
              }}
              className="p-1.5 hover:bg-surface rounded-lg transition-all duration-200 text-text-secondary hover:text-foreground group/close"
              aria-label="Close response details"
            >
              <X size={16} className="group-hover/close:rotate-90 transition-transform duration-200" />
            </button>
          </div>

          {selectedRequest && (
            <>
              <div className="px-5 py-3 text-xs text-foreground break-all border-b border-border/50 flex-shrink-0 bg-surface/50">
                <span className="text-text-secondary font-medium">URL: </span>
                <span className="font-mono">{selectedRequest.url}</span>
              </div>

              <div className="px-5 py-2.5 text-xs text-text-secondary flex gap-6 flex-shrink-0 border-b border-border/50 bg-surface/30">
                <span>
                  Status: <span className={`font-semibold ${!selectedRequest.status ? 'text-text-tertiary' :
                    selectedRequest.status >= 400 ? 'text-error' :
                      selectedRequest.status >= 300 ? 'text-accent' :
                        'text-success'
                    }`}>{selectedRequest.status || 'pending'}</span>
                </span>
                <span>Type: <span className="font-semibold text-foreground">{selectedRequest.type || '—'}</span></span>
                <span>MIME: <span className="font-semibold text-foreground font-mono">{selectedRequest.mimeType || '—'}</span></span>
              </div>
            </>
          )}

          <div className="flex-1 overflow-auto min-h-0 p-5">
            {loadingBody ? (
              <div className="flex items-center gap-3 text-text-secondary text-sm">
                <div className="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
                <span>Loading response body...</span>
              </div>
            ) : (
              <pre className="text-xs font-mono text-foreground whitespace-pre-wrap bg-surface border border-border p-4 rounded-lg leading-relaxed shadow-inner">
                {responseBody || '(empty response)'}
              </pre>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
