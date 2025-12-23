'use client';

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { 
  X, 
  Trash2, 
  Globe, 
  Clock, 
  FileText, 
  Send, 
  ArrowDown, 
  Layout, 
  Code, 
  Braces 
} from 'lucide-react';
import { NetworkRequest, NetworkRequestDetails, ResourceType } from '@/lib/types';
import { getNetworkRequestDetails, clearNetworkRequests } from '@/lib/api';
import { NetworkFilter, FilterGroup, FILTER_GROUPS } from './network-filter';

interface NetworkPanelProps {
  requests: NetworkRequest[];
  sessionId: string | null;
  onClearRequests: () => void;
}

const STATUS_COLORS = {
  2: 'text-green-400',
  3: 'text-yellow-400',
  4: 'text-orange-400',
  5: 'text-red-400',
  0: 'text-text-tertiary',
};

const getStatusColor = (status?: number) => {
  if (!status) return STATUS_COLORS[0];
  const firstDigit = Math.floor(status / 100);
  return STATUS_COLORS[firstDigit as keyof typeof STATUS_COLORS] || STATUS_COLORS[0];
};

const formatTime = (ms: number) => {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
};

const formatSize = (bytes?: number) => {
  if (!bytes) return '-';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

export function NetworkPanel({ requests, sessionId, onClearRequests }: NetworkPanelProps) {
  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
  const [requestDetails, setRequestDetails] = useState<NetworkRequestDetails | null>(null);
  const [loadingDetails, setLoadingDetails] = useState(false);
  const [selectedGroups, setSelectedGroups] = useState<Set<FilterGroup>>(new Set(Object.keys(FILTER_GROUPS) as FilterGroup[]));
  const [activeTab, setActiveTab] = useState<'headers' | 'payload' | 'response'>('headers');

  const handleToggleGroup = useCallback((group: FilterGroup) => {
    setSelectedGroups(prev => {
      const next = new Set(prev);
      if (next.has(group)) {
        next.delete(group);
      } else {
        next.add(group);
      }
      return next;
    });
  }, []);

  const filteredRequests = useMemo(() => {
    return requests.filter(req => {
      if (req.capturedByKey) {
        return selectedGroups.has('Captured');
      }

      for (const [group, types] of Object.entries(FILTER_GROUPS)) {
        if (group === 'Captured') continue;
        if ((types as ResourceType[]).includes(req.type)) {
          return selectedGroups.has(group as FilterGroup);
        }
      }
      
      return selectedGroups.has('Other');
    });
  }, [requests, selectedGroups]);

  const groupCounts = useMemo(() => {
    const counts = {} as Record<FilterGroup, number>;
    (Object.keys(FILTER_GROUPS) as FilterGroup[]).forEach(g => counts[g] = 0);

    requests.forEach(req => {
      if (req.capturedByKey) {
        counts['Captured']++;
        return;
      }
      for (const [group, types] of Object.entries(FILTER_GROUPS)) {
        if (group === 'Captured') continue;
        if ((types as ResourceType[]).includes(req.type)) {
          counts[group as FilterGroup]++;
          return;
        }
      }
      counts['Other']++;
    });
    return counts;
  }, [requests]);

  const selectedRequest = useMemo(() => 
    requests.find(r => r.requestId === selectedRequestId),
    [requests, selectedRequestId]
  );

  useEffect(() => {
    if (!selectedRequestId || !sessionId) {
      setRequestDetails(null);
      return;
    }

    const fetchDetails = async () => {
      setLoadingDetails(true);
      try {
        const details = await getNetworkRequestDetails(sessionId, selectedRequestId);
        setRequestDetails(details);
      } catch (error) {
        console.error('Failed to fetch request details:', error);
        setRequestDetails(null);
      } finally {
        setLoadingDetails(false);
      }
    };

    fetchDetails();
  }, [selectedRequestId, sessionId]);

  const handleClear = useCallback(async () => {
    if (!sessionId) return;
    onClearRequests();
    setSelectedRequestId(null);
    try {
      await clearNetworkRequests(sessionId);
    } catch (error) {
      console.error('Failed to clear network requests:', error);
    }
  }, [sessionId, onClearRequests]);

  const JsonViewer = ({ data }: { data: string }) => {
    try {
      const parsed = JSON.parse(data);
      return (
        <pre className="text-xs font-mono whitespace-pre-wrap text-text-secondary overflow-auto max-h-[500px] p-2 bg-background/50 rounded border border-border/50">
          {JSON.stringify(parsed, null, 2)}
        </pre>
      );
    } catch {
      return (
        <pre className="text-xs font-mono whitespace-pre-wrap text-text-secondary overflow-auto max-h-[500px] p-2 bg-background/50 rounded border border-border/50">
          {data}
        </pre>
      );
    }
  };

  return (
    <div className="flex flex-col h-full bg-surface border border-border rounded-lg shadow-sm">
      <div className="flex items-center justify-between px-4 py-2 border-b border-border bg-surface/50 backdrop-blur-sm rounded-t-lg">
        <div className="flex items-center gap-4">
          <NetworkFilter 
            selectedGroups={selectedGroups} 
            onToggleGroup={handleToggleGroup}
            groupCounts={groupCounts}
          />
          <div className="h-4 w-px bg-border/50" />
          <span className="text-xs text-text-tertiary font-mono">
            {filteredRequests.length} / {requests.length} requests
          </span>
        </div>
        <button
          onClick={handleClear}
          className="p-1.5 text-text-tertiary hover:text-error hover:bg-error/10 rounded-md transition-colors"
          title="Clear network log"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 flex overflow-hidden relative rounded-b-lg min-h-[300px]">
        <div className="flex-1 overflow-auto bg-background custom-scrollbar">
          <table className="w-full text-left border-collapse">
            <thead className="sticky top-0 z-10 bg-surface border-b border-border shadow-sm">
              <tr>
                <th className="px-4 py-2 text-xs font-medium text-text-tertiary whitespace-nowrap w-24">Status</th>
                <th className="px-4 py-2 text-xs font-medium text-text-tertiary whitespace-nowrap w-20">Method</th>
                <th className="px-4 py-2 text-xs font-medium text-text-tertiary whitespace-nowrap">Name</th>
                <th className="px-4 py-2 text-xs font-medium text-text-tertiary whitespace-nowrap w-24">Type</th>
                <th className="px-4 py-2 text-xs font-medium text-text-tertiary whitespace-nowrap w-20 text-right">Time</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border/30">
              {filteredRequests.map((req) => (
                <tr 
                  key={req.requestId}
                  onClick={() => setSelectedRequestId(req.requestId)}
                  className={`
                    group cursor-pointer transition-colors text-sm
                    ${selectedRequestId === req.requestId 
                      ? 'bg-accent/5 hover:bg-accent/10' 
                      : 'hover:bg-surface'
                    }
                  `}
                >
                  <td className="px-4 py-2 whitespace-nowrap">
                    <div className="flex items-center gap-2">
                      <div className={`w-1.5 h-1.5 rounded-full ${req.status === 0 ? 'bg-text-tertiary' : req.status && req.status < 400 ? 'bg-green-500' : 'bg-red-500'}`} />
                      <span className={`font-mono font-medium ${getStatusColor(req.status)}`}>
                        {req.status || (req.errorText ? 'ERR' : '...')}
                      </span>
                    </div>
                  </td>
                  <td className="px-4 py-2 whitespace-nowrap font-mono text-xs text-text-secondary">
                    {req.method}
                  </td>
                  <td className="px-4 py-2 max-w-[300px]">
                    <div className="flex flex-col truncate">
                      <span 
                        className={`truncate font-medium ${selectedRequestId === req.requestId ? 'text-accent' : 'text-foreground'}`} 
                        title={req.url}
                      >
                        {req.url.split('/').pop()?.split('?')[0] || '/'}
                      </span>
                      <span className="text-xs text-text-tertiary truncate font-mono opacity-60">
                        {req.url}
                      </span>
                    </div>
                  </td>
                  <td className="px-4 py-2 whitespace-nowrap text-xs text-text-secondary">
                    {req.capturedByKey ? (
                      <span 
                        className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-accent/10 text-accent border border-accent/20 max-w-[100px] truncate" 
                        title={`Captured as: ${req.capturedByKey}`}
                      >
                        {req.capturedByKey}
                      </span>
                    ) : (
                      <span className="opacity-80">{req.type}</span>
                    )}
                  </td>
                  <td className="px-4 py-2 whitespace-nowrap text-xs text-text-tertiary text-right font-mono">
                    {req.responseTimestamp 
                      ? formatTime(req.responseTimestamp - req.timestamp)
                      : <span className="animate-pulse">...</span>
                    }
                  </td>
                </tr>
              ))}
              {filteredRequests.length === 0 && (
                <tr>
                  <td colSpan={5} className="px-4 py-12 text-center text-text-tertiary">
                    <div className="flex flex-col items-center gap-3">
                      <Globe className="w-8 h-8 opacity-20" />
                      <p className="text-sm">No requests recorded</p>
                    </div>
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <div 
          className={`
            absolute top-0 right-0 h-full w-[500px] bg-surface border-l border-border shadow-2xl z-20
            transform transition-transform duration-300 ease-out flex flex-col
            ${selectedRequestId ? 'translate-x-0' : 'translate-x-full'}
          `}
        >
          {selectedRequest && (
            <>
              <div className="flex items-center justify-between px-4 py-3 border-b border-border bg-surface">
                <div className="flex flex-col overflow-hidden mr-4">
                  <h3 className="text-sm font-medium text-foreground truncate" title={selectedRequest.url}>
                    {selectedRequest.url.split('/').pop()?.split('?')[0] || '/'}
                  </h3>
                  <div className="flex items-center gap-2 text-xs text-text-tertiary mt-0.5">
                    <span className={`font-mono ${getStatusColor(selectedRequest.status)}`}>
                      {selectedRequest.method} {selectedRequest.status || '...'}
                    </span>
                    <span>•</span>
                    <span className="font-mono">{selectedRequest.type}</span>
                  </div>
                </div>
                <button 
                  onClick={() => setSelectedRequestId(null)}
                  className="p-1.5 text-text-tertiary hover:text-foreground hover:bg-background rounded-md transition-colors shrink-0"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>

              <div className="flex px-4 border-b border-border bg-background/50">
                {[
                  { id: 'headers', icon: Layout, label: 'Headers' },
                  { id: 'payload', icon: Code, label: 'Payload' },
                  { id: 'response', icon: Braces, label: 'Response' },
                ].map((tab) => (
                  <button
                    key={tab.id}
                    onClick={() => setActiveTab(tab.id as any)}
                    className={`
                      flex items-center gap-2 px-4 py-2.5 text-xs font-medium border-b-2 transition-colors
                      ${activeTab === tab.id 
                        ? 'border-accent text-accent bg-accent/5' 
                        : 'border-transparent text-text-secondary hover:text-foreground hover:bg-surface'
                      }
                    `}
                  >
                    <tab.icon className="w-3.5 h-3.5" />
                    {tab.label}
                  </button>
                ))}
              </div>

              <div className="flex-1 overflow-auto p-4 custom-scrollbar bg-background">
                {loadingDetails ? (
                  <div className="flex flex-col items-center justify-center h-full gap-3 text-text-tertiary">
                    <div className="w-5 h-5 border-2 border-accent/30 border-t-accent rounded-full animate-spin" />
                    <span className="text-xs">Loading details...</span>
                  </div>
                ) : !requestDetails ? (
                  <div className="flex flex-col items-center justify-center h-full text-text-tertiary">
                    <FileText className="w-8 h-8 opacity-20 mb-2" />
                    <span className="text-xs">No details available</span>
                  </div>
                ) : (
                  <div className="space-y-4">
                    {activeTab === 'headers' && (
                      <div className="space-y-6">
                        <div>
                          <h4 className="text-xs font-bold text-text-secondary uppercase tracking-wider mb-2 flex items-center gap-2">
                            <ArrowDown className="w-3 h-3" /> Request Headers
                          </h4>
                          <div className="bg-surface rounded-md border border-border/50 divide-y divide-border/30">
                            {Object.entries(requestDetails.requestHeaders).map(([key, value]) => (
                              <div key={key} className="flex text-xs px-3 py-2">
                                <span className="text-text-secondary font-medium w-1/3 shrink-0 break-words">{key}</span>
                                <span className="text-text-tertiary font-mono break-all select-all">{value}</span>
                              </div>
                            ))}
                            {Object.keys(requestDetails.requestHeaders).length === 0 && (
                              <div className="px-3 py-2 text-xs text-text-tertiary italic">No request headers</div>
                            )}
                          </div>
                        </div>

                        <div>
                          <h4 className="text-xs font-bold text-text-secondary uppercase tracking-wider mb-2 flex items-center gap-2">
                            <Send className="w-3 h-3 rotate-180" /> Response Headers
                          </h4>
                          <div className="bg-surface rounded-md border border-border/50 divide-y divide-border/30">
                            {Object.entries(requestDetails.responseHeaders).map(([key, value]) => (
                              <div key={key} className="flex text-xs px-3 py-2">
                                <span className="text-text-secondary font-medium w-1/3 shrink-0 break-words">{key}</span>
                                <span className="text-text-tertiary font-mono break-all select-all">{value}</span>
                              </div>
                            ))}
                             {Object.keys(requestDetails.responseHeaders).length === 0 && (
                              <div className="px-3 py-2 text-xs text-text-tertiary italic">No response headers</div>
                            )}
                          </div>
                        </div>
                      </div>
                    )}

                    {activeTab === 'payload' && (
                      <div>
                        <div className="flex items-center justify-between mb-2">
                          <h4 className="text-xs font-bold text-text-secondary uppercase tracking-wider">Request Body</h4>
                          {requestDetails.requestBody && (
                            <span className="text-[10px] px-1.5 py-0.5 rounded bg-surface border border-border text-text-tertiary">
                              {formatSize(requestDetails.requestBody.length)}
                            </span>
                          )}
                        </div>
                        {requestDetails.requestBody ? (
                          <JsonViewer data={requestDetails.requestBody} />
                        ) : (
                          <div className="text-xs text-text-tertiary italic p-4 text-center border border-dashed border-border rounded-md">
                            No request body
                          </div>
                        )}
                      </div>
                    )}

                    {activeTab === 'response' && (
                      <div>
                        <div className="flex items-center justify-between mb-2">
                          <h4 className="text-xs font-bold text-text-secondary uppercase tracking-wider">Response Body</h4>
                          {requestDetails.responseBody && (
                            <span className="text-[10px] px-1.5 py-0.5 rounded bg-surface border border-border text-text-tertiary">
                              {formatSize(requestDetails.responseBody.length)}
                            </span>
                          )}
                        </div>
                        {requestDetails.responseBody ? (
                          requestDetails.base64Encoded ? (
                            <div className="space-y-2">
                              <div className="text-xs text-accent bg-accent/10 px-2 py-1 rounded inline-block">Base64 Encoded Binary</div>
                              {selectedRequest.type === 'Image' && (
                                <div className="border border-border rounded-lg p-2 bg-checkerboard flex items-center justify-center">
                                  <img 
                                    src={`data:${selectedRequest.mimeType || 'image/png'};base64,${requestDetails.responseBody}`} 
                                    className="max-w-full max-h-[300px] object-contain"
                                    alt="Response preview"
                                  />
                                </div>
                              )}
                              <pre className="text-xs font-mono whitespace-pre-wrap text-text-secondary overflow-auto max-h-[200px] p-2 bg-background/50 rounded border border-border/50 opacity-50">
                                {requestDetails.responseBody.slice(0, 500)}...
                              </pre>
                            </div>
                          ) : (
                            <JsonViewer data={requestDetails.responseBody} />
                          )
                        ) : (
                          <div className="text-xs text-text-tertiary italic p-4 text-center border border-dashed border-border rounded-md">
                            No response body available
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
