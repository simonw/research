'use client';

import React from 'react';
import { Filter, Download } from 'lucide-react';
import { ResourceType } from '@/lib/types';

export type FilterGroup = 'Captured' | 'API' | 'Doc' | 'Assets' | 'Realtime' | 'Other';

export const FILTER_GROUPS: Record<FilterGroup, (ResourceType | '__captured__')[]> = {
  Captured: ['__captured__'],
  API: ['XHR', 'Fetch', 'Preflight'],
  Doc: ['Document'],
  Assets: ['Script', 'Stylesheet', 'Image', 'Font', 'Media'],
  Realtime: ['WebSocket', 'EventSource'],
  Other: ['Manifest', 'Prefetch', 'Other'],
};

interface NetworkFilterProps {
  selectedGroups: Set<FilterGroup>;
  onToggleGroup: (group: FilterGroup) => void;
  groupCounts?: Record<FilterGroup, number>;
}

export function NetworkFilter({ selectedGroups, onToggleGroup, groupCounts }: NetworkFilterProps) {
  const groups: FilterGroup[] = ['Captured', 'API', 'Doc', 'Assets', 'Realtime', 'Other'];

  return (
    <div className="flex items-center gap-2 overflow-x-auto no-scrollbar">
      <div className="flex items-center gap-1.5 text-text-tertiary mr-2">
        <Filter className="w-3.5 h-3.5" />
        <span className="text-xs font-medium">Filter:</span>
      </div>
      
      {groups.map((group) => {
        const isSelected = selectedGroups.has(group);
        const isCaptured = group === 'Captured';
        const count = groupCounts?.[group] ?? 0;

        return (
          <button
            key={group}
            onClick={() => onToggleGroup(group)}
            className={`
              flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-full border transition-all
              ${isSelected 
                ? isCaptured
                  ? 'bg-accent/5 border-accent text-accent'
                  : 'bg-surface border-foreground text-foreground'
                : 'bg-transparent border-border text-text-secondary hover:border-text-secondary hover:text-foreground'
              }
            `}
          >
            {isCaptured && <Download className="w-3 h-3" />}
            <span>{group}</span>
            {count > 0 && (
              <span className={`
                ml-0.5 text-[10px] px-1 rounded-full
                ${isSelected 
                  ? isCaptured ? 'bg-accent text-white' : 'bg-foreground text-background'
                  : 'bg-surface border border-border text-text-secondary'
                }
              `}>
                {count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
