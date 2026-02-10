# Browser automation & scraping agents — market landscape + learnings to steal (2026-02-07)

Goal: survey OSS (and adjacent) tools for **reliable browser scraping/automation** and extract concrete patterns to apply to **data-agent** ("bulletproof data scraping CLI agent").

> Focus: reliability (self-healing), extraction correctness, anti-bot, observability, deterministic replay, and prompt/system-design.

## TL;DR (what we should steal)

### 1) Treat the browser as a *sensor* and API traffic as the real payload
- Your current approach (HAR → IR → standalone Playwright) is directionally right. The biggest wins come from:
  - **API-first extraction** when possible (waitForResponse predicates, request templates).
  - **UI-driven triggering** (click the UI *only* to trigger APIs; then parse JSON).

### 2) Make “agent mode” and “production mode” different (Stagehand philosophy)
- Use AI when exploring/repairing, but **cache deterministic actions** and re-run without the LLM.
- Detect drift; only re-invoke the LLM when selectors/structure change.

### 3) Strong loop hygiene + timeout discipline
- Use explicit step budgets, loop detection, and “break glass” fallbacks.
- Enforce *time remaining* guards around each expensive action (Stagehand does this).

### 4) Hybrid snapshotting beats pure a11y tree
- Pure a11y tree is token-efficient but misses lots of web reality (iframes, shadow DOM, canvas, virtualized lists).
- Stagehand’s **hybrid snapshot** approach (a11y + url map + xpath/id encoding + diffing) is a good model.

### 5) Anti-bot is multi-layered — stealth plugins are necessary but not sufficient
- Reddit/WSB blocking in our tests is typical: IP reputation, TLS fingerprinting, behavior, cookies.
- Design for:
  - **proxy/session mgmt**, sticky sessions,
  - captcha handling boundaries,
  - and fallback surfaces (official APIs, cached registries).

### 6) Observability is a feature
- Save **artifacts** every run: HAR, screenshots, page snapshots, action logs, diffs, extracted JSON, error classification.
- Make it easy for a user to replay and debug without re-running the LLM.

---

## Tools reviewed (primary list)

### 1) browser-use/browser-use
Repo: https://github.com/browser-use/browser-use

**What it is**
- A browser agent framework (Python) that feeds the LLM a structured view of the current browser state + optional vision screenshot and runs an iterative agent loop.

**Notable patterns to steal**
1. **Rich system prompt template** with explicit loop rules, anti-loop guidance, captcha limits, and “don’t retry 403 forever”.
   - Ref: `browser_use/agent/system_prompts/system_prompt.md`
     - https://github.com/browser-use/browser-use/blob/main/browser_use/agent/system_prompts/system_prompt.md
2. **Runtime prompt selection by model type** (different system prompts for different models/modes).
   - Ref: `browser_use/agent/prompts.py` — chooses templates for flash mode, no-thinking mode, “browser-use” finetunes, Anthropic 4.5 caching constraints.
     - https://github.com/browser-use/browser-use/blob/main/browser_use/agent/prompts.py
3. **Page statistics** (counts of links/iframes/shadow roots/interactive nodes) injected into context.
   - This is a cheap, useful heuristic for:
     - detecting “block pages” / empty SPA states
     - deciding whether to switch strategies (HTTP fetch vs render)
   - Ref: `AgentMessagePrompt._extract_page_statistics()` in `browser_use/agent/prompts.py`.

**Gotchas / limits**
- Heavy reliance on structured DOM/a11y views can miss:
  - nested iframes, closed shadow DOM, canvas content
  - virtualized lists (content not in tree)
- Still needs strong anti-bot infrastructure (proxy/cookies/session, human-in-loop for CAPTCHAs).

**Concrete ideas for data-agent**
- Add “page stats” to your snapshot payload: iframes/shadow/open/closed counts and interactive element count.
- Implement hard anti-loop rules like:
  - if same URL 3+ steps and no new APIs observed → switch to alternate path.

---

### 2) Skyvern-AI/skyvern
Repo: https://github.com/Skyvern-AI/skyvern

