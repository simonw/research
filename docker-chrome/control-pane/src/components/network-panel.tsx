import React, { useRef, useEffect, useState, useMemo } from "react";
import { NetworkRequest, NetworkRequestDetails } from "@/lib/types";
import { NetworkFilter, FilterGroup, FILTER_GROUPS } from "./network-filter";
import { X, Trash2, Download, ChevronDown, ChevronRight } from "lucide-react";

interface NetworkPanelProps {
  requests: NetworkRequest[];
  onClearActivity?: () => void;
  apiBase?: string;
}

export function NetworkPanel({ requests, onClearActivity, apiBase = "https://docker-chrome-432753364585.us-central1.run.app" }: NetworkPanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [selectedFilters, setSelectedFilters] = useState<Set<FilterGroup>>(
    new Set()
  );

  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
  const [requestDetails, setRequestDetails] = useState<NetworkRequestDetails | null>(null);
  const [loadingDetails, setLoadingDetails] = useState(false);
  const [activeTab, setActiveTab] = useState<"headers" | "payload" | "response">("headers");
  const [expandedSections, setExpandedSections] = useState<Set<string>>(new Set(["requestHeaders", "responseHeaders"]));

  const toggleFilter = (group: FilterGroup) => {
    const newFilters = new Set(selectedFilters);
    if (newFilters.has(group)) {
      newFilters.delete(group);
    } else {
      newFilters.add(group);
    }
    setSelectedFilters(newFilters);
  };

  const toggleSection = (section: string) => {
    const newSections = new Set(expandedSections);
    if (newSections.has(section)) {
      newSections.delete(section);
    } else {
      newSections.add(section);
    }
    setExpandedSections(newSections);
  };

  const handleRequestClick = async (req: NetworkRequest) => {
    if (selectedRequestId === req.requestId) {
      setSelectedRequestId(null);
      setRequestDetails(null);
      return;
    }

    setSelectedRequestId(req.requestId);
    setRequestDetails(null);
    setLoadingDetails(true);
    setActiveTab("headers");

    try {
      const res = await fetch(`${apiBase}/api/network/${req.requestId}`);
      if (res.ok) {
        const data: NetworkRequestDetails = await res.json();
        setRequestDetails(data);
      } else {
        setRequestDetails(null);
      }
    } catch {
      setRequestDetails(null);
    } finally {
      setLoadingDetails(false);
    }
  };

  const filteredRequests = useMemo(() => {
    if (selectedFilters.size === 0) return requests;

    const hasCapturedFilter = selectedFilters.has("Captured");
    const otherFilters = new Set(selectedFilters);
    otherFilters.delete("Captured");

    const allowedTypes = new Set<string>();
    otherFilters.forEach((group) => {
      FILTER_GROUPS[group].forEach((type) => allowedTypes.add(type));
    });

    return requests.filter((req) => {
      if (hasCapturedFilter && otherFilters.size === 0) {
        return !!req.capturedByKey;
      }
      if (hasCapturedFilter && req.capturedByKey) {
        return true;
      }
      if (otherFilters.size > 0 && req.type && allowedTypes.has(req.type)) {
        return true;
      }
      return false;
    });
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
              <div className="col-span-7 flex items-center gap-2 min-w-0">
                <span className="truncate text-foreground font-medium group-hover:text-accent transition-colors" title={req.url || ''}>
                  {req.url || ''}
                </span>
                {req.capturedByKey && (
                  <span className="flex-shrink-0 inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-semibold bg-accent/10 text-accent border border-accent/20 rounded">
                    <Download className="w-2.5 h-2.5" />
                    {req.capturedByKey}
                  </span>
                )}
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

      {/* Request Details Panel */}
      {selectedRequestId && (
        <div className="border-t border-border bg-background/50 backdrop-blur-sm max-h-[400px] overflow-hidden flex flex-col animate-in slide-in-from-bottom duration-300">
          <div className="px-5 py-3 border-b border-border/50 flex items-center justify-between flex-shrink-0">
            <div className="flex items-center gap-4">
              <span className="text-sm font-semibold text-foreground">Request Details</span>
              {selectedRequest?.capturedByKey && (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-semibold bg-accent/10 text-accent border border-accent/20 rounded">
                  <Download className="w-3 h-3" />
                  {selectedRequest.capturedByKey}
                </span>
              )}
            </div>
            <button
              onClick={(e) => {
                e.stopPropagation();
                setSelectedRequestId(null);
                setRequestDetails(null);
              }}
              className="p-1.5 hover:bg-surface rounded-lg transition-all duration-200 text-text-secondary hover:text-foreground group/close"
              aria-label="Close request details"
            >
              <X size={16} className="group-hover/close:rotate-90 transition-transform duration-200" />
            </button>
          </div>

          {selectedRequest && (
            <>
              <div className="px-5 py-2 text-xs text-foreground break-all border-b border-border/50 flex-shrink-0 bg-surface/50">
                <span className="text-text-secondary font-medium">{selectedRequest.method} </span>
                <span className="font-mono">{selectedRequest.url}</span>
              </div>

              <div className="px-5 py-2 text-xs text-text-secondary flex gap-6 flex-shrink-0 border-b border-border/50 bg-surface/30">
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

              {/* Tabs */}
              <div className="flex border-b border-border/50 bg-surface/20 flex-shrink-0">
                {(["headers", "payload", "response"] as const).map((tab) => (
                  <button
                    key={tab}
                    onClick={() => setActiveTab(tab)}
                    className={`px-4 py-2.5 text-xs font-medium capitalize transition-colors ${
                      activeTab === tab
                        ? "text-accent border-b-2 border-accent bg-background/50"
                        : "text-text-secondary hover:text-foreground hover:bg-background/30"
                    }`}
                  >
                    {tab}
                  </button>
                ))}
              </div>
            </>
          )}

          <div className="flex-1 overflow-auto min-h-0 p-4">
            {loadingDetails ? (
              <div className="flex items-center gap-3 text-text-secondary text-sm">
                <div className="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
                <span>Loading request details...</span>
              </div>
            ) : requestDetails ? (
              <>
                {activeTab === "headers" && (
                  <div className="space-y-3">
                    {/* Request Headers */}
                    <div className="border border-border rounded-lg overflow-hidden">
                      <button
                        onClick={() => toggleSection("requestHeaders")}
                        className="w-full flex items-center gap-2 px-3 py-2 bg-surface/50 hover:bg-surface transition-colors"
                      >
                        {expandedSections.has("requestHeaders") ? (
                          <ChevronDown className="w-3.5 h-3.5 text-text-secondary" />
                        ) : (
                          <ChevronRight className="w-3.5 h-3.5 text-text-secondary" />
                        )}
                        <span className="text-xs font-semibold text-foreground">Request Headers</span>
                        <span className="text-xs text-text-tertiary">({Object.keys(requestDetails.requestHeaders).length})</span>
                      </button>
                      {expandedSections.has("requestHeaders") && (
                        <div className="px-3 py-2 bg-background/30 border-t border-border/50">
                          {Object.entries(requestDetails.requestHeaders).length > 0 ? (
                            Object.entries(requestDetails.requestHeaders).map(([key, value]) => (
                              <div key={key} className="flex gap-2 py-1 text-xs font-mono">
                                <span className="text-accent font-medium">{key}:</span>
                                <span className="text-foreground break-all">{value}</span>
                              </div>
                            ))
                          ) : (
                            <span className="text-xs text-text-tertiary">No request headers</span>
                          )}
                        </div>
                      )}
                    </div>

                    {/* Response Headers */}
                    <div className="border border-border rounded-lg overflow-hidden">
                      <button
                        onClick={() => toggleSection("responseHeaders")}
                        className="w-full flex items-center gap-2 px-3 py-2 bg-surface/50 hover:bg-surface transition-colors"
                      >
                        {expandedSections.has("responseHeaders") ? (
                          <ChevronDown className="w-3.5 h-3.5 text-text-secondary" />
                        ) : (
                          <ChevronRight className="w-3.5 h-3.5 text-text-secondary" />
                        )}
                        <span className="text-xs font-semibold text-foreground">Response Headers</span>
                        <span className="text-xs text-text-tertiary">({Object.keys(requestDetails.responseHeaders).length})</span>
                      </button>
                      {expandedSections.has("responseHeaders") && (
                        <div className="px-3 py-2 bg-background/30 border-t border-border/50">
                          {Object.entries(requestDetails.responseHeaders).length > 0 ? (
                            Object.entries(requestDetails.responseHeaders).map(([key, value]) => (
                              <div key={key} className="flex gap-2 py-1 text-xs font-mono">
                                <span className="text-accent font-medium">{key}:</span>
                                <span className="text-foreground break-all">{value}</span>
                              </div>
                            ))
                          ) : (
                            <span className="text-xs text-text-tertiary">No response headers</span>
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                )}

                {activeTab === "payload" && (
                  <div>
                    {requestDetails.requestBody ? (
                      <pre className="text-xs font-mono text-foreground whitespace-pre-wrap bg-surface border border-border p-3 rounded-lg leading-relaxed">
                        {(() => {
                          try {
                            return JSON.stringify(JSON.parse(requestDetails.requestBody), null, 2);
                          } catch {
                            return requestDetails.requestBody;
                          }
                        })()}
                      </pre>
                    ) : (
                      <div className="text-xs text-text-tertiary text-center py-8">No request payload</div>
                    )}
                  </div>
                )}

                {activeTab === "response" && (
                  <div>
                    {requestDetails.responseBody ? (
                      <pre className="text-xs font-mono text-foreground whitespace-pre-wrap bg-surface border border-border p-3 rounded-lg leading-relaxed max-h-[200px] overflow-auto">
                        {(() => {
                          try {
                            return JSON.stringify(JSON.parse(requestDetails.responseBody), null, 2);
                          } catch {
                            return requestDetails.responseBody;
                          }
                        })()}
                      </pre>
                    ) : (
                      <div className="text-xs text-text-tertiary text-center py-8">No response body</div>
                    )}
                  </div>
                )}
              </>
            ) : (
              <div className="text-xs text-text-tertiary text-center py-8">Request details not available</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
