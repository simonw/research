import React from "react";
import { Filter, Check, ChevronDown } from "lucide-react";
import { ResourceType } from "@/lib/types";

export type FilterGroup = "API" | "Doc" | "Assets" | "Realtime" | "Other";

export const FILTER_GROUPS: Record<FilterGroup, ResourceType[]> = {
  API: ["XHR", "Fetch", "Preflight"],
  Doc: ["Document"],
  Assets: ["Script", "Stylesheet", "Image", "Font", "Media"],
  Realtime: ["WebSocket", "EventSource"],
  Other: ["Manifest", "Prefetch", "Other"],
};

interface NetworkFilterProps {
  selectedGroups: Set<FilterGroup>;
  onToggleGroup: (group: FilterGroup) => void;
}

export function NetworkFilter({
  selectedGroups,
  onToggleGroup,
}: NetworkFilterProps) {
  const groups = Object.keys(FILTER_GROUPS) as FilterGroup[];
  const activeCount = selectedGroups.size;

  return (
    <details className="relative group z-10">
      <summary className="flex items-center gap-2 px-2 py-1 text-xs font-medium bg-zinc-800 border border-zinc-700 rounded cursor-pointer hover:bg-zinc-700 hover:border-zinc-600 transition-colors list-none select-none text-zinc-300">
        <Filter className="w-3 h-3" />
        <span>{activeCount === 0 ? "Filter" : `${activeCount} active`}</span>
        <ChevronDown className="w-3 h-3 text-zinc-500 group-open:rotate-180 transition-transform" />
      </summary>

      <div className="fixed inset-0 z-[-1] hidden group-open:block" onClick={(e) => {
          const details = e.currentTarget.parentElement as HTMLDetailsElement;
          details.removeAttribute('open');
      }} />

      <div className="absolute right-0 top-full mt-1 w-40 p-1 bg-zinc-900 border border-zinc-700 rounded-lg shadow-xl backdrop-blur-sm">
        {groups.map((group) => {
          const isSelected = selectedGroups.has(group);
          return (
            <label
              key={group}
              className="flex items-center gap-2 px-2 py-1.5 text-xs text-zinc-300 hover:bg-zinc-800 rounded cursor-pointer transition-colors"
            >
              <div
                className={`w-3.5 h-3.5 rounded border flex items-center justify-center transition-colors ${
                  isSelected
                    ? "bg-blue-600 border-blue-600"
                    : "border-zinc-600 bg-zinc-800"
                }`}
              >
                {isSelected && <Check className="w-2.5 h-2.5 text-white" />}
              </div>
              <input
                type="checkbox"
                className="hidden"
                checked={isSelected}
                onChange={() => onToggleGroup(group)}
              />
              {group}
            </label>
          );
        })}
      </div>
    </details>
  );
}
