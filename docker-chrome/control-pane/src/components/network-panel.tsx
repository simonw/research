import React, { useRef, useEffect } from "react";
import { NetworkRequest } from "@/lib/types";
import { ArrowDown, ArrowUp } from "lucide-react";

interface NetworkPanelProps {
  requests: NetworkRequest[];
}

export function NetworkPanel({ requests }: NetworkPanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [requests]);

  return (
    <div className="flex flex-col h-full bg-surface border border-border rounded-lg overflow-hidden">
      <div className="px-4 py-2 border-b border-border bg-surface flex items-center justify-between">
        <h2 className="text-sm font-medium text-foreground">Network Activity</h2>
        <span className="text-xs text-zinc-500">{requests.length} requests</span>
      </div>
      
      <div className="grid grid-cols-12 gap-2 px-4 py-2 text-xs font-medium text-zinc-500 border-b border-border bg-zinc-900/50">
        <div className="col-span-1">Stat</div>
        <div className="col-span-1">Meth</div>
        <div className="col-span-7">URL</div>
        <div className="col-span-2">Type</div>
        <div className="col-span-1">Time</div>
      </div>

      <div ref={scrollRef} className="flex-1 overflow-y-auto min-h-[200px] max-h-[400px]">
        {requests.length === 0 ? (
          <div className="flex items-center justify-center h-full text-zinc-600 text-sm">
            No network activity recorded
          </div>
        ) : (
          requests.map((req, i) => (
            <div 
              key={`${req.requestId}-${i}`}
              className="grid grid-cols-12 gap-2 px-4 py-2 text-xs border-b border-border/50 hover:bg-zinc-800/50 transition-colors"
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
          ))
        )}
      </div>
    </div>
  );
}
