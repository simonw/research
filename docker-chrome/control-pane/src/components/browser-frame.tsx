"use client";

import React, { useRef } from "react";

interface BrowserFrameProps {
  url: string;
  apiBase?: string;
}

export function BrowserFrame({ url }: BrowserFrameProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  return (
    <div
      ref={containerRef}
      className="relative h-full aspect-[430/932] mx-auto bg-black rounded-lg overflow-hidden shadow-2xl ring-1 ring-white/10"
    >
      <iframe
        src={url}
        className="w-full h-full border-none bg-white"
        allow="cross-origin-isolated"
        referrerPolicy="no-referrer"
      />
    </div>
  );
}
