"use client";

import React from "react";
import { Filter, ChevronDown, Check, Download } from "lucide-react";
import { ResourceType } from "@/lib/types";

export type FilterGroup = "API" | "Doc" | "Assets" | "Realtime" | "Other" | "Captured";

export const FILTER_GROUPS: Record<FilterGroup, ResourceType[] | "__captured__"[]> = {
  API: ["XHR", "Fetch", "Preflight"],
  Doc: ["Document"],
  Assets: ["Script", "Stylesheet", "Image", "Font", "Media"],
  Realtime: ["WebSocket", "EventSource"],
  Other: ["Manifest", "Prefetch", "Other"],
  Captured: ["__captured__"],
};

interface NetworkFilterProps {
  selectedGroups: Set<FilterGroup>;
  onToggleGroup: (group: FilterGroup) => void;
}

export function NetworkFilter({ selectedGroups, onToggleGroup }: NetworkFilterProps) {
  const groups: FilterGroup[] = ["Captured", "API", "Doc", "Assets", "Realtime", "Other"];
  const activeCount = selectedGroups.size;

  return (
    <details className="relative group z-10">
      <summary className="flex items-center gap-2.5 px-3.5 py-2 text-xs font-medium bg-surface border border-border rounded-lg cursor-pointer hover:border-border-emphasis hover:bg-background/50 transition-all duration-200 list-none select-none text-foreground shadow-sm">
        <Filter className="w-3.5 h-3.5 text-text-secondary" />
        <span className="font-semibold">
          {activeCount === 0 ? "Filter" : `${activeCount} Active`}
        </span>
        <ChevronDown className="w-3.5 h-3.5 text-text-secondary group-open:rotate-180 transition-transform duration-200" />
      </summary>

      {/* Backdrop to close dropdown */}
      <div
        className="fixed inset-0 z-[-1] hidden group-open:block"
        onClick={(e) => {
          const details = e.currentTarget.parentElement as HTMLDetailsElement;
          details.removeAttribute('open');
        }}
      />

      <div className="absolute right-0 top-full mt-2 w-48 p-1.5 bg-surface border border-border rounded-lg shadow-xl animate-in slide-in-from-top-2 duration-200">
        {/* Dropdown Header */}
        <div className="px-3 py-2 border-b border-border mb-1">
          <span className="text-xs font-semibold text-foreground uppercase tracking-wide">
            Filter by Type
          </span>
        </div>

        {/* Filter Options */}
        {groups.map((group) => {
          const isSelected = selectedGroups.has(group);
          const isCaptured = group === "Captured";
          return (
            <label
              key={group}
              className={`flex items-center gap-3 px-3 py-2.5 text-sm text-foreground hover:bg-background rounded-md cursor-pointer transition-all duration-150 group/item ${isCaptured ? "border-b border-border mb-1" : ""}`}
            >
              <div
                className={`w-4 h-4 rounded border-2 flex items-center justify-center flex-shrink-0 transition-all duration-200 ${
                  isSelected
                    ? "bg-accent border-accent shadow-sm"
                    : "border-border bg-surface group-hover/item:border-border-emphasis"
                }`}
              >
                {isSelected && <Check className="w-3 h-3 text-white" strokeWidth={3} />}
              </div>
              <input
                type="checkbox"
                className="hidden"
                checked={isSelected}
                onChange={() => onToggleGroup(group)}
              />
              {isCaptured && <Download className="w-3.5 h-3.5 text-accent" />}
              <span className={`font-medium ${isCaptured ? "text-accent" : ""}`}>{group}</span>
              {!isCaptured && (
                <span className="ml-auto text-xs text-text-tertiary font-mono">
                  {FILTER_GROUPS[group].length}
                </span>
              )}
            </label>
          );
        })}

        {/* Footer Info */}
        {activeCount > 0 && (
          <div className="mt-1 pt-1.5 border-t border-border px-3 py-2">
            <button
              onClick={(e) => {
                e.preventDefault();
                groups.forEach((group) => {
                  if (selectedGroups.has(group)) {
                    onToggleGroup(group);
                  }
                });
              }}
              className="text-xs text-error hover:text-error/80 font-medium transition-colors"
            >
              Clear all filters
            </button>
          </div>
        )}
      </div>
    </details>
  );
}
