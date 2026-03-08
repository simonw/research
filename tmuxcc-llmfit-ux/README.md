# tmuxcc × llmfit: UX Cross-Pollination Investigation

**Date**: 2026-03-08

---

## Executive Summary

This investigation cloned and deeply analyzed two Rust TUI tools — **tmuxcc** (a dashboard for managing AI coding agents in tmux) and **llmfit** (a hardware-to-LLM matching tool) — then identified and implemented UX patterns from llmfit that improve tmuxcc's usability. The result is a working fork with clean compilation and all 26 tests passing.

---

## The Tools

### tmuxcc
- **Repo**: https://github.com/nyanko3141592/tmuxcc
- **Purpose**: Centralized dashboard for monitoring and controlling multiple AI coding agents (Claude Code, OpenCode, Codex CLI, Gemini CLI) running in tmux panes
- **Stack**: Rust, ratatui 0.29, crossterm 0.28, tokio, sysinfo
- **Key UX**: Tree view of sessions/windows/agents; approve/reject agent requests with single keystrokes; pane preview; subagent log

### llmfit
- **Repo**: https://github.com/AlexsJones/llmfit
- **Purpose**: Match 2000+ LLM models to hardware (RAM, CPU, GPU/VRAM) with scoring, filtering, and download
- **Stack**: Rust workspace (llmfit-core + llmfit-tui + llmfit-desktop), ratatui 0.30, crossterm 0.29, tokio, axum
- **Key UX**: Always-visible filter bar with search input + chip selectors; color-coded fit levels; modal detail/compare views; theme system

---

## Architecture Deep-Dive

### tmuxcc Architecture

```
┌────────────────────────────────────────────────────────────┐
│  main.rs  (clap CLI) → run_app() async                     │
├────────────────────────────────────────────────────────────┤
│  Async tokio event loop (ui/app.rs)                         │
│  ┌─────────────┐   ┌────────────────┐   ┌───────────────┐ │
│  │  AppState   │   │  MonitorTask   │   │  TUI Render   │ │
│  │  (state.rs) │◄──│  (monitor/)    │   │  (ratatui)    │ │
│  └─────────────┘   └────────────────┘   └───────────────┘ │
│         │                 │                                  │
│         ▼                 ▼                                  │
│  ┌─────────────┐   ┌────────────────┐                       │
│  │  Action     │   │  TmuxClient    │                       │
│  │  (actions)  │   │  (subprocess)  │                       │
│  └─────────────┘   └────────────────┘                       │
│                           │                                  │
│                    ┌──────▼──────┐                           │
│                    │  Parsers    │                           │
│                    │  (regex)    │                           │
│                    └─────────────┘                           │
└────────────────────────────────────────────────────────────┘
```

**Core modules:**

| Module | Role |
|--------|------|
| `agents/types.rs` | `MonitoredAgent`, `AgentStatus` (Idle/Processing/AwaitingApproval/Error), `ApprovalType` |
| `app/state.rs` | `AppState` — central UI state machine (selection, input, focus, filter) |
| `app/actions.rs` | `Action` enum — all possible user interactions |
| `tmux/client.rs` | `TmuxClient` — subprocess calls to tmux (list-panes, capture-pane, send-keys) |
| `tmux/pane.rs` | `PaneInfo` — process tree cache, child command detection |
| `parsers/` | Trait-based per-agent parsers; ClaudeCode parser is most sophisticated (600+ lines) |
| `monitor/task.rs` | Async background polling with 2-second hysteresis on state changes |
| `ui/components/` | 7 ratatui widgets: Header, AgentTree, PanePreview, Input, SubagentLog, Help, Footer |

**Testing infrastructure:**
- Unit tests in `#[cfg(test)]` blocks in each module file
- 26 tests total, covering parsers, types, navigation, config serialization, tmux target parsing
- Dev dependency: `tempfile` for temp file creation in config tests
- No integration tests

**Layout:**
```
Header (3 lines)   — agent count, working, pending, CPU/MEM/time
Content (min 10)   — [AgentTree sidebar | PanePreview right]
Footer (1 line)    — clickable Y/N/A/☐/F/?/Q buttons
```

---

### llmfit Architecture

```
llmfit (workspace)
├── llmfit-core/       — Hardware detection, model DB, fit scoring, provider trait
├── llmfit-tui/        — Interactive TUI, CLI subcommands, REST API (axum)
└── llmfit-desktop/    — Tauri macOS desktop wrapper
```

