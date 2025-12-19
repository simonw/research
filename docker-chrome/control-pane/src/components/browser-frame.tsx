"use client";

import React from "react";

interface BrowserFrameProps {
  url: string;
  apiBase?: string;
}

export function BrowserFrame({ url }: BrowserFrameProps) {
  return (
    <div className="w-full h-full flex items-center justify-center">
      <div className="aspect-[430/932] max-h-[600px] bg-surface border border-border rounded-lg overflow-hidden shadow-lg">
        <iframe
          src={url}
          className="w-full h-full border-none bg-white"
          allow="cross-origin-isolated"
          referrerPolicy="no-referrer"
          title="Browser content"
        />
      </div>
    </div>
  );
}
