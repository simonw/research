'use client';

import React, { useState } from 'react';
import { Database, ChevronDown, ChevronRight, Copy, Download } from 'lucide-react';

interface DataPanelProps {
  data: Record<string, unknown>;
  isExpanded?: boolean;
}

const JsonValue = ({ value, depth = 0, name }: { value: unknown; depth?: number; name?: string }) => {
  const [isExpanded, setIsExpanded] = useState(depth < 2);

  if (value === null || value === undefined) {
    return (
      <div className="flex items-start">
        {name && <span className="text-accent mr-2">{name}:</span>}
        <span className="text-text-tertiary">{String(value)}</span>
      </div>
    );
  }

  if (typeof value === 'boolean') {
    return (
      <div className="flex items-start">
        {name && <span className="text-accent mr-2">{name}:</span>}
        <span className="text-purple-400">{value.toString()}</span>
      </div>
    );
  }

  if (typeof value === 'number') {
    return (
      <div className="flex items-start">
        {name && <span className="text-accent mr-2">{name}:</span>}
        <span className="text-blue-400">{value}</span>
      </div>
    );
  }

  if (typeof value === 'string') {
    return (
      <div className="flex items-start">
        {name && <span className="text-accent mr-2">{name}:</span>}
        <span className="text-green-400">"{value}"</span>
      </div>
    );
  }

  if (Array.isArray(value)) {
    const isEmpty = value.length === 0;
    
    if (isEmpty) {
      return (
        <div className="flex items-start">
          {name && <span className="text-accent mr-2">{name}:</span>}
          <span className="text-text-secondary">[]</span>
        </div>
      );
    }

    return (
      <div className="flex flex-col">
        <div 
          className="flex items-center cursor-pointer hover:bg-background rounded px-1 -ml-1"
          onClick={(e) => {
            e.stopPropagation();
            setIsExpanded(!isExpanded);
          }}
        >
          <span className="text-text-tertiary mr-1">
            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </span>
          {name && <span className="text-accent mr-2">{name}:</span>}
          <span className="text-text-secondary">
            {isExpanded ? '[' : `[${value.length}]`}
          </span>
        </div>
        
        {isExpanded && (
          <div className="pl-6 border-l border-border ml-2">
            {value.map((item, index) => (
              <div key={index} className="my-1">
                <JsonValue value={item} depth={depth + 1} />
              </div>
            ))}
            <span className="text-text-secondary">]</span>
          </div>
        )}
      </div>
    );
  }

  if (typeof value === 'object') {
    const keys = Object.keys(value as Record<string, unknown>);
    const isEmpty = keys.length === 0;

    if (isEmpty) {
      return (
        <div className="flex items-start">
          {name && <span className="text-accent mr-2">{name}:</span>}
          <span className="text-text-secondary">{'{}'}</span>
        </div>
      );
    }

    return (
      <div className="flex flex-col">
        <div 
          className="flex items-center cursor-pointer hover:bg-background rounded px-1 -ml-1"
          onClick={(e) => {
            e.stopPropagation();
            setIsExpanded(!isExpanded);
          }}
        >
          <span className="text-text-tertiary mr-1">
            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </span>
          {name && <span className="text-accent mr-2">{name}:</span>}
          <span className="text-text-secondary">
            {isExpanded ? '{' : '{...}'}
          </span>
        </div>

        {isExpanded && (
          <div className="pl-6 border-l border-border ml-2">
            {keys.map((key) => (
              <div key={key} className="my-1">
                <JsonValue 
                  name={key} 
                  value={(value as Record<string, unknown>)[key]} 
                  depth={depth + 1} 
                />
              </div>
            ))}
            <span className="text-text-secondary">{'}'}</span>
          </div>
        )}
      </div>
    );
  }

  return null;
};

export function DataPanel({ data, isExpanded = true }: DataPanelProps) {
  const [expanded, setExpanded] = useState(isExpanded);
  const keyCount = data ? Object.keys(data).length : 0;
  const hasData = keyCount > 0;

  const handleCopy = () => {
    if (!data) return;
    navigator.clipboard.writeText(JSON.stringify(data, null, 2));
  };

  const handleDownload = () => {
    if (!data) return;
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `automation-data-${Date.now()}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  return (
    <div className="bg-surface border border-border rounded-lg overflow-hidden flex flex-col w-full">
      <div 
        className="flex items-center justify-between p-4 bg-surface border-b border-border cursor-pointer hover:bg-background transition-colors"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          <Database className="w-4 h-4 text-accent" />
          <span className="font-medium text-foreground">Collected Data</span>
          <span className="text-sm text-text-tertiary">({keyCount})</span>
        </div>
        <div className="flex items-center gap-2">
          {hasData && (
            <div className="flex items-center gap-1 mr-2" onClick={(e) => e.stopPropagation()}>
              <button 
                onClick={handleCopy}
                className="p-1.5 text-text-secondary hover:text-foreground hover:bg-background rounded-md transition-colors"
                title="Copy JSON"
              >
                <Copy className="w-4 h-4" />
              </button>
              <button 
                onClick={handleDownload}
                className="p-1.5 text-text-secondary hover:text-foreground hover:bg-background rounded-md transition-colors"
                title="Download JSON"
              >
                <Download className="w-4 h-4" />
              </button>
            </div>
          )}
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-text-tertiary" />
          ) : (
            <ChevronRight className="w-4 h-4 text-text-tertiary" />
          )}
        </div>
      </div>

      {expanded && (
        <div className="p-4 bg-surface overflow-x-auto">
          {!hasData ? (
            <div className="text-text-tertiary text-sm italic py-2">
              No data collected yet
            </div>
          ) : (
            <div className="font-mono text-sm">
              <JsonValue value={data} depth={0} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