**What it is**
- A production-oriented AI workflow automation system built around browser automation, planning, and execution with strong infra/observability.

**What to steal (beyond the planner you already use)**
1. **Explicit “Page AI” / script-generation abstraction**
   - Skyvern has internal modules that appear to wrap “browser page AI” as a reusable component.
   - Refs (examples found locally):
     - `skyvern/library/skyvern_browser_page_ai.py`
     - `skyvern/core/script_generations/skyvern_page_ai.py`
2. **Service boundary**: treat “browser execution” as a service that produces artifacts.
   - Pattern: isolate flaky browser tasks with queues/timeouts/artifacts.

**Concrete ideas for data-agent**
- Consider an internal service boundary between:
  - explore → analyze → generate
  - and validate/replay
- Make every run produce a “bundle” (session dir) with a stable schema for artifacts.

---

### 3) google-gemini/gemini-cli — web-fetch tool
Doc: https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/web-fetch.md

**Key design patterns**
- **Explicit confirmation** before network fetch.
- **Two-tier retrieval**: provider-side `urlContext` fetch first, fall back to local fetch.
- **Citations** as first-class output.

**What to steal**
- Add an approval gate when prompts contain URLs (esp. when running in sensitive environments).
- Add caching / dedupe and SSRF protections (the doc doesn’t cover them, but it’s implied by “safe fetch”).

---

### 4) browserbase/stagehand
Repo: https://github.com/browserbase/stagehand

**What it is**
- A developer-first automation framework that blends code + AI with strong emphasis on “reliability in production”.

**Notable patterns to steal (very relevant)**
1. **Hybrid snapshots + diffs**
   - Ref: `packages/core/lib/v3/understudy/a11y/snapshot` (see calls from act handler)
   - In `ActHandler`, they:
     - `captureHybridSnapshot()`
     - optionally `diffCombinedTrees()` between snapshots
     - use two-step act: act → deterministic action → if needed, focused second step
   - Ref: `packages/core/lib/v3/handlers/actHandler.ts`
     - https://github.com/browserbase/stagehand/blob/main/packages/core/lib/v3/handlers/actHandler.ts
2. **Timeout guard discipline**
   - They create a “time remaining” function and call it before/after expensive steps.
   - Ref: `createTimeoutGuard()` usage in `actHandler.ts` and `extractHandler.ts`.
3. **Two-step “act”**
   - The LLM proposes an action; they execute deterministically; if the LLM requested two-step, they re-snapshot and do a follow-up action.
   - This reduces token cost and increases success on dynamic UIs.
4. **Schema-aware extract**
   - Extract uses Zod schema; they transform `z.string().url()` → numeric IDs and later re-inject URLs from `combinedUrlMap`.
   - This is a clever trick: LLM outputs stable IDs; the runtime rehydrates real URLs.
   - Ref: `packages/core/lib/v3/handlers/extractHandler.ts`
     - https://github.com/browserbase/stagehand/blob/main/packages/core/lib/v3/handlers/extractHandler.ts

**Concrete ideas for data-agent**
- Add a “two-step” optional mode to exploration:
  - step 1: broad snapshot → action
  - step 2: diff/focused snapshot → finish the interaction
- In your script generator, prefer generating:
  - listener-before-action patterns (`waitForResponse` predicate installed prior)
  - plus a “fallback re-snapshot and retry” block.
- Adopt the “URL as ID” trick in your extraction schema path to avoid brittle string matching.

---

### 5) berstend/puppeteer-extra (and stealth)
Repo: https://github.com/berstend/puppeteer-extra
Stealth plugin: https://github.com/berstend/puppeteer-extra/tree/master/packages/puppeteer-extra-plugin-stealth

**What it is**
- Plugin system + stealth evasions (also provides `playwright-extra`).

**What stealth actually does (pattern)**
- Patches common automation fingerprints via a set of “evasions” (navigator.webdriver, plugins, user agent override, webgl vendor, iframe contentWindow, chrome.runtime mocks, default args, etc.).
- Ref: stealth readme + evasions directory
  - `packages/puppeteer-extra-plugin-stealth/readme.md`
  - `packages/puppeteer-extra-plugin-stealth/evasions/*/index.js`

