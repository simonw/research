# Browser Automation Benchmark

## Current status

`scripts/run_benchmark.py` now does three things the old harness did not:

- runs a preflight check and writes `results/preflight.json`
- keeps `results/attempts.json` and `results/summary.json` schema-compatible while adding `failure_category`, `failure_reason`, and `failure_stage`
- preserves `stdout.log` and `stderr.log` for every attempt even on timeout or startup failure

The latest clean baseline was generated on `2026-03-03` with:

```bash
cd /home/kahtaf/Documents/workspace/research/browser-automation-benchmark
python3 scripts/run_benchmark.py --mode sanity
```

That sanity pass completed all `12` attempts, but it did **not** clear the full-run gate:

- `agent-browser`: 4 startup failures (`daemon-start-failed`)
- `camofox-browser`: 4 startup failures (`browser-closed`)
- `Scrapling`: 4 setup failures (`preflight-failed` because Patchright Chromium `1194` is missing)

Because sanity still has setup/startup failures, the full benchmark was not run.

## Install and repair commands for this machine

Run these from the repo root:

```bash
cd /home/kahtaf/Documents/workspace/research/browser-automation-benchmark
python3 -m pip install --user -U playwright patchright scrapling
python3 -m patchright install chromium
agent-browser install
camoufox path
```

If `camoufox path` does not print a valid binary path, install or refresh it:

```bash
camoufox fetch https://example.com
```

## Exact rerun steps

1. Clean previous benchmark outputs:

```bash
cd /home/kahtaf/Documents/workspace/research/browser-automation-benchmark
find artifacts -mindepth 1 -maxdepth 1 -exec rm -r {} +
find results -mindepth 1 -maxdepth 1 -type f -delete
find .ab -mindepth 1 -maxdepth 1 -delete
find .runtime -mindepth 1 -maxdepth 1 -delete
```

2. Run sanity:

```bash
python3 scripts/run_benchmark.py --mode sanity
```

3. Check whether sanity is clean enough for a full run:

```bash
python3 - <<'PY'
import json
rows = json.load(open('results/summary.json'))
bad = [row for row in rows if row['setup_failures'] or row['startup_failures']]
print(f'sanity_blockers={len(bad)}')
for row in bad:
    print(row['tool'], row['site'], row['failure_reasons'])
PY
```

4. Only if `sanity_blockers=0`, run the full benchmark:

```bash
python3 scripts/run_benchmark.py --mode full
```

## Outputs

- `results/preflight.json`: local dependency and binary checks
- `results/attempts.json`: per-attempt records
- `results/summary.json`: aggregated metrics, plus setup/startup/site failure counts
- `artifacts/<run_id>/`: raw logs and page artifacts for each attempt
