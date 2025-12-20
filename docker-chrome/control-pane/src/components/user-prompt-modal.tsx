"use client";

import React from "react";
import { Hand } from "lucide-react";

interface UserPromptBannerProps {
    isVisible: boolean;
    prompt: string;
}

export function UserPromptBanner({ isVisible, prompt }: UserPromptBannerProps) {
    if (!isVisible || !prompt) return null;

    return (
        <div className="w-full bg-accent/10 border border-accent/30 rounded-lg px-3 py-2 mb-2 flex items-center gap-2">
            <Hand size={14} className="text-accent flex-shrink-0" />
            <span className="text-sm text-foreground">{prompt}</span>
        </div>
    );
}
