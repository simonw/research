# Playwright Automator — Development Notes

## Research Phase

### Analyzed unbrowse-openclaw (https://github.com/lekt9/unbrowse-openclaw)
- TypeScript plugin for the OpenClaw AI agent platform
- Core flow: visit website -> capture HAR -> parse API endpoints -> extract auth -> generate reusable "skill" package
- Key files studied:
  - `har-parser.ts` — HAR filtering (skip analytics, static assets, third-party domains), endpoint grouping, auth header extraction
  - `har-capture.ts` — Playwright `recordHar` option for full traffic capture
  - `auth-extractor.ts` — Auth method detection (Bearer, API Key, Cookie, CSRF, etc.)
  - `workflow-recorder.ts` — Multi-step session recording with navigation, API calls, DOM snapshots
  - `skill-generator.ts` — Generates SKILL.md, auth.json, and TypeScript API client
  - `types.ts` — HarEntry, ParsedRequest, ApiData interfaces

### Key Insights from Reference Repo
1. Playwright's `recordHar` option captures complete request/response data including headers, cookies, timing
2. HAR filtering is critical — need to skip analytics (Google, Facebook, etc.), static assets, and CDN requests
3. Auth detection works by pattern matching header names against known auth patterns
4. The reference repo has no direct LLM calls — it IS the LLM's tool. Our tool calls Gemini directly.

## Architecture Decisions

### HAR-First Strategy
- Use Playwright's native `recordHar: { mode: 'full' }` for complete traffic capture
- Full response bodies are captured, which is critical for LLM analysis
- The HAR file serves as the single source of truth for all API data

### API Interception Over DOM Scraping
- The Gemini prompt prioritizes `page.route()` / `page.waitForResponse()` over DOM scraping
- API data is more structured and reliable than scraped DOM content
- The generated scripts navigate to trigger API calls, then intercept responses

### Per-Run Folder Structure
Each recording creates a folder with:
- `recording.har` — Complete HAR file
- `session.json` — Full session metadata (actions, API requests, auth, cookies)
- `actions.json` — User interactions (clicks, types, navigation)
- `auth.json` — Extracted authentication data (headers, cookies, auth method)
- `automation.ts` — Generated Playwright script (if Gemini key provided)
- `generation-info.json` — Script generation metadata (strategy, targeted endpoints)
- `screenshots/` — Captured screenshots at key moments
- `run.sh` — Convenience script to run the automation

### Gemini Integration
- Uses `gemini-2.0-flash` for fast, cost-effective script generation
- Temperature set to 0.2 for deterministic code output
- Prompt includes: task description, recorded actions, HAR API summary, auth info
- Supports refinement: feed back errors and the script gets improved

## Implementation Notes

### Action Recording
- Injected via `context.addInitScript()` as a string (not a function) to avoid TypeScript issues with browser-context code
- Records: clicks (with CSS selector), input (debounced), key presses (Enter/Escape/Tab)
- Selector priority: id > data-testid > aria-label > name > placeholder > role > text > nth-of-type path

### HAR Analysis
- Adapted from unbrowse-openclaw's filtering logic
- Skip lists: 60+ analytics/tracking domains, static asset extensions, infrastructure paths
- Auth header detection: 30+ exact-match names + pattern-based matching
- Response bodies are truncated at 10KB for LLM context window management
- Arrays in responses are sampled (first 3 items) to save tokens

### Browser Compatibility
- Playwright 1.56.0 used to match available browser revision (chromium-1194)
- `--no-sandbox` and `--disable-dev-shm-usage` flags for container environments
- `--single-process` needed in some constrained environments (CI/containers)

## Testing

### E2E Test Results (19/19 passed)
- Test server with login, dashboard, messages pages + 5 API endpoints
- Full flow: server start -> browser launch -> HAR recording -> login -> navigation -> cookie capture -> HAR analysis -> session save
- Verified: HAR file creation, API request extraction, response body capture, auth detection, LLM summary generation

### Test Server API Endpoints
- `POST /api/login` — Cookie-based auth
- `GET /api/stats` — Dashboard statistics
- `GET /api/threads` — Message thread list
- `GET /api/messages` — All messages (paginated)
- `GET /api/users` — Contact list

## Challenges Encountered

1. **Playwright browser version mismatch** — npm playwright 1.58.x wanted chromium-1208 but only 1194 was installable. Fixed by pinning to 1.56.0.
2. **Container environment** — Chrome crashes in constrained containers without `--single-process` and `--disable-dev-shm-usage` flags.
3. **TypeScript in browser context** — `addInitScript()` with function caused TS errors for browser-only APIs. Switched to string-based injection.
4. **HAR file storage** — Playwright only flushes HAR on `context.close()`, so cookies must be captured before closing.
