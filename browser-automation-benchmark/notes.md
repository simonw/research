# Notes

## 2026-03-02
- Created benchmark branch `benchmark/browser-automation-tools-2026-03-02` in `~/Documents/workspace/research`.
- Created folder `browser-automation-benchmark/` with `artifacts/`, `results/`, and `scripts/`.
- Verified tools:
  - agent-browser `0.15.1`
  - camoufox CLI present (`camoufox`, package `0.1.19`)
  - Scrapling Python package `0.4.1`
- Installed browser dependencies for agent-browser (`agent-browser install`) and validated camoufox binary path/permissions.
- Implemented benchmark runner script at `scripts/run_benchmark.py`.
- Executed full benchmark matrix (132 attempts): `python3 scripts/run_benchmark.py`.
- Generated aggregate outputs: `results/attempts.json` and `results/summary.json`.
- Wrote final report: `README.md`.
- Ran scheduled backup task: `/home/kahtaf/.openclaw/gitclaw/auto_backup.sh` (exit success, no stderr/stdout).
