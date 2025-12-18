# Network Type Filter Chips UX Analysis

## Research Context

Based on analysis of existing codebase and CDP network capture research:

### Current Implementation Status
- Network capture working via CDP (Chrome DevTools Protocol)
- 19 possible resource types available from CDP ResourceType enum
- Current UI shows network requests but lacks type-based filtering
- Type field not currently being captured from CDP events

### Available Resource Types (from CDP)
- Document, Stylesheet, Image, Media, Font, Script
- XHR, Fetch, Prefetch, EventSource, WebSocket
- Manifest, SignedExchange, Ping, CSPViolationReport, Preflight, FedCM
- Other (catch-all)

### Existing Filter Research
- Chrome DevTools uses: All, XHR, JS, CSS, Img, Media, Font, Doc, WS, Other
- Button-based multi-select with visual active states
- Text filter box for advanced queries
- State management patterns documented

## UX Analysis Framework

Following deep analysis mode structure:
- Phase 1: Gather context (completed)
- Phase 2: Expert consultation (in progress)
- Synthesis: Cross-reference findings

## Key UX Dimensions to Analyze

1. **Discoverability** - How users find and understand the filter chips
2. **Default Sets** - Which types to show by default vs advanced
3. **Multi-select Behavior** - How selection works (single vs multi-select modes)
4. **Counts** - Whether to show request counts on chips
5. **Keyboard Navigation** - Accessibility and power user support
6. **Persistence** - How filter state is saved/restored
7. **Unknown/Other Types** - Handling edge cases and new types
8. **Edge Cases & Mitigation** - Error states, performance, mobile UX

## Initial Findings

### Current Network Panel UX
- Table-based display with columns: Status, Method, URL, Type, Time
- Click to select request and view details drawer
- No filtering capabilities currently implemented
- Type column shows resource type but no filtering UI

### Chrome DevTools Patterns
- Horizontal filter bar with button chips
- Multi-select with Cmd/Ctrl+click
- Visual active/inactive states (filled vs outline)
- 'All' button to show everything
- Text filter for advanced queries

### Common Resource Type Categories
1. **Core Content**: Document, Stylesheet, Script, Image
2. **Media**: Media (video/audio), Font
3. **API**: XHR, Fetch
4. **Real-time**: WebSocket, EventSource
5. **Meta**: Manifest, Other

## Next Steps

1. Analyze each UX dimension in detail
2. Research similar patterns in other tools
3. Create implementation recommendations
4. Document edge cases and solutions
## Phase 1 Complete - Context Gathered

### Key Findings from Research

#### Chrome DevTools Network Filter Patterns
- **Horizontal button bar** with filter chips: All, XHR, JS, CSS, Img, Media, Font, Doc, WS, Other
- **Multi-select behavior**: Cmd/Ctrl + click for multiple selection
- **Visual states**: Filled background for active, outline for inactive
- **Text filter box** for advanced queries (domain:, mime-type:, status-code:, etc.)
- **Invert checkbox** for negative filtering
- **More filters dropdown** for additional options

#### Browser Comparison
- **Chrome/Edge**: Button chips + text filter + invert option
- **Firefox**: Text-based filtering with autocomplete (status-code:, mime-type:, etc.)
- **Safari**: Limited filtering options, mostly text-based

#### Material Design Chip Patterns
- **Filter chips**: Rounded buttons with optional icons
- **Selection states**: Selected (filled), unselected (outlined)
- **Multi-select**: Supported via multiple chip selection
- **Input chips**: For dynamic filter addition/removal

#### Common UX Patterns
- **Progressive disclosure**: Basic filters visible, advanced in dropdown/expand
- **Visual feedback**: Clear active/inactive states
- **Keyboard navigation**: Tab through chips, Space/Enter to select
- **Counts**: Some implementations show request counts on chips
- **Clear all**: Easy way to reset filters

### Current Implementation Gaps
- NetworkPanel shows 'type' column but no filtering UI
- Server captures CDP events but doesn't send 'type' field to frontend
- No filter state management or persistence
- No keyboard navigation or accessibility features

### CDP Resource Types Available
From protocol research: Document, Stylesheet, Image, Media, Font, Script, XHR, Fetch, Prefetch, EventSource, WebSocket, Manifest, SignedExchange, Ping, CSPViolationReport, Preflight, FedCM, Other

## Moving to Phase 2 - Expert Consultation

## Phase 2 Complete - Expert Synthesis

### Cross-Referenced Findings

#### Consensus Patterns
- **Horizontal filter bar**: All major DevTools use this pattern
- **Multi-select with modifiers**: Cmd/Ctrl+click is standard
- **Core + Advanced split**: 7-8 core filters, additional in dropdown
- **Visual states**: Filled active, outlined inactive
- **Chrome DevTools influence**: Most tools follow their patterns

#### Contradictions Resolved
- **Counts**: Show on active filters only (performance + clarity)
- **Persistence**: LocalStorage with privacy considerations
- **Unknown types**: Dynamic generation under "Other" category
- **Keyboard nav**: Full WCAG compliance with power-user shortcuts

### Key UX Principles Established

1. **Discoverability First**: Filters must be immediately visible and actionable
2. **Progressive Complexity**: Start simple, allow advanced usage patterns
3. **Performance Conscious**: Don't compromise speed for features
4. **Accessibility Core**: WCAG compliance from day one
5. **Chrome DevTools Compatibility**: Match expectations from most-used tool

### Implementation Confidence: High
- Strong precedent in Chrome DevTools
- Clear technical path using existing CDP infrastructure  
- Modular approach allows incremental delivery
- Comprehensive edge case analysis completed

## Final Recommendations

**Primary Pattern**: Chrome DevTools-style filter bar with core chip set, multi-select behavior, and progressive disclosure.

**Key Differentiators**:
- Count badges on active filters only
- Full keyboard accessibility
- Smart handling of unknown resource types
- Privacy-conscious persistence

**Success Factors**:
- Match user expectations from Chrome DevTools
- Maintain performance with large request sets
- Provide clear visual feedback
- Support both novice and expert users

Analysis complete. Ready for implementation following the staged rollout plan.
