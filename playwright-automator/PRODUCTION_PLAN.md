# Productionization Plan: `playwright-automator` (CLI)

This repo is evolving from “record a session + ask an LLM for a script” into a **repeatable, testable automation generator**.

## Goals / Priorities

1) **Best‑effort extraction** even when API interception fails.
2) **Auth / 2FA** supported via headed runs + reusable browser auth state.
3) **Deterministic outputs**: versioned artifacts, replayable generation, regression tests.

## Key ideas we’re adopting (inspired by `unbrowse-openclaw`)

- Treat **HAR as ground truth** for what actually happened.
- Aggressively filter/suppress noise (static assets, third‑party domains, HTML navigations).
- Strip HTTP/2 **pseudo‑headers** (headers starting with `:`) before any replay.
- Prefer **API‑first extraction**, but keep a fallthrough ladder.

## Phased roadmap

### Phase 0 — Hardening baseline (done in this PR)

- Save Playwright **`storageState.json`** in each run bundle for better auth replay.
- Introduce a deterministic **IR** (`ir.json`) built from captured HAR requests:
  - endpoint grouping + normalization
  - heuristic scoring (JSON‑like, API‑like, frequency, de‑prioritize HTML)
- Feed the IR into the Gemini prompt so the model picks better endpoints.
- Add a small unit test harness (`npm test`).

### Phase 1 — Endpoint catalog → extraction ladder

Add an extractor interface and implement “try in order until sufficient data”:

1) **API intercept** (waitForResponse / page.on('response'))
2) **API replay** (re‑issue HAR-derived requests via `page.request.fetch` or in‑page `fetch`)
3) **GraphQL replay** (detect operationName/query hash, paginate)
4) **DOM/a11y fallback** (last resort)

### Phase 2 — Auth profiles + 2FA assist

- `auth-profiles/<domain>/<name>/storageState.json`
- `login` command: record only until user completes login/2FA, then save state.
- `doctor auth` command: check if still authenticated.
- `run --auth <profile>`: run automations with reusable state.

### Phase 3 — Deterministic codegen + regression suite

- Split generated output into a stable runner + generated extractor module(s).
- Pin prompt templates by version (`prompts/v1/...`), record generation inputs.
- Add golden tests: fixed `ir.json` fixtures → deterministic code output.

## Run bundle format (target)

Today the tool produces `runs/<run-id>/...`. The end-state is:

- `recording.har` — ground truth network traffic
- `storageState.json` — auth replay
- `ir.json` — deterministic endpoint catalog
- `session.json` / `actions.json` / `auth.json`
- `automation.ts` — generated runnable script
- `generation-info.json` — prompt version/model + endpoint selection notes

