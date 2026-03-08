# tmuxcc + llmfit UX Investigation Notes

**Date**: 2026-03-08

## Objective
- Clone and explore tmuxcc and llmfit repos
- Break down architecture and testing infrastructure
- Identify UX patterns from llmfit that could apply to tmuxcc
- Attempt implementation
- Compile findings into a markdown report

## Repos
- tmuxcc: https://github.com/nyanko3141592/tmuxcc
- llmfit: https://github.com/AlexsJones/llmfit

---

## Progress Log

### Step 1: Cloning repos

Both repos cloned successfully. Both are pure Rust TUI apps using ratatui + crossterm.

---

## Architecture Summary

### tmuxcc
- **Stack**: Rust, ratatui 0.29, crossterm 0.28, tokio (async), serde/toml, sysinfo
- **Purpose**: Dashboard for managing AI coding assistants (Claude Code, OpenCode, Codex, Gemini) running in tmux panes
- **Architecture**: Async monitor loop → parse tmux pane content → update UI state → ratatui render
- **Key modules**:
  - `agents/` — types (MonitoredAgent, AgentStatus, ApprovalType), subagents
  - `app/` — AppState (UI state machine), Action enum, Config
  - `tmux/` — TmuxClient (subprocess calls), PaneInfo (process tree cache)
  - `parsers/` — Trait-based per-agent parser (Claude Code most sophisticated: regex for approvals, subagents, context %)
  - `monitor/` — Background tokio task, hysteresis logic, SystemStats
  - `ui/` — ratatui widgets: Header, AgentTree, PanePreview, Input, SubagentLog, Help, Footer
- **Testing**: Unit tests in each module file (`#[cfg(test)]`), dev dep: tempfile. No integration tests.
- **Layout**: Header (3h) | [AgentTree sidebar | PanePreview] | Footer (1h)
- **Colors**: Basic styling, not semantically rich

### llmfit
- **Stack**: Rust workspace (3 crates), ratatui 0.30, crossterm 0.29, tokio, axum (REST API), sysinfo, serde
- **Purpose**: Match LLM models to hardware — detect GPU/RAM, score 2000+ models, recommend downloads
- **Architecture**: Workspace: llmfit-core (algorithms) + llmfit-tui (TUI + REST API + CLI) + llmfit-desktop (Tauri)
- **Key UX patterns**:
  1. **System bar**: Always-visible line showing CPU, RAM, GPU, provider status
  2. **Semantic color coding**: Fit levels (green=Perfect, yellow=Good, magenta=Marginal, red=TooTight) — consistent throughout
  3. **Rich status indicators**: ● dot colored by level, ✓ for installed, spinner for downloads
  4. **Filter bar**: 8 live-update filters (search, provider, use-case, capability, fit, availability, sort, theme)
  5. **Popup filter overlays**: Multi-select modals rendered on top of table
  6. **Modal stack**: Table → Detail view (Enter), Compare (m/c), Plan (p), Escape to go back
  7. **Sort columns**: 7 sortable columns, visual indicator under active sort
  8. **Theme system**: 6 built-in themes, persisted, cycled with 't'
  9. **Information-dense table**: 14 columns with color-coded metrics
  10. **Plan mode**: Editable fields to estimate hardware requirements
  11. **Download workflow**: In-table progress bars with braille spinner + block fill

---

## UX Patterns to Apply: llmfit → tmuxcc

| llmfit Pattern | tmuxcc Application | Priority |
|---|---|---|
| Semantic color coding (fit levels) | Status colors: green=Idle, yellow=Processing, magenta=AwaitingApproval, red=Error | High |
| System/info bar | Enhanced header: show per-agent-type counts, system stats more visibly | Medium |
| Live search/filter | Agent search bar: filter agents by name, status, session, agent type | High |
| Popup filter overlays | Filter by AgentType (Claude/OpenCode/Codex/Gemini) and Status | Medium |
| Compact table view | Alternative flat table view (instead of tree) with sortable columns | Medium |
| Sort columns | Sort agents by: Status, AgentType, Session, SubagentCount | Low |
| Rich ● indicators | Colored ● status dot before each agent entry | High |
| Theme system | Basic theme cycling (2-3 themes) | Low |
| Information density | Add columns: AgentType, SubagentCount, ContextRemaining % in tree | Medium |

### Decision: What to Implement
Focus on highest-impact, lowest-complexity changes:
1. **Semantic status colors** — change styles.rs to use meaningful colors per AgentStatus
2. **Agent filter/search bar** — new UI component + AppState field + live filtering
3. **Colored ● status indicators** — improve agent_tree.rs rendering
4. **Enhanced header info** — add agent-type breakdown to header

---

## Implementation Plan

### 1. styles.rs — Semantic Color System
Add `status_color(status)` function returning Color based on AgentStatus:
- Idle → Color::Green
- Processing → Color::Yellow
- AwaitingApproval → Color::Magenta
- Error → Color::Red
- Unknown → Color::Gray

### 2. app/state.rs — Add filter state
- Add `filter_query: String` to AppState
- Add `filter_status: Option<AgentStatusFilter>`
- Add method `filtered_agents()` that returns agents matching filter

### 3. ui/components/ — New FilterBar widget
- Shows search input and status filter chips (like llmfit's filter row)
- Only visible when filter is active or user presses '/' (like llmfit's search)

### 4. agent_tree.rs — Rich indicators
- Use ● colored by status instead of plain status text
- Show agent type abbreviation [CC]/[OC]/[GC]/[CD]
- Show subagent count if > 0

### 5. ui/layout.rs — Add filter bar row
- When filter is active, insert filter bar between header and content

### 6. app/actions.rs — New actions
- FilterMode (enter filter), FilterChar(char), FilterBackspace, ClearFilter, CycleStatusFilter

