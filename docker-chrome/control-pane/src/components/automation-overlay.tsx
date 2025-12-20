"use client";

import React from "react";

interface AutomationOverlayProps {
    isActive: boolean;
    cursorPosition: { x: number; y: number } | null;
    cursorAction?: "move" | "click";
}

export function AutomationOverlay({
    isActive,
    cursorPosition,
    cursorAction = "move",
}: AutomationOverlayProps) {
    if (!isActive) return null;

    return (
        <div className="absolute inset-0 z-50 pointer-events-auto">
            {/* Semi-transparent overlay */}
            <div className="absolute inset-0 bg-black/20 backdrop-blur-[1px]" />

            {/* Running indicator */}
            <div className="absolute top-3 left-1/2 -translate-x-1/2 flex items-center gap-2 bg-surface/90 backdrop-blur border border-border rounded-full px-3 py-1.5 shadow-lg">
                <div className="w-2 h-2 rounded-full bg-accent animate-pulse" />
                <span className="text-xs font-medium text-foreground">
                    Automation Running
                </span>
            </div>

            {/* Animated cursor */}
            {cursorPosition && (
                <div
                    className="absolute pointer-events-none transition-all duration-75 ease-out"
                    style={{
                        left: cursorPosition.x,
                        top: cursorPosition.y,
                        transform: "translate(-50%, -50%)",
                    }}
                >
                    {/* Cursor dot */}
                    <div
                        className={`w-4 h-4 rounded-full border-2 border-white shadow-lg transition-all duration-100 ${cursorAction === "click"
                                ? "bg-accent scale-150"
                                : "bg-accent/60"
                            }`}
                    />
                    {/* Click ripple effect */}
                    {cursorAction === "click" && (
                        <div className="absolute inset-0 w-4 h-4 rounded-full border-2 border-accent animate-ping" />
                    )}
                </div>
            )}
        </div>
    );
}
