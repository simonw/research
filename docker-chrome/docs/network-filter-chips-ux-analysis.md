# UX Analysis: Network Type Filter Chips

## Purpose of the Research
To analyze the user experience implications of adding network request type filter chips to a developer tool's network panel, based on patterns from Chrome DevTools and other inspectors.

## Key Findings/Notes

### Core Recommendations
1.  **Discoverability**: Place a horizontal filter bar prominently above the network table.
2.  **Default Sets**:
    - **Core**: All, XHR, JS, CSS, Img, Media, Other.
    - **Advanced**: Doc, Font, WS, Manifest (in dropdown).
3.  **Multi-Select**: Support additive multi-select with Cmd/Ctrl+click. "All" should be mutually exclusive.
4.  **Counts**: Show counts as badges only on active filters to reduce clutter and performance cost.
5.  **Keyboard Nav**: WCAG 2.1 AA compliant (Tab/Arrow nav, Space/Enter select).

### Implementation Pattern
```
[All] [XHR] [JS] [CSS] [Img] [Media] [Other ▼] [Text Filter]
```

### Persistence
Use LocalStorage with session fallback for privacy. Persist active selections and custom configs.

## Important Code Snippets/Structures

**Proposed Component Architecture:**
```
NetworkPanel
├── FilterBar
│   ├── FilterChip (reusable)
│   ├── FilterDropdown (advanced)
│   └── FilterSearch (text input)
├── NetworkTable (filtered)
└── NetworkDetails (filtered context)
```

**State Management Pattern:**
```typescript
interface FilterState {
  selectedTypes: string[];
  mode: 'single' | 'multi';
  showCounts: boolean;
  customFilters: FilterConfig[];
}
```

## Conclusion/Next Steps

### Implementation Plan
1.  **Phase 1 (MVP)**: Core filter bar, toggle behavior, visual states.
2.  **Phase 2**: Multi-select, count badges, keyboard nav, persistence.
3.  **Phase 3**: Advanced dropdown, dynamic type handling, optimizations.

**Primary Recommendation**: Follow Chrome DevTools patterns to ensure familiarity and usability.