**Critical reality check**
- Even if you pass public “bot test” pages, real sites still block on:
  - IP reputation
  - TLS fingerprint (JA3/HTTP2 behavior)
  - behavior (click cadence, scroll patterns)
  - cookies/consent flows

**Concrete ideas for data-agent**
- Keep stealth, but treat it as only one layer.
- Build first-class proxy/session rotation (sticky sessions) and configurable “humanization” delays.

---

### 6) Lightpanda
Docs: https://lightpanda.io/docs/

**What it is (high level)**
- A lightweight headless browser/runtime aiming to be faster/cheaper than full Chrome in some cases.

**Why it matters to a scraping CLI**
- “Bulletproof” often means scaling to many pages. Chrome is heavy; lighter engines can reduce cost and improve throughput.
- Tradeoff: compatibility (web APIs, rendering completeness) vs speed.

**What to steal**
- Consider a dual-engine strategy:
  - default: full Playwright/Chromium for compatibility
  - optional: lighter engine for simpler sites/API harvesting
- Expose engine choice as a CLI flag.

---

## Other similar OSS tools worth mining (shortlist)

### Microsoft Playwright MCP
Repo: https://github.com/microsoft/playwright-mcp
- Pattern: MCP server exposing Playwright via structured a11y snapshots.
- Key theme from docs: MCP vs CLI tradeoffs (token cost, schema verbosity).

### vercel-labs/agent-browser
Repo: https://github.com/vercel-labs/agent-browser
- Pattern: token-efficient **CLI** interface (open/snapshot/click/fill/wait) and a11y refs.
- Useful for data-agent: a CLI interface can be cheaper than MCP for coding agents.

### Apify Crawlee
Repo: https://github.com/apify/crawlee
- Pattern: industrial crawling primitives: queues, retries, storage, proxy rotation, session pools.
- We should steal these crawl-level concerns (politeness, backoff, persistence) even if we don’t adopt the whole framework.

(Additional candidates to evaluate next: SeleniumBase, Playwright Stealth approaches, LaVague, browserless/chrome, scrapegraph-ai, firecrawl, etc.)

---

## Reliability gotchas we directly hit (data-agent experience)

1. **Generated scripts must be cross-platform**
- Gemini generated `automation.ts` with a hard-coded macOS Chrome path; validation failed on the Pi.
- Fix we applied: ensure replay injects detected system browser path via `CHROME_PATH`.

2. **Playwright browser binaries may not be installed**
- Running a generated script failed until `npx playwright install chromium` was executed.
- Recommendation: validator should auto-detect this failure mode and run install (or print a one-liner fix).

3. **Real sites block even real browsers** (Reddit/WSB)
- We saw 403 block pages even when using Playwright + stealth.
- This suggests a need for:
  - manual login flow (`data-agent login`)
  - proxy/session support
  - fallback to official APIs

---

## Concrete roadmap for “bulletproof data-agent”

### A) Extraction strategy improvements
- Prefer API-first scripts, but formalize fallback:
  1) try JSON endpoints discovered in HAR
  2) if blocked, use UI extraction of visible text
  3) if blocked, require login/auth profile
  4) if blocked, recommend official API

### B) Better error classification (validator)
- Add explicit patterns:
  - missing browser executable
  - blocked/interstitial detection (tiny snapshot, 403 HTML)
  - cookie/consent gate
  - captcha detection

### C) Session + proxy primitives
- First-class flags:
  - `--proxy`, `--proxy-pool`, `--session <name>` (sticky cookies)
  - `--humanize` (randomized delays, scroll jitter)

### D) Artifact bundle contract
- Standardize session dir outputs:
  - `recording.har`, `analysis.json`, `session.json`
  - `snapshots/*.json`, `screenshots/*.png`
  - `automation.ts`, `output.json`, `run.log`

### E) Prompt/system design
- Borrow browser-use’s loop rules + Stagehand’s two-step/diff design.
- Keep prompts short but policy-rich (anti-loop, fallback, stop conditions).

