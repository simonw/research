# Plan: data-agent — AI-powered data extractor with explore/replay

## Context

Build a new CLI tool called `data-agent` at `/Users/kahtaf/Documents/workspace_kahtaf/research/data-agent/`. Fresh codebase — we'll reference `playwright-automator` for proven patterns (HAR analysis, IR building, workflow detection, API interception) but rewrite from scratch with the new architecture in mind.

**The vision**: A simple CLI where users type a natural language request and the tool autonomously explores the site, learns the data flow, generates a deterministic Playwright script, and publishes it for future replay.

```
npx data-agent "get a list of 10 of my chatgpt conversations"
```

**Built with**:
- **Playwright** (direct) — browser control, HAR recording, API interception (`waitForResponse`)
- **agent-browser's `snapshot.ts`** (copied) — compact accessibility snapshots with @refs
- **Skyvern patterns** (adapted to TypeScript) — explore/replay, Planner→Actor→Validator loop, intent metadata
- **MCP SDK** — agent integration for any coding agent

---

## Architecture Overview

```
npx data-agent "get my chatgpt conversations"
  │
  ├─ 1. Parse intent → domain: chatgpt.com, task: get conversations
  │
  ├─ 2. Registry lookup → found? → REPLAY (deterministic, no LLM)
  │                         │
  │                     not found
  │                         ↓
  ├─ 3. Auth check → need login? → open headed browser, user logs in
  │
  ├─ 4. EXPLORE (Playwright + LLM agent loop)
  │     ├─ System Chrome via executablePath (no Chromium download)
  │     ├─ Compact accessibility snapshots (copied from agent-browser)
  │     ├─ LLM decides: what to click, where to navigate (Skyvern-style loop)
  │     ├─ Playwright HAR recording captures all API traffic
  │     ├─ Intent metadata captured per action (Skyvern-style)
  │     └─ Stops when enough data patterns observed
  │
  ├─ 5. ANALYZE (adapted from playwright-automator)
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

### Three Modes

| Mode | Command | LLM needed? | Description |
|------|---------|-------------|-------------|
| **Auto** | `npx data-agent "description"` | Yes | Full: explore → analyze → generate → validate → publish |
| **Replay** | `npx data-agent replay chatgpt.com` | No | Run saved script deterministically |
| **MCP** | `npx data-agent mcp` | Agent's LLM | MCP server for any coding agent |

---

## What We Copy/Adapt

### From agent-browser (copy `snapshot.ts`)

Copy `snapshot.ts` (~17KB) from [vercel-labs/agent-browser](https://github.com/vercel-labs/agent-browser/blob/main/src/snapshot.ts). This gives us:

- `getSnapshot(page, options)` — compact accessibility tree with @refs
- `RefMap` — maps @e1, @e2 → `{ selector, role, name, nth }`
- `INTERACTIVE_ROLES` / `CONTENT_ROLES` / `STRUCTURAL_ROLES` — role classification
- `compactTree()` — removes empty structural branches
- `findCursorInteractiveElements()` — detects clickables with `cursor:pointer` that lack ARIA roles
- `getSnapshotStats()` — token/line/ref counts

Uses Playwright's `page.locator(':root').ariaSnapshot()` under the hood.

### From Skyvern (adapt patterns to TypeScript)

| Pattern | Skyvern source | Our adaptation |
|---------|---------------|----------------|
| **Agent loop** | `forge/agent.py:agent_step()` | `src/explorer.ts` — plan→act→observe→record→evaluate loop |
| **Action + intent** | `webeye/actions/actions.py` | `src/types.ts` — `{ actionType, ref, reasoning, intention, confidence }` |
| **Goal verification** | `forge/agent.py:complete_verify()` | LLM evaluates: "have we seen enough API patterns?" after each step |
| **Element hashing** | `webeye/scraper/scraper.py:hash_element()` | v2: hash element structure for self-healing selector recovery |
| **MCP server** | `cli/mcp.py` | `src/mcp-server.ts` — expose `explore`, `replay`, `list` tools |

### From playwright-automator (reference for analysis pipeline)

We'll study and rewrite these modules (not copy verbatim — fresh TypeScript, cleaner APIs):

| Concept | Reference file | What we take |
|---------|---------------|--------------|
| HAR parsing | `src/har-analyzer.ts` | Extract API calls from HAR, filter noise, detect auth headers |
| Endpoint scoring | `src/ir-builder.ts` + `src/ir.ts` | Rank endpoints by usefulness (JSON response, array data, high score) |
| Workflow detection | `src/workflow-analyzer.ts` | Detect list→detail pairs, pagination, variable flow |
| Action→API correlation | `src/action-api-correlator.ts` | Link user actions to triggered API calls (2s correlation window) |
| Response schemas | `src/response-schema.ts` | Extract response shapes for LLM context |
| Request templates | `src/request-templates.ts` | Sanitized request shapes (strip sensitive headers) |
| Script generation prompts | `src/gemini.ts` | API-first strategy, listener-before-action, UI-driven interception |
| Validation | `src/runner.ts` | Execute script, validate output.json, structured error feedback |
| Error classification | `src/runner.ts:extractErrorFeedback()` | 403/401/429/timeout/empty-output diagnosis patterns |

---

## Project Structure

```
data-agent/
├── package.json
├── tsconfig.json
├── src/
│   ├── index.ts                 # CLI entry point
│   ├── types.ts                 # Shared types (Action, Session, etc.)
│   │
│   ├── browser/
│   │   ├── detect.ts            # Find system Chrome/Edge
│   │   └── snapshot.ts          # Copied from agent-browser
│   │
│   ├── explore/
│   │   ├── explorer.ts          # AI agent loop (plan→act→observe→record→evaluate)
│   │   └── intent-parser.ts     # Natural language → domain + task
│   │
│   ├── analyze/
│   │   ├── har-analyzer.ts      # HAR parsing + API extraction
│   │   ├── ir-builder.ts        # Endpoint scoring and ranking
│   │   ├── workflow-analyzer.ts # Pattern detection (list→detail, pagination)
│   │   ├── correlator.ts        # Action→API correlation
│   │   ├── response-schema.ts   # Schema extraction
│   │   └── templates.ts         # Request template sanitization
│   │
│   ├── generate/
│   │   └── script-generator.ts  # LLM generates automation.ts from analysis
│   │
│   ├── validate/
│   │   └── validator.ts         # Execute script, validate output, error feedback
│   │
│   ├── replay/
│   │   ├── replay.ts            # Run saved script (no LLM)
│   │   └── registry.ts          # Global extractor registry
│   │
│   ├── auth/
│   │   ├── login.ts             # Headed browser login with 2FA
│   │   └── profiles.ts          # Auth profile management
│   │
│   ├── llm/
│   │   ├── provider.ts          # Pluggable LLM interface
│   │   ├── gemini.ts            # Gemini provider
│   │   └── claude.ts            # Claude provider
│   │
│   └── mcp/
│       └── server.ts            # MCP server
│
└── prompts/
    ├── explore.md               # System prompt for explore agent
    ├── generate.md              # System prompt for script generation
    └── diagnose.md              # System prompt for error diagnosis
