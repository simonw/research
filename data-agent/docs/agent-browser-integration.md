# Plan: Productionize pwa — AI-powered data extractor with explore/replay

## Context

The `playwright-automator` is a TypeScript CLI that records browser sessions, analyzes API traffic, and generates deterministic Playwright scripts. Currently it requires users to **manually record** browser sessions and provide a Gemini API key.

**The new vision**: A simple CLI where users type a natural language request and the tool autonomously explores the site, learns the data flow, generates a deterministic script, and publishes it for future replay.

```
npx pwa "get a list of 10 of my chatgpt conversations"
```

**Built on top of**:
- [agent-browser](https://github.com/vercel-labs/agent-browser) — Use its `BrowserManager` as a library for the explore phase. Gives us compact accessibility snapshots, system Chrome, and browser lifecycle management without bundling Playwright ourselves.
- [Skyvern](https://github.com/Skyvern-AI/skyvern) — Adapt its explore/replay architecture, Planner→Actor→Validator agent loop, intent metadata, and element hashing for self-healing.
- [PostHog Wizard](https://posthog.com/blog/envoy-wizard-llm-agent) — Envoy/MCP pattern for agent integration.

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
  ├─ 4. EXPLORE (agent-browser + LLM agent loop)
  │     ├─ agent-browser BrowserManager (system Chrome, no Chromium download)
  │     ├─ Compact accessibility snapshots via agent-browser's snapshot system
  │     ├─ LLM decides: what to click, where to navigate (Skyvern-style loop)
  │     ├─ Playwright HAR recording on the underlying context
  │     ├─ Intent metadata captured per action (Skyvern-style)
  │     └─ Stops when enough data patterns observed
  │
  ├─ 5. ANALYZE (our existing pipeline)
  │     └─ HAR → IR → workflow patterns → timeline → analysis.json
  │
  ├─ 6. GENERATE (LLM writes automation.ts)
  │     └─ Standalone Playwright script with API interception (waitForResponse)
  │
  ├─ 7. VALIDATE (run script, verify output)
  │     └─ Up to 5 fix-and-retry iterations
  │
  ├─ 8. PUBLISH (save to registry for future replay)
  │
  └─ 9. Return results (output.json)
```

---

## What We Reuse

### From agent-browser (TypeScript — use as dependency)

| Component | File in agent-browser | How we use it |
|-----------|----------------------|---------------|
| **BrowserManager** | `src/browser.ts` | Library import — launch browser, navigate, click. Gives us system Chrome, multi-session, state save/load. No daemon needed. |
| **Accessibility snapshots** | `src/snapshot.ts` | Direct reuse — `getSnapshot()` produces compact @ref format (200-400 tokens). Uses `page.locator(':root').ariaSnapshot()` + `compactTree()` + `findCursorInteractiveElements()`. |
| **System Chrome** | `src/browser.ts` | Via `executablePath` option. Supports `AGENT_BROWSER_EXECUTABLE_PATH` env var. |
| **State save/load** | `src/actions.ts` | `saveStorageState()` / `loadStorageState()` for auth persistence. |
| **Network tracking** | `src/browser.ts` | `startRequestTracking()` / `getRequests()` for monitoring API calls during exploration. |
| **Ref-to-selector mapping** | `src/snapshot.ts` | The `RefMap` maps @e1, @e2 → CSS selector + ARIA role. LLM outputs "@e3", we resolve to a Playwright locator. |

**Not reused from agent-browser**:
- Daemon/socket architecture (we don't need multi-client)
- CLI command parsing (we have our own)
- iOS automation
- Video recording/streaming

### From Skyvern (Python — adapt patterns to TypeScript)

| Pattern | File in Skyvern | How we adapt it |
|---------|----------------|-----------------|
| **Planner→Actor→Validator loop** | `forge/agent.py:agent_step()` | Port loop structure: scrape→decide→execute→verify. Our explore agent does plan→act→observe→record→evaluate. |
| **Action schema with intent** | `webeye/actions/actions.py` | Adapt `Action` class: `{ actionType, elementRef, reasoning, intention, confidence }`. Each action stores *why* not just *what*. |
| **Element hashing for recovery** | `webeye/scraper/scraper.py:hash_element()` | Hash element structure (tag, attributes, text) excluding IDs. Store hash with action for replay recovery. v2 self-healing. |
| **DOM extraction (domUtils.js)** | `webeye/scraper/domUtils.js` | Consider adapting `get_interactable_element_tree()` JS to complement agent-browser's accessibility snapshots. |
| **Script generation from workflow** | `core/script_generations/generate_script.py` | Study their `ACTION_MAP` pattern for converting recorded actions to Playwright code. |
| **Goal verification** | `forge/agent.py:complete_verify()` | Adapt: after each explore step, LLM checks if we've seen enough API patterns to understand the data flow. |
| **MCP server** | `cli/mcp.py` | Simple `FastMCP` pattern — expose `run_task(prompt, url)` tool. |

### From our existing codebase (keep as-is)

| Component | File | Notes |
|-----------|------|-------|
| HAR analyzer | `src/har-analyzer.ts` | Parses HAR into structured data |
| IR builder | `src/ir-builder.ts` + `src/ir.ts` | Endpoint scoring and ranking |
| Workflow analyzer | `src/workflow-analyzer.ts` | Detects list→detail, pagination, variable flow |
| Action-API correlator | `src/action-api-correlator.ts` | Links user actions to API calls |
| Response schema | `src/response-schema.ts` | Extract response shapes |
| Request templates | `src/request-templates.ts` | Sanitized request templates |
| Auth profiles | `src/auth-profiles.ts` + `src/auth-profile-discovery.ts` | Auth management |
| Validator | `src/runner.ts` | Execute script, validate output, structured error feedback |
| Login capture | `src/login.ts` | Headed browser login with 2FA support |
| Script generation prompts | `src/gemini.ts` | Adapt prompt construction (make LLM-agnostic) |

---

## Explore Phase: Agent Loop (Skyvern-inspired, agent-browser-powered)

```typescript
// Pseudocode for the explore loop
import { BrowserManager } from 'agent-browser';

async function explore(task: string, url: string, authState?: string) {
  const browser = new BrowserManager();
  await browser.launch({
    executablePath: detectSystemChrome(), // or AGENT_BROWSER_EXECUTABLE_PATH
    headless: false,
  });

  // Load auth if available
  if (authState) await browser.loadStorageState(authState);

  // Enable HAR recording on underlying Playwright context
  // (agent-browser wraps Playwright — we access the context for HAR)

  await browser.navigate(url);
  browser.startRequestTracking(); // Monitor API calls

  const actions: ActionWithIntent[] = [];

  for (let step = 0; step < MAX_STEPS; step++) {
    // 1. OBSERVE: Get compact snapshot with @refs
    const snapshot = await browser.getSnapshot({ interactive: true, compact: true });
    const networkActivity = browser.getRequests();

    // 2. PLAN: LLM decides next action (Skyvern-style)
    const decision = await llm.exploreStep({
      task,
      currentSnapshot: snapshot,     // "@e1 [link] 'All chats'\n@e2 [link] 'Yesterday'..."
      actionsSoFar: actions,         // Previous actions with intent
      apisSeen: networkActivity,     // API calls observed so far
    });

    // 3. ACT: Execute via agent-browser
    if (decision.action === 'click') await browser.click(decision.ref); // @e3
    if (decision.action === 'navigate') await browser.navigate(decision.url);
    if (decision.action === 'scroll') await browser.scroll(decision.direction);
    if (decision.action === 'done') break;

    // 4. RECORD: Log action + intent metadata (Skyvern-style)
    actions.push({
      step,
      action: decision.action,
      ref: decision.ref,
      reasoning: decision.reasoning,    // "Click conversation to trigger detail API"
      intention: decision.intention,     // "Discover how conversation detail data is loaded"
      timestamp: Date.now(),
    });

    // 5. EVALUATE: Check if we've seen enough API patterns
    if (decision.hasEnoughPatterns) break;
  }

  // Save HAR + actions + auth state to session directory
  return { har, actions, authState };
}
```

---

## Generated Scripts: Standalone Playwright

Generated `automation.ts` files remain pure Playwright with API interception — **no dependency on agent-browser at runtime**. This means:
- Users can `npx tsx automation.ts` anywhere
- Scripts are portable, shareable, CI-friendly
- Use system Chrome via `executablePath` (auto-detected or env var)

```typescript
// Generated automation.ts (simplified example)
import { chromium } from 'playwright';

const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH || '/Applications/Google Chrome.app/...',
  headless: true,
});
const context = await browser.newContext({ storageState: './storageState.json' });
const page = await context.newPage();

// API interception — our core strength
const listPromise = page.waitForResponse(
  r => r.url().includes('/api/conversations') && r.status() === 200
);
await page.goto('https://chatgpt.com');
const conversations = await (await listPromise).json();

writeFileSync('./output.json', JSON.stringify(conversations, null, 2));
await browser.close();
```

---

## File Changes

### New files
| File | Purpose | Based on |
|------|---------|----------|
| `src/explorer.ts` | AI explore agent loop | Skyvern `agent.py:agent_step()` |
| `src/intent-parser.ts` | Natural language → domain + task | New |
| `src/browser-detect.ts` | Find system Chrome/Edge | New (agent-browser uses env vars) |
| `src/replay.ts` | Run saved script deterministically | New |
| `src/registry.ts` | Global extractor registry | Adapted from skill `registry.mjs` |
| `src/mcp-server.ts` | MCP server for agent integration | Inspired by Skyvern `cli/mcp.py` |
| `src/llm/provider.ts` | Pluggable LLM interface | New |
| `src/llm/gemini.ts` | Gemini provider | Extracted from `src/gemini.ts` |
| `src/llm/claude.ts` | Claude provider | New |

### Modified files
| File | Change |
|------|--------|
| `src/index.ts` | New CLI: `pwa "description"`, `pwa replay`, `pwa list`, `pwa mcp` |
| `src/recorder.ts` | Adapt to work alongside agent-browser for explore phase |
| `package.json` | Add `agent-browser`, `@modelcontextprotocol/sdk`, `@anthropic-ai/sdk` |

### Keep unchanged
- `src/har-analyzer.ts`, `src/ir-builder.ts`, `src/ir.ts`
- `src/workflow-analyzer.ts`, `src/action-api-correlator.ts`
- `src/response-schema.ts`, `src/request-templates.ts`
- `src/auth-profiles.ts`, `src/auth-profile-discovery.ts`
- `src/runner.ts`, `src/login.ts`

### Retire
- `~/.claude/skills/web-data-extractor/` — MCP mode replaces the skill

---

## Implementation Stages

### Stage 1: agent-browser integration + system Chrome
**Goal**: Use agent-browser as the browser layer. Verify system Chrome works.

1. Add `agent-browser` as dependency in `package.json`
2. `src/browser-detect.ts` — detect system Chrome/Chromium/Edge paths on macOS/Linux/Windows
3. Prototype: import `BrowserManager`, launch with system Chrome, take a snapshot
4. Verify agent-browser's snapshot produces compact @ref output
5. Verify we can enable Playwright HAR recording alongside agent-browser's browser context
6. Verify `state save/load` works for auth

**Key risk**: Does agent-browser expose the underlying Playwright context for HAR recording? If not, we may need to fork or use Playwright directly with our own snapshot function.

**Files**: `package.json`, `src/browser-detect.ts`, prototype script

### Stage 2: Explorer agent loop
**Goal**: Replace manual recording with autonomous AI exploration.

1. `src/explorer.ts` — implement the explore loop:
   - Launch agent-browser BrowserManager
   - Enable HAR recording
   - Run plan→act→observe→record→evaluate loop
   - Capture actions with intent metadata (Skyvern-style Action schema)
   - Stop when: enough API patterns found, OR max steps reached
   - Output: session dir with HAR, actions.json, auth state
2. `src/intent-parser.ts` — extract domain + task from natural language
3. Wire up: `pwa "description"` → intent parser → auth check → explorer → analysis pipeline → generate → validate

**Files**: `src/explorer.ts`, `src/intent-parser.ts`, `src/index.ts`

### Stage 3: Pluggable LLM + registry + replay
**Goal**: Support multiple LLM providers. Ship the full CLI.

1. `src/llm/provider.ts` — interface for `generateScript`, `refineScript`, `exploreStep`
2. `src/llm/gemini.ts` — extracted from current `gemini.ts`
3. `src/llm/claude.ts` — Claude provider via Anthropic SDK
4. `src/registry.ts` — global registry (`~/.pwa/registry.json`), auto-publish after validation
5. `src/replay.ts` — run saved script, exit 0/1/2, auth freshness check
6. Refactor `src/index.ts`:
   - Default: `pwa "description"` → full auto pipeline
   - `pwa replay [domain]` → deterministic replay
   - `pwa login [url]` → auth capture (unchanged)
   - `pwa list` → show registry

**Files**: `src/llm/*.ts`, `src/index.ts`, `src/registry.ts`, `src/replay.ts`

### Stage 4: MCP server + polish + publish
**Goal**: Agent integration + npm publish.

1. `src/mcp-server.ts` — expose tools: `explore(prompt, url)`, `replay(domain)`, `list_extractors()`
2. CLI: `pwa mcp` starts MCP server on stdio
3. README with quick start for CLI, MCP, and replay modes
4. Test against 3-5 real sites
5. `npm publish`

**Files**: `src/mcp-server.ts`, `package.json`, `README.md`

---

## Verification

1. **Natural language**: `npx pwa "get the top 10 stories from hacker news"` → AI explores HN, captures API, generates script, returns stories
2. **System Chrome**: No Chromium download prompt. Uses system Chrome via agent-browser.
3. **Compact snapshots**: During explore, snapshots are 200-400 tokens (visible in logs)
4. **Replay**: `npx pwa replay news.ycombinator.com` → runs saved script without LLM
5. **Registry**: `npx pwa list` → shows HN extractor with creation date and stats
6. **Auth flow**: `npx pwa "get my chatgpt conversations"` → detects auth → login → explore → return data
7. **MCP**: Connect from Claude Code, say "extract my github stars" → agent invokes MCP tools
8. **Portable script**: Copy `automation.ts` to another machine, `npx tsx automation.ts` works standalone

---

## v2 Roadmap (not in scope)

- **Self-healing**: Element hashing (Skyvern-style) → when replay fails, try hash-based selector recovery → LLM fixes script → retry
- **Intent-based recovery**: Use action intent metadata to find alternative paths when DOM changes
- **Web UI**: Paste URL + describe task → get data (non-technical users)
- **Script marketplace**: Community-shared extractors for popular sites
- **Scheduled runs**: Built-in cron + webhook delivery