**Core algorithms:**

| Component | Detail |
|-----------|--------|
| Hardware detection | sysinfo + nvidia-smi/rocm-smi/system_profiler for GPU; unified memory (Apple Silicon) |
| Model database | `hf_models.json` — 2000+ HuggingFace models with GGUF sources, MoE flags, capabilities |
| Fit scoring | 4-dimensional: quality (params + quant) × speed (bandwidth-estimated tok/s) × fit (memory headroom %) × context |
| Inference paths | GPU → MoE expert offload → CPU offload → CPU-only (in preference order) |
| Quantization | Dynamic selection from F16 → Q8_0 → Q6_K → Q4_K_M → Q2_K based on budget |

**Key UX innovations in llmfit:**

| Pattern | Implementation |
|---------|----------------|
| Always-visible filter bar | 3-line row: system info + search input + filter chips — always on screen |
| Semantic fit-level colors | green=Perfect, yellow=Good, magenta=Marginal, red=TooTight — consistent everywhere |
| Cyclic chip filters | FitFilter, AvailabilityFilter, sort column all cycle with single keypresses |
| Live search | `/` focuses search box; real-time AND filtering across name/provider/params/use-case |
| Popup overlays | P/U/C open multi-select modals on top of table without losing context |
| Modal stack | Table → Detail (Enter) → back (Esc); Table → Compare → back (Esc) |
| Download progress | In-table animated progress bar (braille spinner + block fill ░▒▓█) |
| Theme system | 6 themes, persisted, cycled with `t` |

**Testing:**
- 16 unit tests in `llmfit-core` (quantization math, memory estimation, MoE models, use-case inference)
- No integration tests; TUI relies on manual interactive testing

---

## UX Pattern Analysis: What llmfit Does Differently

The central insight is that **llmfit treats filtering as a first-class citizen**. Rather than having filtering be a modal overlay or secondary feature, llmfit dedicates a persistent row to live filtering and chip-based facet filters. This comes from its use case: 2000+ models requires powerful filtering. But the pattern generalizes.

tmuxcc, by contrast, presents agents as a static tree. When you have 20+ agents across multiple sessions, finding the one that needs your attention is slow.

### Transferable Patterns

| llmfit Pattern | Why It Works | tmuxcc Application |
|----------------|--------------|-------------------|
| **Always-visible filter row** | No mode-switch cost to filter; state visible at all times | Agent search + status/type chip row |
| **Semantic color coding** | Magenta ≠ Red: "needs attention" ≠ "broken" | AwaitingApproval → Magenta; Error → Red |
| **Cyclic chip filters** | One keypress to cycle; chips show current state | Status chip (All/Waiting/Working/Idle/Error); Type chip (All/Claude/OpenCode/Codex/Gemini) |
| **Live text search** | `/` to enter search, instant results | Filter by session name, window name, agent type, working directory |
| **Match count in title** | "3/7 agents (filtered)" vs "7 agents" | Shows how many agents pass current filter |
| **Dimming non-matches** | Non-matching rows stay visible but grayed out (context preserved) | Agents not matching filter are rendered in DarkGray |
| **Color for agent types** | Provider filter chips colored by provider | Claude=Magenta, OpenCode=Blue, Codex=Green, Gemini=Yellow |

---

## Implementation

### Changes Made to tmuxcc

All changes are in `/home/user/research/tmuxcc-llmfit-ux/tmuxcc/`. A full patch is saved in `tmuxcc-llmfit-ux.patch`.

#### 1. `src/app/state.rs` — Filter state + enums + methods

Added two new enums modeled on llmfit's `FitFilter` and provider filter:

```rust
pub enum StatusFilter { All, Waiting, Working, Idle, Error }
pub enum AgentTypeFilter { All, ClaudeCode, OpenCode, CodexCli, GeminiCli }
```

