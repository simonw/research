'use client';

import React, { useEffect, useRef, useState } from 'react';
import { debounce } from '@/lib/utils';

interface BrowserFrameProps {
    nekoUrl: string;
    cdpAgentUrl: string;
    onViewportChange?: (width: number, height: number) => void;
}

export function BrowserFrame({ nekoUrl, cdpAgentUrl, onViewportChange }: BrowserFrameProps) {
    const containerRef = useRef<HTMLDivElement>(null);
    const [dimensions, setDimensions] = useState<{ width: number; height: number } | null>(null);
    const lastSentRef = useRef<{ width: number; height: number } | null>(null);

    useEffect(() => {
        if (!containerRef.current) return;

        const handleResize = debounce((entries: ResizeObserverEntry[]) => {
            for (const entry of entries) {
                const { width, height } = entry.contentRect;

                // Clamp dimensions
                const clampedWidth = Math.max(320, Math.min(3840, Math.round(width)));
                const clampedHeight = Math.max(240, Math.min(2160, Math.round(height)));

                const last = lastSentRef.current;
                // Only update if changed by more than 16 pixels to avoid excessive updates
                if (
                    !last ||
                    Math.abs(last.width - clampedWidth) > 16 ||
                    Math.abs(last.height - clampedHeight) > 16
                ) {
                    lastSentRef.current = { width: clampedWidth, height: clampedHeight };
                    setDimensions({ width: clampedWidth, height: clampedHeight });

                    // Sync viewport to CDP agent
                    fetch(`${cdpAgentUrl}/api/viewport`, {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ width: clampedWidth, height: clampedHeight }),
                    }).catch(() => { });

                    if (onViewportChange) {
                        onViewportChange(clampedWidth, clampedHeight);
                    }
                }
            }
        }, 300);

        const observer = new ResizeObserver(handleResize);
        observer.observe(containerRef.current);

        return () => {
            observer.disconnect();
        };
    }, [cdpAgentUrl, onViewportChange]);

    return (
        <div
            ref={containerRef}
            className="relative w-full h-full bg-black rounded-lg overflow-hidden shadow-2xl ring-1 ring-white/10"
        >
            {nekoUrl ? (
                <iframe
                    src={nekoUrl}
                    className="w-full h-full border-none bg-white"
                    allow="autoplay; fullscreen; microphone; camera"
                    referrerPolicy="no-referrer"
                    title="Neko Browser"
                />
            ) : (
                <div className="absolute inset-0 flex items-center justify-center">
                    <div className="text-center">
                        <div className="w-12 h-12 mx-auto mb-4 rounded-full bg-zinc-800 flex items-center justify-center">
                            <svg className="w-6 h-6 text-zinc-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
                            </svg>
                        </div>
                        <p className="text-zinc-500 text-sm">No stream connected</p>
                        <p className="text-zinc-600 text-xs mt-1">Start a session or enter a neko URL</p>
                    </div>
                </div>
            )}

            {/* Viewport indicator */}
            {dimensions && (
                <div className="absolute top-2 right-2 px-2 py-1 bg-black/50 backdrop-blur text-white text-xs rounded pointer-events-none font-mono z-50">
                    {dimensions.width}x{dimensions.height}
                </div>
            )}
        </div>
    );
}