```

---

## Explore Phase: Agent Loop

```typescript
// src/explore/explorer.ts — Pseudocode

async function explore(task: string, url: string, authState?: string) {
  const executablePath = detectSystemChrome();
  const browser = await chromium.launch({ executablePath, headless: false });
  const context = await browser.newContext({
    storageState: authState || undefined,
    recordHar: { path: sessionDir + '/recording.har', mode: 'full' },
  });
  const page = await context.newPage();

  await page.goto(url);

  const actions: ActionWithIntent[] = [];
  const apisSeen: ApiCall[] = [];

  // Track all API responses
  page.on('response', async (r) => {
    if (isApiResponse(r)) {
      apisSeen.push({ url: r.url(), status: r.status(), method: r.request().method() });
    }
  });

  for (let step = 0; step < MAX_STEPS; step++) {
    // 1. OBSERVE: Compact snapshot with @refs (from agent-browser snapshot.ts)
    const { snapshot, refMap } = await getSnapshot(page, { interactive: true, compact: true });

    // 2. PLAN: LLM decides next action
    const decision = await llm.exploreStep({
      task,
      snapshot,           // "@e1 [link] 'All chats'\n@e2 [link] 'Yesterday'..."
      actionsSoFar: actions,
      apisSeen,           // API calls observed so far
    });

    if (decision.done) break;

    // 3. ACT: Execute via Playwright using ref-to-selector mapping
    const selector = refMap[decision.ref]; // @e3 → getByRole('link', { name: 'Chat' })
    if (decision.action === 'click') await page.locator(selector).click();
    if (decision.action === 'type') await page.locator(selector).fill(decision.text);
    if (decision.action === 'scroll') await page.evaluate(() => window.scrollBy(0, 500));
    if (decision.action === 'navigate') await page.goto(decision.url);

    await page.waitForLoadState('networkidle');

    // 4. RECORD: Action + intent metadata
    actions.push({
      step,
      action: decision.action,
      ref: decision.ref,
      reasoning: decision.reasoning,
      intention: decision.intention,
      timestamp: Date.now(),
    });
  }

  await context.close();
  await browser.close();

  return { harPath: sessionDir + '/recording.har', actions, apisSeen };
}
```

---

## Generated Scripts: Standalone Playwright

Generated `automation.ts` files are pure Playwright — **zero dependencies on data-agent at runtime**.

```typescript
// Example generated automation.ts
import { chromium } from 'playwright';
import { writeFileSync } from 'fs';