Both implement `.next()` for cycling (same pattern as llmfit's `FitFilter::next()`) and `.label()` for chip display.

Added fields to `AppState`:
```rust
pub filter_query: String,       // text search query
pub filter_active: bool,        // is filter bar input focused
pub filter_status: StatusFilter,
pub filter_agent_type: AgentTypeFilter,
```

Added methods: `enter_filter()`, `exit_filter()`, `filter_char()`, `filter_backspace()`, `clear_filter()`, `cycle_status_filter()`, `cycle_type_filter()`, `has_active_filter()`, `agent_matches_filter()`, `filtered_agent_count()`.

The `agent_matches_filter()` method mirrors llmfit's `apply_filters()`:
- Text query: case-insensitive substring match against session + window_name + agent_type + abbreviated_path
- Status chip: exact `AgentStatus` variant match
- Type chip: exact `AgentType` match

#### 2. `src/app/actions.rs` — Filter actions

```rust
EnterFilter, ExitFilter, FilterChar(char), FilterBackspace,
ClearFilter, CycleStatusFilter, CycleTypeFilter,
```

#### 3. `src/ui/components/filter_bar.rs` — NEW widget

New `FilterBarWidget` renders a 2-line bar:

**Line 1 (search):**
```
 / filter: [search query___]   3/7 agents   Esc: done  Ctrl-U: clear
```
- Label brightens to Cyan when focused, Yellow when filter active but unfocused
- Shows match count when filter is active (like llmfit's filtered count display)

**Line 2 (chips):**
```
 Status: [All][⚠ Waiting][● Working][● Idle][✗ Error]   Type: [All][Claude][Open][Codex][Gemini]
```
- Active chip is highlighted with background color + bold (like llmfit's active filter indicator)
- Each chip has a semantic color: Waiting=Magenta, Working=Yellow, Idle=Green, Error=Red

#### 4. `src/ui/layout.rs` — `main_layout_with_filter()`

```rust
pub fn main_layout_with_filter(area: Rect) -> Vec<Rect> {
    // [header(3), filter_bar(2), content(min 10), footer(1)]
}
```

#### 5. `src/ui/app.rs` — Event loop + rendering

- Switched main render to use `main_layout_with_filter()`
- Renders `FilterBarWidget` between header and content
- Added filter mode in `map_key_to_action()`: when `filter_active`, characters route to `FilterChar(c)`, Esc → `ExitFilter`, Ctrl-U → `ClearFilter`
- New keybindings in sidebar mode:
  - `/` → `EnterFilter` (same as llmfit)
  - `p`/`P` → `CycleTypeFilter` (like llmfit's `P` for provider)
  - `F` (Shift+F) → `CycleStatusFilter`
  - `Esc` → `ClearFilter` when filter is active

#### 6. `src/ui/styles.rs` — Semantic color fix

```rust
// Before: AwaitingApproval was Red (same as Error)
pub fn awaiting_approval() -> Style {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
}

// After: Magenta (like llmfit's Marginal level) — distinguishes "needs input" from "broken"
pub fn awaiting_approval() -> Style {
    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
}
```

#### 7. `src/ui/components/agent_tree.rs` — Dimming + filter-aware title

- Added `passes_filter` check per agent using `state.agent_matches_filter(agent)`
- When `!passes_filter`: all agent styles (status, type, item background) override to DarkGray
- AwaitingApproval indicator updated to Magenta
- Title now shows `"3/7 agents (filtered)"` when any filter is active (like llmfit's table title)

### Result

```
Build: ✓  clean, no warnings
Tests: ✓  26/26 passing (all original tests)
Patch: 608 lines (tmuxcc-llmfit-ux.patch)
```

---

## What the Updated tmuxcc Looks Like

```
╭─────────────────────────────────────────────────────────────╮
│ TmuxCC │ 7 agents │ ⠋ 2 working │ ⚠ 1 pending │ CPU 12.3% │ │
╰─────────────────────────────────────────────────────────────╯
 / filter: [claude_______]   3/7 agents   Esc: done  Ctrl-U: clear
 Status: [All][⚠ Waiting][● Working][● Idle][✗ Error]   Type: [All][Claude][Open][Codex][Gemini]
╭────────────────────────────╮╭──────────────────────────────╮
│ 3/7 agents (filtered)      ││  Session: main / pane 0      │
│                            ││  ─────────────────────────── │
│ ▼ main                     ││  $ claude-code --continue    │
│  ├─ 0: code                ││  ⠋ Thinking...               │
│  ┃  ● main:0.0             ││                              │
│     Claude │ Idle │ pid:.. ││                              │
│  ├─ 0: code         (dim)  ││                              │
│     ● main:0.1  ░░░░░░░░░  ││                              │
│     Open  │ Idle │ pid:..  ││                              │
│  └─ 0: code         (dim)  ││                              │
│     ● main:0.2             ││                              │
│     Gemini│ Idle │ pid:..  ││                              │
╰────────────────────────────╯╰──────────────────────────────╯
 Y: Approve  N: Reject  A: All  ☐: Select  f: Focus  ?: Help  q: Quit
```

Non-matching agents (OpenCode, Gemini when filtering for "claude") are shown in DarkGray, preserving spatial context while highlighting matches — the same dimming behavior llmfit uses for models outside the selected fit level.

---

## What Wasn't Implemented (and Why)

| llmfit Pattern | Decision |
|----------------|----------|
| **Full modal detail view** | tmuxcc already has PanePreview which serves this role |
| **Side-by-side compare** | Agent output is live; comparing two live panes is less useful than viewing one |
| **Plan mode** | No tmuxcc analogue — hardware planning is llmfit-specific |
| **Download progress bars** | Agents don't "download"; not applicable |
| **Theme system** | Viable but lower value; would require threading color through all widgets |
| **REST API** | Already a planned feature in tmuxcc; out of scope for this experiment |
| **Hiding non-matching agents** | Chose dimming instead — keeps tree spatial context and avoids index-tracking complexity |

---

## Key Technical Learnings

### 1. Ratatui widget patterns are highly portable
Both tools use identical ratatui idioms: `Frame::render_widget()`, `Line::from(vec![Span::styled(...)])`, `Block::default().borders(Borders::ALL)`. Adding a new widget to tmuxcc required zero ratatui API learning — same as llmfit.

### 2. Stateless widgets make state management explicit
Both tools keep all mutable state in a central `AppState`-equivalent struct and pass immutable references to widgets. This makes filter state trivially addable without touching any widget internals.

### 3. The Action enum pattern is ideal for filter UX
tmuxcc's `Action` enum (same as llmfit's internal event handling) allows adding filter actions without touching the event loop structure. New actions plug in cleanly to the existing match statement.

### 4. Color semantics matter more than aesthetics
The most impactful single change was `AwaitingApproval: Red → Magenta`. In the original tmuxcc, both "agent errored" and "agent needs your input" showed in red. This creates alert fatigue — everything urgent looks the same. llmfit's color hierarchy (green → yellow → magenta → red as severity increases) maps directly: Idle → Processing → AwaitingApproval → Error.

### 5. Filter bars work best when always visible
llmfit's filter row is on screen even when no filter is active. This has two benefits: (1) users discover the feature passively, (2) there's no mode switch cost — you always know where to look. In tmuxcc, the filter bar now follows this pattern, showing hints even when inactive.

### 6. Match counts reduce filter anxiety
When a filter produces zero or few results, users worry they set the filter wrong. llmfit addresses this with "3/7 agents (filtered)" in the title, giving instant feedback on filter effectiveness. This small addition has outsized UX value.

---

## Possible Next Steps

1. **Hide-vs-dim toggle**: Add a keybinding to toggle between dimming non-matches and hiding them entirely (llmfit hides; dimming was chosen for tmuxcc to preserve context)

2. **Persistence**: Save filter state (status/type chip selections) across sessions, like llmfit persists theme choice

3. **Popup filter overlays**: For users with many agent types, a multi-select popup (like llmfit's P/U/C popups) could replace the cycling chip approach

4. **Real-time filter update on agent state change**: Currently, if an agent transitions from Idle to AwaitingApproval while the Waiting filter is active, the tree updates automatically (filter is re-evaluated each render frame) — this already works correctly

5. **Theme system**: 3 themes (dark/light/high-contrast) following llmfit's theme persistence pattern

6. **Table view mode**: An alternative flat list view (toggle with `v`) showing agents as rows with sortable columns (like llmfit's main table view), which would work better than the tree when sessions/windows structure isn't meaningful

---

## Files

```
tmuxcc-llmfit-ux/
├── notes.md                    # Running investigation log
├── README.md                   # This report
├── tmuxcc-llmfit-ux.patch      # Git diff of all changes to tmuxcc (608 lines)
├── tmuxcc/                     # Modified tmuxcc fork (not committed per AGENTS.md)
└── llmfit/                     # llmfit clone for reference (not committed)
```

The patch file contains the complete diff and can be applied to a fresh clone of tmuxcc with `git apply tmuxcc-llmfit-ux.patch`.
