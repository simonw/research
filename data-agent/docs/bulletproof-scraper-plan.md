# Bulletproof Scraper — Integration Plan

> High-level plan for reliability upgrades to data-agent, derived from
> [browser-automation-landscape.md](./browser-automation-landscape.md).

## Goals

1. **Reliability** — detect and recover from common failure modes (blocked pages,
   missing browsers, network flakiness, empty SPA states).
2. **Observability** — every run produces a standardized artifact bundle that can
   be debugged without re-running the LLM.
3. **Determinism** — optional action-cache skeleton so repeat runs can skip the
   LLM when the page structure hasn't drifted.

---

## Upstream patterns adopted

| Pattern | Source | Upstream ref |
|---------|--------|-------------|
| Network-quiet DOM settle (bounded, ignores WS/SSE, stall sweeper) | Stagehand | [`packages/core/lib/v3/handlers/handlerUtils/actHandlerUtils.ts` L537–L679](https://github.com/browserbase/stagehand/blob/main/packages/core/lib/v3/handlers/handlerUtils/actHandlerUtils.ts) |
| Page statistics injection (links, iframes, shadow roots, interactive count) | browser-use | [`browser_use/agent/prompts.py` `_extract_page_statistics()`](https://github.com/browser-use/browser-use/blob/main/browser_use/agent/prompts.py) |
| Blocked-page / empty-SPA detection heuristic | browser-use | System prompt anti-loop rules in [`browser_use/agent/system_prompts/system_prompt.md`](https://github.com/browser-use/browser-use/blob/main/browser_use/agent/system_prompts/system_prompt.md) |
| Standardized artifact bundle per run | Skyvern | Service-boundary pattern in [`skyvern/forge/agent.py`](https://github.com/Skyvern-AI/skyvern/blob/main/skyvern/forge/agent.py) |
| Error classification + auto-fix hints | Skyvern | Step-level retries in [`skyvern/forge/agent.py` L3962–L4010](https://github.com/Skyvern-AI/skyvern/blob/main/skyvern/forge/agent.py) |
| Action cache skeleton (hash-keyed, deterministic replay) | Stagehand | [`packages/core/lib/v3/cache/ActCache.ts` L40–L141](https://github.com/browserbase/stagehand/blob/main/packages/core/lib/v3/cache/ActCache.ts) |
| Playwright auto-install hint | Own experience | Documented in `browser-automation-landscape.md` gotcha #2 |

---

## Deliverables

### 1. `src/browser/network-settle.ts` — Network-quiet settle helper

Adapted from Stagehand's `waitForDomNetworkQuiet()`:
- Enable CDP `Network` + `Page` domain events.
- Wait until 0 in-flight requests for a configurable quiet window (default 500 ms).
- Ignore WebSocket and EventSource connections (they never "complete").
- Stall sweeper: any request older than a threshold (default 2 s) is force-counted
  as done so we never deadlock on slow third-party scripts.
- Hard timeout bound (default 5 s) — return even if still noisy.

Consumed by `explore()` after `goto` and after each action instead of the
current `waitForLoadState('networkidle')` + `waitForTimeout(500)`.

### 2. `src/browser/page-stats.ts` — Page statistics + blocked detection

Inspired by browser-use `_extract_page_statistics()`:
- Counts: total elements, interactive elements, links, iframes, images,
  shadow roots, form fields.
- Returns `isLikelyBlocked` flag when:
  - Interactive element count < 3 AND total element count < 30
  - OR page title matches common block patterns ("Access Denied", "403", etc.)
  - OR body text length < 200 characters
- Used by explorer to detect blocked/interstitial pages early and bail with a
  clear diagnostic message.

### 3. `src/browser/artifacts.ts` — Standardized artifacts bundle

Formalizes the session directory contract:
```
session-{timestamp}/
  recording.har        # Full network traffic
  session.json         # Metadata: task, url, timestamps, actions, apis
  analysis.json        # IR, timeline, workflow, templates
  automation.ts        # Generated script
  automation.backup-*.ts  # Backup iterations
  output.json          # Extracted data
  run.log              # Combined stdout+stderr from validation runs
  page-stats.json      # Page statistics snapshots per step
```

Provides `writeArtifact()` / `readArtifact()` helpers and an `ArtifactBundle`
type for type-safe access.

### 4. Enhanced `src/validate/validator.ts` — Error classification with hints

Extend the existing error classifier with:

| Error class | Detection pattern | Hint |
|-------------|-------------------|------|
| `missing_browser` | "Executable doesn't exist" / "browserType.launch" | Auto-run `npx playwright install chromium` if `--auto-install` |
| `blocked_page` | 403 status + tiny snapshot / "Access Denied" | "Site is blocking automation. Try `data-agent login <url>` first" |
| `navigation_timeout` | "Navigation timeout" / "Timeout exceeded" | "Page did not load in time. Check URL and network" |
| `selector_timeout` | "waiting for selector" / "strict mode violation" | "Element not found. Page structure may have changed" |
| `auth_required` | 401 status / redirect to login page | "Authentication required. Run `data-agent login <url>`" |
| `rate_limited` | 429 status | "Rate limited. Add delays or use --proxy" |
| `captcha` | "captcha" / "challenge" in page text | "CAPTCHA detected. Manual intervention required" |
| `json_parse` | "Unexpected token <" / "SyntaxError.*JSON" | "API returned HTML instead of JSON. Check auth" |

Optional `--auto-install` flag: when `missing_browser` is detected, the validator
automatically runs `npx playwright install chromium` and retries.

### 5. `src/browser/action-cache.ts` — Deterministic action cache (skeleton)

Skeleton implementation adapted from Stagehand's `ActCache`:
- Cache key: `hash({ canonicalUrl, taskString, variableKeys })`.
- Cache value: ordered list of deterministic actions `{ action, selector, text }`.
- `tryCacheHit(key)` → returns cached actions or null.
- `recordActions(key, actions)` → persist to `~/.data-agent/action-cache/`.
- No replay logic yet (Phase 3) — this is the storage skeleton only.

### 6. Integration test harness — `src/test/integration.ts`

No test framework. A standalone script (`npx tsx src/test/integration.ts`) that:
1. Launches Playwright against 4 public demo sites:
   - `quotes.toscrape.com` — paginated quotes list
   - `books.toscrape.com` — book catalog with prices
   - `httpbin.org` — JSON API endpoints
   - `wikipedia.org` — article content extraction
2. For each site, runs a targeted scraping flow (not the full LLM pipeline —
   direct Playwright + our helpers).
3. Asserts: output files exist, are valid JSON, have expected item counts.
4. Writes artifacts to `test-artifacts/` directory.
5. Prints pass/fail summary with timing.

---

## Non-goals (deferred)

- Full proxy/session rotation (Phase 5).
- Two-step action mode (Phase 2 follow-up).
- URL-as-ID extraction trick (Phase 4).
- Lightweight engine alternative (Lightpanda).
- History compaction / prompt compression.

---

## File change summary

| File | Change type |
|------|-------------|
| `src/browser/network-settle.ts` | **New** |
| `src/browser/page-stats.ts` | **New** |
| `src/browser/artifacts.ts` | **New** |
| `src/browser/action-cache.ts` | **New** |
| `src/validate/validator.ts` | **Modified** — enhanced error classification |
| `src/explore/explorer.ts` | **Modified** — use network-settle, page-stats, blocked detection |
| `src/types.ts` | **Modified** — add PageStats, ArtifactBundle, ActionCacheEntry types |
| `src/test/integration.ts` | **New** — integration test harness |
| `package.json` | **Modified** — add `test:integration` script |
| `tsconfig.json` | **Modified** — include test directory |
