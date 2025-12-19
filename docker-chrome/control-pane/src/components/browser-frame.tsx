"use client";

import React, { useEffect, useRef, useState } from "react";
import { debounce } from "@/lib/utils";

interface BrowserFrameProps {
  url: string;
  apiBase?: string;
}

export function BrowserFrame({ url, apiBase }: BrowserFrameProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState<{ width: number; height: number } | null>(null);
  const lastSentRef = useRef<{ width: number; height: number } | null>(null);

  const baseUrl = apiBase || url;

  useEffect(() => {
    if (!containerRef.current) return;

    const handleResize = debounce((entries: ResizeObserverEntry[]) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;

        const clampedWidth = Math.max(320, Math.min(1920, Math.round(width)));
        const clampedHeight = Math.max(480, Math.min(1080, Math.round(height)));

        const last = lastSentRef.current;
        if (
          !last ||
          Math.abs(last.width - clampedWidth) > 16 ||
          Math.abs(last.height - clampedHeight) > 16
        ) {
          lastSentRef.current = { width: clampedWidth, height: clampedHeight };
          setDimensions({ width: clampedWidth, height: clampedHeight });

          fetch(`${baseUrl}/api/viewport`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ width: clampedWidth, height: clampedHeight }),
          }).catch(() => {});
        }
      }
    }, 300);

    const observer = new ResizeObserver(handleResize);
    observer.observe(containerRef.current);

    return () => {
      observer.disconnect();
    };
  }, [baseUrl]);

  return (
    <div
      ref={containerRef}
      className="relative w-full h-full bg-black rounded-lg overflow-hidden shadow-2xl ring-1 ring-white/10"
    >
      <iframe
        src={url}
        className="w-full h-full border-none bg-white"
        allow="cross-origin-isolated"
        referrerPolicy="no-referrer"
      />
      {dimensions && (
        <div className="absolute top-2 right-2 px-2 py-1 bg-black/50 backdrop-blur text-white text-xs rounded pointer-events-none font-mono z-50">
          {dimensions.width}x{dimensions.height}
        </div>
      )}
    </div>
  );
}
