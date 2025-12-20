"use client";

import React from "react";
import { AutomationOverlay } from "./automation-overlay";
import { UserPromptBanner } from "./user-prompt-modal";

interface BrowserFrameProps {
  url: string;
  apiBase?: string;
  automationMode?: "idle" | "automation" | "user-input";
  automationPrompt?: string;
  cursorPosition?: { x: number; y: number } | null;
  cursorAction?: "move" | "click";
}

export function BrowserFrame({
  url,
  automationMode = "idle",
  automationPrompt = "",
  cursorPosition = null,
  cursorAction = "move",
}: BrowserFrameProps) {
  return (
    <div className="w-full h-full flex flex-col items-center justify-start">
      {/* Container matching browser width */}
      <div className="w-full max-w-[280px]">
        {/* User prompt banner - above the browser */}
        <UserPromptBanner
          isVisible={automationMode === "user-input"}
          prompt={automationPrompt}
        />

        {/* Browser frame */}
        <div className="relative aspect-[430/932] bg-surface border border-border rounded-lg overflow-hidden shadow-lg">
          {/* Browser content iframe */}
          <iframe
            src={url}
            className="w-full h-full border-none bg-white"
            allow="cross-origin-isolated"
            referrerPolicy="no-referrer"
            title="Browser content"
          />

          {/* Automation overlay - only shown in automation mode */}
          <AutomationOverlay
            isActive={automationMode === "automation"}
            cursorPosition={cursorPosition}
            cursorAction={cursorAction}
          />
        </div>
      </div>
    </div>
  );
}
