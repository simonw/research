# Browser Automation Anti-Bot Benchmark Report

## 1) Test setup

- **Date/time window:** 2026-03-02 22:53–23:16 (America/Toronto)
- **Machine:** Pop!_OS 24.04 LTS, Linux 6.17.9, 8 vCPU, 7.6 GiB RAM
- **Network/proxy:** default local network, no explicit proxy configured
- **Session auth:** Netscape cookies supplied for X, LinkedIn, Instagram; Reddit unauthenticated
- **Tools tested:**
  - `agent-browser 0.15.1`
  - `camoufox` CLI `0.1.19` (Camoufox binary v135.0.1-beta.24)
  - `Scrapling 0.4.1`
- **Project location:** `~/Documents/workspace/research/browser-automation-benchmark/`
- **Branch:** `benchmark/browser-automation-tools-2026-03-02`

## 2) Benchmark methodology

### Targets and page types
- **X:** public post page (`https://x.com/jack/status/20`)
- **Reddit:** public post page (`https://www.reddit.com/r/Python/comments/10wxbk8/whats_everyone_working_on_this_week/`)
- **LinkedIn:** public company page (`https://www.linkedin.com/company/microsoft/`)
- **Instagram:** public profile page (`https://www.instagram.com/instagram/`)

### Expected fields
- **X:** post text, author handle, timestamp, canonical URL
- **Reddit:** title, body (if any), subreddit, author, timestamp, canonical URL
- **LinkedIn:** title/company, location, page URL, visible metadata
- **Instagram:** username, timestamp (if visible), canonical URL

### Run plan
For every `tool × site`:
- 1 cold run (fresh session/process)
- 10 warm runs (session reuse attempt)

Total attempts: `3 tools × 4 sites × 11 runs = 132` attempts.

### Outcome classes used
- `success`
- `blocked/challenged`
- `partial`
- `timeout`
- `crash/error`

## 3) Per-tool results

## agent-browser
- **Observed behavior:** all runs failed before extraction due CLI invocation mismatch (`get html` requires selector).
- **Failure mode:** `Missing arguments for: get html`.
- **Result:** 44/44 attempts ended in `crash/error`.
- **Setup friction:** extraction command interface mismatch in benchmark harness.

## camofox-browser
- **Observed behavior:** repeated browser startup/profile lock failures.
- **Failure mode:** launch context closes with `Firefox is already running, but is not responding` (profile lock behavior), plus timeout runs.
- **Result:** 36 `crash/error` + 8 `timeout` (44 total), 0 successful.
- **Setup friction:** persistent-profile warm strategy conflicted with Camoufox runtime/profile locking.

## Scrapling
- **Observed behavior:** all runs failed at browser startup.
- **Failure mode:** missing expected Chromium executable path (`~/.cache/ms-playwright/chromium-1194/...`).
- **Result:** 44/44 attempts `crash/error`, 0 successful.
- **Setup friction:** Patchright/Playwright browser binary mismatch for local environment.

## 4) Per-site results (all tools)

All four sites were **inconclusive** because no tool reached a successful extraction state in this run set.

- **X:** 0 success / 33 attempts
- **Reddit:** 0 success / 33 attempts
- **LinkedIn:** 0 success / 33 attempts
- **Instagram:** 0 success / 33 attempts

No meaningful anti-bot comparison (challenge-vs-success) could be established due pre-extraction failures.

## 5) Stability findings

- **agent-browser:** deterministic crash on every attempt (command-level harness failure), no drift.
- **camofox-browser:** unstable under repeated warm attempts with persistent profile; high crash + timeout.
- **Scrapling:** deterministic startup failure (missing browser binary), no drift.
- **Success degradation:** not measurable (0 baseline success).

## 6) Final ranking

Given this run's outcomes, ranking is only possible on **failure severity**, not scraping success:

1. **camofox-browser** (least-bad: some runs reached timeout instead of immediate crash)
2. **agent-browser** (deterministic harness crash)
3. **Scrapling** (deterministic environment/browser binary failure)

### Best by requested categories (this run)
- **Best overall:** inconclusive
- **Best anti-bot success:** tie at 0%
- **Fastest successful scraper:** none (no successful runs)
- **Most stable repeated local runs:** tie among deterministic-failure tools; practically inconclusive

## 7) Raw appendix

## Exact run command
```bash
cd ~/Documents/workspace/research/browser-automation-benchmark
python3 scripts/run_benchmark.py
```

## Artifacts and data
- Per-attempt artifacts: `artifacts/<tool>_<site>_<attempt>_<cold|warm>/`
  - `record.json`
  - `stdout.log` / `stderr.log`
  - when available: `screen.png`, `page.html`, `url.txt`, snapshots
- Attempt dataset: `results/attempts.json`
- Aggregated metrics: `results/summary.json`
- Worklog: `notes.md`

## Representative error snippets
- agent-browser: `Missing arguments for: get html` / `Usage: agent-browser get html <selector>`
- camofox-browser: `Firefox is already running, but is not responding... use a different profile`
- Scrapling: `Executable doesn't exist at ~/.cache/ms-playwright/chromium-1194/chrome-linux/chrome`

## Metric table (tool × site)
(From `results/summary.json`)

- Every pair had:
  - **Success Rate:** 0.00%
  - **Challenge/Block Rate:** 0.00% (no page-level challenge reached)
  - **Partial Rate:** 0.00%
  - **Data Completeness:** 0.00%
  - **Correctness Spot-Check:** 0.00%
- Stability scores generated by harness:
  - agent-browser: 12
  - camofox-browser: 12
  - Scrapling: 12

## Reproducibility notes
- This run is reproducible as a **failure-state baseline** under the same local environment.
- For a valid anti-bot comparison run, fix harness/runtime blockers first:
  1. agent-browser extraction command (`get html` usage)
  2. Camoufox warm-session profile strategy
  3. Scrapling/patchright browser binary installation and path alignment

## Uncertainty / limitations
- Results were dominated by local harness/runtime issues, not target-site anti-bot behavior.
- Therefore conclusions about anti-bot bypass performance are **inconclusive** for this run.
