"use client";

import React, { useState } from "react";
import { Database, ChevronDown, ChevronRight, Copy, Download } from "lucide-react";

interface DataPanelProps {
    data: Record<string, any>;
    isExpanded?: boolean;
}

function JsonValue({ value, depth = 0 }: { value: any; depth?: number }) {
    const [isOpen, setIsOpen] = useState(depth < 2);

    if (value === null) return <span className="text-text-tertiary">null</span>;
    if (value === undefined) return <span className="text-text-tertiary">undefined</span>;

    if (typeof value === "string") {
        return <span className="text-green-400">&quot;{value}&quot;</span>;
    }

    if (typeof value === "number") {
        return <span className="text-blue-400">{value}</span>;
    }

    if (typeof value === "boolean") {
        return <span className="text-purple-400">{value.toString()}</span>;
    }

    if (Array.isArray(value)) {
        if (value.length === 0) return <span className="text-text-tertiary">[]</span>;

        return (
            <div>
                <button
                    onClick={() => setIsOpen(!isOpen)}
                    className="inline-flex items-center gap-1 text-text-secondary hover:text-foreground"
                >
                    {isOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                    <span className="text-text-tertiary">[{value.length}]</span>
                </button>
                {isOpen && (
                    <div className="ml-4 border-l border-border pl-2">
                        {value.map((item, idx) => (
                            <div key={idx} className="py-0.5">
                                <span className="text-text-tertiary mr-2">{idx}:</span>
                                <JsonValue value={item} depth={depth + 1} />
                            </div>
                        ))}
                    </div>
                )}
            </div>
        );
    }

    if (typeof value === "object") {
        const keys = Object.keys(value);
        if (keys.length === 0) return <span className="text-text-tertiary">{"{}"}</span>;

        return (
            <div>
                <button
                    onClick={() => setIsOpen(!isOpen)}
                    className="inline-flex items-center gap-1 text-text-secondary hover:text-foreground"
                >
                    {isOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                    <span className="text-text-tertiary">{"{"}...{"}"}</span>
                </button>
                {isOpen && (
                    <div className="ml-4 border-l border-border pl-2">
                        {keys.map((key) => (
                            <div key={key} className="py-0.5">
                                <span className="text-accent">{key}</span>
                                <span className="text-text-tertiary">: </span>
                                <JsonValue value={value[key]} depth={depth + 1} />
                            </div>
                        ))}
                    </div>
                )}
            </div>
        );
    }

    return <span>{String(value)}</span>;
}

export function DataPanel({ data, isExpanded = true }: DataPanelProps) {
    const [expanded, setExpanded] = useState(isExpanded);
    const hasData = Object.keys(data).length > 0;

    const handleCopy = () => {
        navigator.clipboard.writeText(JSON.stringify(data, null, 2));
    };

    const handleDownload = () => {
        const blob = new Blob([JSON.stringify(data, null, 2)], {
            type: "application/json",
        });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `automation-data-${Date.now()}.json`;
        a.click();
        URL.revokeObjectURL(url);
    };

    return (
        <div className="bg-surface border border-border rounded-lg overflow-hidden">
            {/* Header */}
            <button
                onClick={() => setExpanded(!expanded)}
                className="w-full flex items-center justify-between px-4 py-3 hover:bg-background/50 transition-colors"
            >
                <div className="flex items-center gap-2">
                    <Database size={16} className="text-text-secondary" />
                    <span className="text-sm font-semibold text-foreground">
                        Collected Data
                    </span>
                    <span className="text-xs text-text-tertiary">
                        ({Object.keys(data).length} keys)
                    </span>
                </div>
                <div className="flex items-center gap-2">
                    {hasData && (
                        <>
                            <button
                                onClick={(e) => {
                                    e.stopPropagation();
                                    handleCopy();
                                }}
                                className="p-1.5 hover:bg-background rounded transition-colors"
                                title="Copy JSON"
                            >
                                <Copy size={14} className="text-text-secondary" />
                            </button>
                            <button
                                onClick={(e) => {
                                    e.stopPropagation();
                                    handleDownload();
                                }}
                                className="p-1.5 hover:bg-background rounded transition-colors"
                                title="Download JSON"
                            >
                                <Download size={14} className="text-text-secondary" />
                            </button>
                        </>
                    )}
                    {expanded ? (
                        <ChevronDown size={16} className="text-text-secondary" />
                    ) : (
                        <ChevronRight size={16} className="text-text-secondary" />
                    )}
                </div>
            </button>

            {/* Content */}
            {expanded && (
                <div className="border-t border-border px-4 py-3 max-h-64 overflow-auto">
                    {hasData ? (
                        <div className="font-mono text-xs">
                            {Object.keys(data).map((key) => (
                                <div key={key} className="py-0.5">
                                    <span className="text-accent">{key}</span>
                                    <span className="text-text-tertiary">: </span>
                                    <JsonValue value={data[key]} />
                                </div>
                            ))}
                        </div>
                    ) : (
                        <p className="text-sm text-text-tertiary text-center py-4">
                            No data collected yet
                        </p>
                    )}
                </div>
            )}
        </div>
    );
}