---

## Specific code references (files + line numbers) for patterns worth stealing

> Line numbers below are from a local clone taken on 2026-02-07. For permanence, also link the file path in the upstream repo and (ideally) pin a commit hash when we implement.

### browser-use — multi-action “page-change guard” + loop nudges + history compaction

1) **Batch action execution with hard guards against stale DOM**
- Pattern: when executing multiple actions, abort the remaining action queue if the page changes (URL or focused target) or an action is marked as terminating.
- Ref: `browser_use/agent/service.py` **L2632–L2640** (docstring describing both guard layers) and **L2683–L2686** (capture pre-action URL/focus)

2) **Soft loop detection that nudges the model without blocking**
- Pattern: hash actions + track page fingerprint stagnation; inject escalating messages at repetition thresholds.
- Ref: `browser_use/agent/views.py` **L155–L230** (`class ActionLoopDetector`, `record_action`, `record_page_state`, `get_nudge_message`)

3) **History compaction w/ a practical compaction prompt + sensitive-data filtering**
- Pattern: compact every N steps (and only above a char threshold), include read_state optionally, filter secrets, preserve URLs/paths.
- Ref: `browser_use/agent/message_manager/service.py` **L200–L280** (`maybe_compact_messages`)

### Stagehand — hybrid snapshot + network-quiet settle + cache replay

1) **Hybrid snapshot as “single source of truth”**
- Pattern: build AX+DOM hybrid snapshot; support scoped snapshot fast-path; merge iframes; produce combined maps.
- Ref: `packages/core/lib/v3/understudy/a11y/snapshot/capture.ts` **L45–L90** (`captureHybridSnapshot` overview) and **L110–L118** (scoped snapshot fallback notes)

2) **Network-quiet DOM settle (bounded, ignores WebSocket/SSE, stall sweeper)**
- Pattern: before acting, enable `Network` + `Page` events, wait for 0 inflight requests for 500ms; ignore WebSocket/EventSource; sweep stalled requests (>2s) so you never deadlock.
- Ref: `packages/core/lib/v3/handlers/handlerUtils/actHandlerUtils.ts` **L537–L679** (`waitForDomNetworkQuiet`)

3) **Action cache with replay + selector wait**
- Pattern: cache key = hash({instruction,url,variableKeys}); on cache hit, replay deterministic actions; wait for selector (best-effort) before each action; optionally refresh cache when actions drift.
- Ref: `packages/core/lib/v3/cache/ActCache.ts` **L40–L141** (`prepareContext`, `tryReplay`, `replayCachedActions`)

### Skyvern — step-level retries + provider-error short-circuit + scraping retry posture

1) **Step-level retries (retry_index) as a first-class state machine**
- Pattern: retries are not a try/catch loop — they create a new `Step` with `retry_index+1` and keep artifacts per step.
- Ref: `skyvern/forge/agent.py` **L3962–L4010** (`handle_failed_step`: retry cap, create next step)

2) **Failure summarization that detects provider errors without calling the LLM again**
- Pattern: while summarizing terminal failure, inspect action results for LLM-provider errors and record them deterministically (avoid “LLM-blame loops”).
- Ref: `skyvern/forge/agent.py` **L4012–L4060** (`summary_failure_reason_for_max_steps`: provider error detection)

3) **Scrape retry wrapper (and explicit “0 retries in prod” note)**
- Pattern: scrape wrapper classifies blank-page vs general exceptions; bounded retry with backoff; with a strong ops note that they disable retries in staging/prod.
- Ref: `skyvern/webeye/scraper/scraper.py` **L139–L220** (`scrape_website` and MAX_SCRAPING_RETRIES notes)

### puppeteer-extra / stealth — “what it changes” (GitHub line references recommended)

- Stealth is a *meta-plugin* that enables a list of “evasions” (navigator.webdriver, user-agent override, plugins/mimetypes, webgl vendor, iframe contentWindow, etc.).
- Recommendation: when we adopt/port, pin and cite:
  - `packages/puppeteer-extra-plugin-stealth/index.js` (enabled evasions list)
  - `packages/puppeteer-extra-plugin-stealth/evasions/user-agent-override/index.js` (CDP `Network.setUserAgentOverride`)

