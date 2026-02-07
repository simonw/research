# Plan: Productionize pwa — AI-powered data extractor with explore/replay

## Context

The `playwright-automator` is a TypeScript CLI that records browser sessions, analyzes API traffic, and generates deterministic Playwright scripts. Currently it requires users to **manually record** browser sessions and provide a Gemini API key.

**The new vision**: A simple CLI where users type a natural language request and the tool autonomously explores the site, learns the data flow, generates a deterministic script, and publishes it for future replay.

```
npx pwa "get a list of 10 of my chatgpt conversations"
```

**Inspired by**:
- [Skyvern](https://github.com/Skyvern-AI/skyvern) — Explore/replay pattern, intent metadata, Planner→Actor→Validator agent loop, self-healing fallback
- [agent-browser](https://agent-browser.dev/) — Compact accessibility snapshots (@refs), system Chrome, token-efficient browser interaction
- [PostHog Wizard](https://posthog.com/blog/envoy-wizard-llm-agent) — Envoy/MCP pattern for agent integration

---

## Architecture Overview

```
npx pwa "get my chatgpt conversations"
  │
  ├─ 1. Parse intent → domain: chatgpt.com, task: get conversations
  │
  ├─ 2. Registry lookup → found? → REPLAY (deterministic, no LLM)
  │                         │
  │                     not found
  │                         ↓
  ├─ 3. Auth check → need login? → open headed browser, user logs in
  │
  ├─ 4. EXPLORE (AI agent navigates site autonomously)
  │     ├─ Playwright with system Chrome (no Chromium download)
  │     ├─ Compact accessibility snapshots (agent-browser inspired)
  │     ├─ LLM decides: what to click, where to navigate
  │     ├─ HAR recording captures all API traffic
  │     └─ Stops when enough data patterns observed
  │
  ├─ 5. ANALYZE (our existing pipeline)
  │     └─ HAR → IR → workflow patterns → timeline → analysis.json
  │
  ├─ 6. GENERATE (LLM writes automation.ts)
  │     └─ API interception script using waitForResponse
  │
  ├─ 7. VALIDATE (run script, verify output)
  │     └─ Up to 5 fix-and-retry iterations
  │
  ├─ 8. PUBLISH (save to registry for future replay)
  │
  └─ 9. Return results (output.json)
```

### Three Modes

| Mode | Command | LLM needed? | Description |
|------|---------|-------------|-------------|
| **Auto** | `npx pwa "description"` | Yes | Full pipeline: explore → analyze → generate → validate → publish |
| **Replay** | `npx pwa replay chatgpt.com` | No | Run saved script deterministically |
| **MCP** | `npx pwa mcp` | Agent's LLM | MCP server — any coding agent can drive the pipeline |

---

## Browser Layer Decision: Playwright with System Chrome

**Why not build on agent-browser directly:**
- agent-browser is a CLI/daemon — hard to integrate into our TypeScript pipeline
- No HAR recording support (we need full traffic capture)
- No `waitForResponse` pattern (critical for our API interception scripts)
- Adds architectural complexity (bridging CLI↔daemon↔our code)

**What we steal from agent-browser:**
- **Compact accessibility snapshots** — Build our own `getAccessibilitySnapshot()` function that produces the compact @ref format (200-400 tokens vs 3000-5000 for full DOM). This is critical for the explore phase where the LLM needs to "see" the page efficiently.
- **System Chrome** — Use Playwright's `executablePath` option to launch system-installed Chrome/Edge. No Chromium download needed. Detect system browser at startup.
- **State save/load** — We already do this (storageState.json). Keep it.

**What we steal from Skyvern:**
- **Explore/replay architecture** — AI explores once, generates deterministic script, replays without LLM
- **Intent metadata** — Each action during exploration captures *why* (e.g., "click conversation to trigger detail API"), not just *what* (e.g., "click @e3"). This helps with self-healing later.
- **Planner→Actor→Validator loop** — Structured agent loop for the explore phase
- **Fallback hierarchy** — When replay breaks: try alternate selectors → single LLM query → full re-explore

---

## Key Components

### 1. Intent Parser
Extracts domain and task from natural language input.

```
"get a list of 10 of my chatgpt conversations"
→ { domain: "chatgpt.com", task: "list conversations", count: 10, needsAuth: true }
```

Uses LLM for ambiguous cases. Hardcoded heuristics for common patterns (domain extraction from keywords).

### 2. Explorer Agent (NEW — replaces manual recording)
An autonomous agent loop that navigates the site to discover data patterns.

**Loop structure** (inspired by Skyvern's Planner→Actor→Validator):
```
1. PLAN: LLM reads task + current page snapshot → decides next action
2. ACT: Execute action via Playwright (click, navigate, scroll)
3. OBSERVE: Take accessibility snapshot + check network activity
4. RECORD: Log action + intent metadata + any new API responses
5. EVALUATE: Have we seen enough API patterns to understand the data flow?
   → Yes: stop exploring
   → No: loop back to PLAN
```

**Key design decisions:**
- Use **compact accessibility snapshots** (agent-browser style) for the OBSERVE step — keeps context small
- HAR recording runs throughout — captures ALL traffic during exploration
- The LLM gets: current snapshot + task description + actions taken so far + API responses seen
- Max exploration budget: ~20 actions (prevents runaway costs)
- The explore phase outputs: HAR file, action log with intent metadata, auth state

### 3. Analysis Pipeline (EXISTING — keep as-is)
- `har-analyzer.ts` → parse HAR
- `ir-builder.ts` → score and rank endpoints
- `workflow-analyzer.ts` → detect list→detail, pagination, variable flow
- `action-api-correlator.ts` → link actions to API calls
- `response-schema.ts` → extract response shapes
- `request-templates.ts` → sanitize request templates

### 4. Script Generator (EXISTING — evolve)
- Currently in `gemini.ts` — make LLM-agnostic
- Takes `analysis.json` + intent metadata from exploration
- Generates `automation.ts` using API interception (waitForResponse)
- Generated scripts use system Chrome via `executablePath`

### 5. Validator (EXISTING — keep as-is)
- `runner.ts` — execute script, validate output, structured error feedback
- Agent loop: up to 5 fix-and-retry iterations

### 6. Registry (EVOLVE from skill version)
- Global at `~/.pwa/registry.json`
- Lookup by domain + fuzzy task match
- Auto-publish after successful validation
- Track replay history (last run, success/failure count)

### 7. Replay Engine (NEW)
- Run saved `automation.ts` without LLM
- Structured exit codes: 0=success, 1=error, 2=auth expired
- Detect system Chrome at runtime
- Auth freshness check before replay

### 8. MCP Server (NEW)
- Expose tools: `explore`, `replay`, `list_extractors`
- Provide resources: analysis data, templates
- Staged prompts for agent-driven workflows

---

## File Changes

### New files
| File | Purpose |
|------|---------|
| `src/explorer.ts` | AI explorer agent loop (plan→act→observe→record→evaluate) |
| `src/snapshot.ts` | Compact accessibility snapshot (agent-browser inspired @ref format) |
| `src/intent-parser.ts` | Parse natural language → domain + task |
| `src/browser-detect.ts` | Find system Chrome/Edge, skip Chromium download |
| `src/replay.ts` | Run saved script deterministically |
| `src/registry.ts` | Global extractor registry |
| `src/mcp-server.ts` | MCP server for agent integration |
| `src/llm/provider.ts` | Pluggable LLM interface |
| `src/llm/gemini.ts` | Gemini provider (extracted from current gemini.ts) |
| `src/llm/claude.ts` | Claude provider |

### Modified files
| File | Change |
|------|--------|
| `src/index.ts` | New CLI: `pwa "description"`, `pwa replay`, `pwa list`, `pwa mcp` |
| `src/gemini.ts` | Extract into `src/llm/gemini.ts`, deprecate |
| `src/recorder.ts` | Add HAR capture mode for explorer (non-interactive) |
| `package.json` | Add `@modelcontextprotocol/sdk`, `@anthropic-ai/sdk`; update bin |

### Keep unchanged
- `src/har-analyzer.ts`, `src/ir-builder.ts`, `src/ir.ts`
- `src/workflow-analyzer.ts`, `src/action-api-correlator.ts`
- `src/response-schema.ts`, `src/request-templates.ts`
- `src/auth-profiles.ts`, `src/auth-profile-discovery.ts`
- `src/runner.ts` (validation loop)
- `src/login.ts` (auth capture)

### Retire
- `~/.claude/skills/web-data-extractor/` — MCP mode replaces the skill

---

## Implementation Stages

### Stage 1: System Chrome + compact snapshots
**Goal**: Use system Chrome instead of downloading Chromium. Build the compact snapshot function.

1. `src/browser-detect.ts` — detect system Chrome/Chromium/Edge on macOS/Linux/Windows
2. Update `src/recorder.ts` to use `executablePath` from browser detection
3. `src/snapshot.ts` — `getAccessibilitySnapshot(page)` returns compact text with @refs:
   ```
   @e1 [link] "All chats"
   @e2 [link] "Yesterday's conversation"
   @e3 [button] "New chat"
   @e4 [input] placeholder="Search conversations"
   ```
4. Test: verify recording works with system Chrome, verify snapshots are compact

**Files**: `src/browser-detect.ts`, `src/snapshot.ts`, `src/recorder.ts`

### Stage 2: Explorer agent
**Goal**: Replace manual recording with autonomous AI exploration.

1. `src/explorer.ts` — the explore agent loop:
   - Takes: task description, URL (optional), auth state
   - Launches browser with HAR recording
   - Runs plan→act→observe→record→evaluate loop
   - Stops after: enough API patterns found, OR max actions reached, OR LLM says done
   - Outputs: session directory with HAR, actions.json (with intent metadata), auth state
2. `src/intent-parser.ts` — extract domain + task from natural language
3. Wire up: `pwa "description"` → intent parser → auth check → explorer → analysis pipeline

**Files**: `src/explorer.ts`, `src/intent-parser.ts`, `src/index.ts`

### Stage 3: Pluggable LLM + natural language CLI
**Goal**: Support multiple LLM providers. Ship the natural language CLI.

1. `src/llm/provider.ts` — interface for generateScript, refineScript, exploreStep
2. `src/llm/gemini.ts` — extract from current gemini.ts
3. `src/llm/claude.ts` — Claude provider via Anthropic SDK
4. Refactor `src/index.ts`:
   - Default command: `pwa "description"` → full auto pipeline
   - `pwa replay [domain]` → deterministic replay
   - `pwa login [url]` → auth capture (unchanged)
   - `pwa list` → show registry
   - `pwa mcp` → start MCP server
5. `src/registry.ts` — global registry with auto-publish
6. `src/replay.ts` — run saved script, exit 0/1/2

**Files**: `src/llm/*.ts`, `src/index.ts`, `src/registry.ts`, `src/replay.ts`

### Stage 4: MCP server + polish
**Goal**: Agent integration + npm publish.

1. `src/mcp-server.ts` — MCP server with tools/resources/prompts
2. Polish CLI UX: progress indicators, colored output, clear error messages
3. README with quick start examples
4. Test against 3-5 real sites
5. `npm publish`

**Files**: `src/mcp-server.ts`, `package.json`, `README.md`

---

## Verification

1. **Natural language**: `npx pwa "get the top 10 stories from hacker news"` → explores HN, captures API, generates script, returns stories
2. **Replay**: `npx pwa replay news.ycombinator.com` → runs saved script without LLM, returns fresh stories
3. **System Chrome**: No Chromium download prompt. Uses system Chrome.
4. **Registry**: `npx pwa list` → shows HN extractor with creation date and stats
5. **Auth flow**: `npx pwa "get my chatgpt conversations"` → detects auth needed → opens browser for login → explores → returns conversations
6. **MCP**: Connect from Claude Code, say "extract my bookmarks from github" → agent invokes MCP tools
7. **Portable script**: Copy `automation.ts` to another machine, `npx tsx automation.ts` works standalone

---

## v2 Roadmap (not in scope)

- **Self-healing**: Replay detects failure → auto-heal (LLM fixes script) → retry
- **Intent metadata for recovery**: When selectors break, use intent to find alternative paths (Skyvern-style)
- **Web UI**: Paste URL + describe task → get data (non-technical users)
- **Script marketplace**: Community-shared extractors for popular sites
- **Scheduled runs**: Built-in cron + webhook delivery
