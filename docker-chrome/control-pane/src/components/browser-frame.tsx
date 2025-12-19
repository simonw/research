"use client";

import React, { useRef } from "react";

interface BrowserFrameProps {
  url: string;
  apiBase?: string;
}

export function BrowserFrame({ url }: BrowserFrameProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  return (
    <div className="relative w-full max-w-[280px] mx-auto">
      {/* Device Shadow */}
      <div className="absolute inset-0 bg-gradient-to-b from-black/10 to-black/30 blur-2xl translate-y-4 scale-95 -z-10" />

      {/* Phone Frame Container */}
      <div
        ref={containerRef}
        className="relative aspect-[430/932] bg-gradient-to-br from-gray-800 via-gray-900 to-black rounded-[2.5rem] p-2 shadow-2xl"
      >
        {/* Notch Area */}
        <div className="absolute top-0 left-1/2 -translate-x-1/2 w-40 h-7 bg-black rounded-b-3xl z-20" />

        {/* Inner Bezel - Creates depth */}
        <div className="absolute inset-2 bg-gradient-to-br from-gray-700 via-gray-800 to-gray-900 rounded-[2.25rem] p-0.5">
          {/* Screen Container */}
          <div className="relative w-full h-full bg-black rounded-[2rem] overflow-hidden">
            {/* Status Bar Overlay - subtle transparency */}
            <div className="absolute top-0 left-0 right-0 h-12 bg-gradient-to-b from-black/40 to-transparent z-10 pointer-events-none" />

            {/* Iframe Content */}
            <iframe
              src={url}
              className="w-full h-full border-none bg-white"
              allow="cross-origin-isolated"
              referrerPolicy="no-referrer"
              title="Browser content"
            />

            {/* Bottom Gesture Bar - Home indicator */}
            <div className="absolute bottom-2 left-1/2 -translate-x-1/2 w-32 h-1 bg-white/30 rounded-full z-10 pointer-events-none" />
          </div>
        </div>

        {/* Side Buttons - Volume & Power */}
        <div className="absolute -left-1 top-24 w-1 h-8 bg-gray-700 rounded-l-sm" />
        <div className="absolute -left-1 top-36 w-1 h-12 bg-gray-700 rounded-l-sm" />
        <div className="absolute -left-1 top-52 w-1 h-12 bg-gray-700 rounded-l-sm" />
        <div className="absolute -right-1 top-32 w-1 h-16 bg-gray-700 rounded-r-sm" />

        {/* Subtle Highlight - Adds realism */}
        <div className="absolute inset-2 rounded-[2.25rem] bg-gradient-to-tr from-white/0 via-white/5 to-white/10 pointer-events-none" />
      </div>
    </div>
  );
}
