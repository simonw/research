'use client';

import React from 'react';
import { X, Shield } from 'lucide-react';

interface FlowHeaderProps {
  providerName: string;
  onClose: () => void;
}

export function FlowHeader({ providerName, onClose }: FlowHeaderProps) {
  return (
    <div className="px-6 py-4 border-b border-border flex items-center justify-between bg-surface/50 backdrop-blur-sm sticky top-0 z-10">
      <div className="flex items-center gap-2">
        <div className="w-6 h-6 rounded-md bg-accent/10 flex items-center justify-center">
          <Shield size={14} className="text-accent" />
        </div>
        <span className="text-sm font-semibold text-foreground">
          Connect {providerName}
        </span>
      </div>
      <button 
        onClick={onClose}
        className="p-1.5 text-text-tertiary hover:text-foreground hover:bg-background rounded-lg transition-all"
      >
        <X size={20} />
      </button>
    </div>
  );
}