---

## Actionable integration plan (apply these patterns to our current data-agent)

### Phase 0 — hygiene + guardrails (0.5 day)
- [ ] Add a standard scratch location for external repos: `~/Downloads/tmp-repos/` (done operationally; keep as documented convention).
- [ ] Ensure *every* run writes a session bundle with stable filenames (you already have `recording.har`, `analysis.json`, `session.json`; add `run.log`, `screenshots/`, `snapshots/`).

### Phase 1 — validator reliability upgrades (1–2 days)
1) **Error classification table** in `src/validate/validator.ts`
   - Detect and classify:
     - missing Playwright browsers (“Executable doesn’t exist…”)
     - wrong CHROME_PATH / missing system Chrome
     - 403/blocked pages / interstitials
     - navigation timeout vs selector timeout
   - Action: auto-suggest or auto-run `npx playwright install chromium` in a safe mode.

2) **Blocked/interstitial detection** in explore phase (`src/explore/explorer.ts`)
   - Use a `pageStats` heuristic (browser-use style): if snapshot refs/tokens extremely small across 2 steps, treat as “blocked” and stop early with a clear reason.

### Phase 2 — Stagehand-style “act/extract” primitives inside explore (2–4 days)
1) **Network-quiet settle helper**
   - Implement `waitForDomNetworkQuiet()` (Stagehand pattern) in `src/browser/` and call it:
     - after `goto` and after click/type actions in explore
   - Ignore WebSocket/EventSource; add stall sweeper; time-bound to ~5s.

2) **Two-step action option**
   - Extend `ExploreDecision` to allow `twoStep=true`.
   - Execute: snapshot → action → resnapshot → diff text → second action (limited action set).

3) **Self-heal selector retry** (optional but high ROI)
   - On click/fill timeout, take a fresh snapshot and ask the LLM only to reselect the element ref (keep method/args stable).

### Phase 3 — Deterministic replay + caching (2–3 days)
1) **Action-plan cache** (Stagehand/Skyvern hybrid)
   - For exploration flows, cache a sequence of deterministic actions keyed by:
     - canonicalized URL (strip volatile params)
     - high-level task string
     - variable *keys* (not values)
   - Gate replay by matching anchors (Skyvern element hashes or Stagehand-style id maps) to avoid unsafe replays.

2) **Replay robustness**
   - Ensure replay sets `CHROME_PATH` and supports `--proxy`, `--headless`, and `--session` consistently.

### Phase 4 — Extraction correctness (2–4 days)
1) **URL-as-ID extraction**
   - If/when we add schema-based extraction, adopt Stagehand’s trick:
     - model outputs stable IDs for URL fields
     - runtime reinjects actual URLs from the snapshot’s urlMap

2) **Citations & provenance**
   - For outputs, include provenance metadata:
     - URL, timestamp, session id, and (if possible) minimal quotes/locators for key fields.

### Phase 5 — Anti-bot posture (ongoing)
- [ ] Treat stealth as only one layer; add:
  - proxy support + sticky sessions
  - humanization delays (jitter) as an option
  - login profiles (`data-agent login`) as a first-class flow
- [ ] When blocked, prefer alternative surfaces (official APIs) rather than infinite retries.

---

## References (URLs)
- browser-use: https://github.com/browser-use/browser-use
- Skyvern: https://github.com/Skyvern-AI/skyvern
- Gemini CLI web-fetch: https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/web-fetch.md
- Stagehand: https://github.com/browserbase/stagehand
- puppeteer-extra: https://github.com/berstend/puppeteer-extra
- Lightpanda docs: https://lightpanda.io/docs/
- Playwright MCP: https://github.com/microsoft/playwright-mcp
- agent-browser: https://github.com/vercel-labs/agent-browser
- Crawlee: https://github.com/apify/crawlee
- HN thread (Smooth CLI): https://news.ycombinator.com/item?id=46901233
