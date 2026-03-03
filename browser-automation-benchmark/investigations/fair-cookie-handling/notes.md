# Notes


- Created investigation folder investigations/fair-cookie-handling and started tracing cookie flow in scripts/run_benchmark.py.
- Worktree already contains many modified benchmark artifacts from earlier runs; will avoid reverting unrelated changes.
- Read current runner implementations: camofox already uses context.add_cookies(parse_cookies(site)); agent-browser and Scrapling still need explicit import paths.
- Verified local agent-browser CLI exposes both storage-state loading and cookie set management.
- Confirmed Scrapling accepts a cookies session parameter at StealthySession construction; verifying internal application path now.
- Patched scripts/run_benchmark.py with shared cookie import logging, agent-browser storage-state priming, and Scrapling StealthySession(cookies=...) injection.