async function main() {
  const browser = await chromium.launch({
    executablePath: process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless: true,
  });
  const context = await browser.newContext({ storageState: './storageState.json' });
  const page = await context.newPage();

  const listPromise = page.waitForResponse(
    r => r.url().includes('/backend-api/conversations') && r.status() === 200
  );
  await page.goto('https://chatgpt.com');
  const data = await (await listPromise).json();

  writeFileSync('./output.json', JSON.stringify(data, null, 2));
  console.log(`Done. Wrote ${data.items?.length ?? 0} items.`);
  await browser.close();
}

main();
```

---

## Implementation Stages

### Stage 1: Project setup + browser + snapshots
**Goal**: New project with system Chrome and compact snapshots working.

1. Init project: `package.json`, `tsconfig.json`, dependencies (`playwright`, `tsx`)
2. Copy `snapshot.ts` from agent-browser, adapt imports
3. `src/browser/detect.ts` — detect system Chrome/Chromium/Edge on macOS/Linux/Windows
4. Prototype: launch system Chrome, navigate to a site, take compact snapshot, print it
5. Verify HAR recording works alongside snapshots

### Stage 2: Explorer agent loop
**Goal**: AI-driven exploration that discovers data patterns autonomously.

1. `src/types.ts` — action schema with intent metadata
2. `src/explore/intent-parser.ts` — extract domain + task from natural language
3. `src/explore/explorer.ts` — the full agent loop (plan→act→observe→record→evaluate)
4. `src/llm/provider.ts` + `src/llm/gemini.ts` — pluggable LLM for explore steps
5. `prompts/explore.md` — system prompt for the explore agent
6. Test: `data-agent "get top stories from hacker news"` → explores HN, records HAR

### Stage 3: Analysis pipeline
**Goal**: Turn recorded HAR + actions into structured analysis.

1. `src/analyze/har-analyzer.ts` — parse HAR, extract API calls, filter noise
2. `src/analyze/ir-builder.ts` — score endpoints, build ranked catalog
3. `src/analyze/workflow-analyzer.ts` — detect list→detail, pagination, variable flow
4. `src/analyze/correlator.ts` — link actions to API calls
5. `src/analyze/response-schema.ts` + `templates.ts` — extract schemas and templates
6. Wire up: explorer output → analysis pipeline → `analysis.json`

### Stage 4: Script generation + validation
**Goal**: LLM generates deterministic Playwright scripts from analysis.

1. `src/generate/script-generator.ts` — build prompt from analysis.json, call LLM, output automation.ts
2. `src/validate/validator.ts` — execute script via `npx tsx`, validate output.json, error feedback
3. `prompts/generate.md` — system prompt for script generation (API-first, listener-before-action, etc.)
4. `prompts/diagnose.md` — error diagnosis patterns
5. Agent loop: generate → validate → diagnose → refine → validate (up to 5x)

### Stage 5: Registry + replay + auth
**Goal**: Publish scripts and replay without LLM.

1. `src/replay/registry.ts` — global registry at `~/.data-agent/registry.json`
2. `src/replay/replay.ts` — run saved automation.ts, exit 0/1/2
3. `src/auth/login.ts` — headed browser login with 2FA support
4. `src/auth/profiles.ts` — auth profile discovery and management
5. CLI commands: `data-agent replay [domain]`, `data-agent list`, `data-agent login [url]`

### Stage 6: MCP server + CLI polish + npm publish
**Goal**: Agent integration and ship v1.

1. `src/mcp/server.ts` — tools: `explore(prompt, url)`, `replay(domain)`, `list()`
2. `src/llm/claude.ts` — Claude provider via Anthropic SDK
3. Polish CLI: progress indicators, colored output, `--help`
4. Test against 3-5 real sites (HN, Reddit, ChatGPT, GitHub, etc.)
5. `npm publish`

---

## Verification

1. **Natural language**: `npx data-agent "get the top 10 stories from hacker news"` → explores HN, generates script, returns stories
2. **System Chrome**: No Chromium download. Uses `/Applications/Google Chrome.app` (or detected path).
3. **Compact snapshots**: Explore phase logs show ~200-400 token snapshots with @refs
4. **Replay**: `npx data-agent replay news.ycombinator.com` → runs saved script, no LLM, fresh output
5. **Registry**: `npx data-agent list` → shows extractors with stats
6. **Auth**: `npx data-agent "get my chatgpt conversations"` → detects auth needed → login → explore → data
7. **MCP**: Connect from Claude Code, invoke `explore` tool → agent-driven workflow
8. **Portable**: Copy `automation.ts` to new machine, `npx tsx automation.ts` works standalone

---

## v2 Roadmap (not in scope)

- **Self-healing**: Element hashing (Skyvern-style) + intent-based recovery
- **Web UI**: Paste URL + describe task → get data
- **Script marketplace**: Community-shared extractors
- **Scheduled runs**: Cron + webhook delivery
- **Claude provider**: Add alongside Gemini in v1 or v2
