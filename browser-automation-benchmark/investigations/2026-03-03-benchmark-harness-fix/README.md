# Benchmark Harness Fix Report

## Scope

This investigation updated the benchmark harness so failed runs produce actionable records instead of opaque startup crashes.

## What changed

- Added preflight checks in `scripts/run_benchmark.py` for:
  - `agent-browser` executable and local socket runtime directory
  - Camoufox binary presence
  - Scrapling/Patchright browser availability
- Fixed the `agent-browser` HTML capture call from `get html` to `get html body`.
- Moved `agent-browser` to short local session names and a writable local socket directory.
- Switched the Camoufox launcher to Playwright Firefox with `executable_path=/home/kahtaf/.cache/camoufox/camoufox-bin`.
- Removed the persistent-profile launch path that was causing profile-lock style failures.
- Made subprocess logging deterministic:
  - `stdout.log`
  - `stderr.log`
  - `commands.log`
- Kept `results/attempts.json` and `results/summary.json` compatible while adding:
  - `failure_category`
  - `failure_reason`
  - `failure_stage`
  - summary-side setup/startup/site/extraction failure counts
- Added `--mode sanity|full` so the rerun sequence can gate the full benchmark on a short sanity pass.

## Clean sanity baseline

Command run:

```bash
cd /home/kahtaf/Documents/workspace/research/browser-automation-benchmark
python3 scripts/run_benchmark.py --mode sanity
```

Outcome counts by tool and site:

- `agent-browser/x`: 1 `crash/error` (`startup`, `daemon-start-failed`)
- `agent-browser/reddit`: 1 `crash/error` (`startup`, `daemon-start-failed`)
- `agent-browser/linkedin`: 1 `crash/error` (`startup`, `daemon-start-failed`)
- `agent-browser/instagram`: 1 `crash/error` (`startup`, `daemon-start-failed`)
- `camofox-browser/x`: 1 `crash/error` (`startup`, `browser-closed`)
- `camofox-browser/reddit`: 1 `crash/error` (`startup`, `browser-closed`)
- `camofox-browser/linkedin`: 1 `crash/error` (`startup`, `browser-closed`)
- `camofox-browser/instagram`: 1 `crash/error` (`startup`, `browser-closed`)
- `Scrapling/x`: 1 `crash/error` (`setup`, `preflight-failed`)
- `Scrapling/reddit`: 1 `crash/error` (`setup`, `preflight-failed`)
- `Scrapling/linkedin`: 1 `crash/error` (`setup`, `preflight-failed`)
- `Scrapling/instagram`: 1 `crash/error` (`setup`, `preflight-failed`)

## Full-run gate

The full benchmark was not run. The sanity pass still has setup/startup blockers for every tool family.

## Commit status

The sandbox cannot write to `/home/kahtaf/Documents/workspace/research/.git/`, so `git add` and `git commit` fail with `Permission denied` on `.git/index.lock`. The working-tree diff was saved to `repo.diff` in this folder instead.

## Remediation still needed on this machine

- Install Patchright Chromium:

```bash
python3 -m patchright install chromium
```

- Reinstall or repair `agent-browser` if daemon startup continues to fail:

```bash
agent-browser install
```

- Re-test Camoufox startup outside the harness if `browser-closed` persists:

```bash
camoufox path
```
