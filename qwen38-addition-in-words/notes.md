# Working notes

- 2026-09-30: Cloned `simonw/research` and reviewed its `AGENTS.md` contribution instructions.
- 2026-09-30: Chose `qwen38-addition-in-words` as the self-contained project directory.
- 2026-09-30: Scope is the two finalized Qwen3.8 27B conditions: the 5,070-case reasoning-disabled benchmark and the paired 169-case medium-reasoning benchmark. Excluded discarded pilots, Python caches, model weights, and unrelated experiments.
- 2026-09-30: Preserved the original root-level script/report names and `runs/` layout so the report builder continues to resolve its inputs without changes.
- 2026-09-30: Copied both finalized run archives, including frozen pairs, raw JSONL attempts, graded CSVs, summaries, charts, server logs, environment snapshots, and exact inference/postprocess revisions. Verified the copied trees match the source apart from excluded Python caches.
- 2026-09-30: Included the original reference images and the two colorblind-safe accuracy grids. Omitted discarded pilot/aborted runs, unrelated model comparisons, model weights, and a host-specific systemd finalizer.
- 2026-09-30: Added a repository-facing README covering the method, results, principal confound, artifact inventory, and reproduction commands.
- 2026-09-30: Packaging target is `simonw/research` `main` at commit `871941f812a4e6347da2114caa102f1555a3d8a2` (`Upgrade to Datasette 1.0a40`). No existing repository files were changed.
- 2026-09-30: Ran both copied runners' `self-test` commands successfully. Regenerated the paired report and accessible chart from inside the new folder; the report and chart remained byte-identical to the originals, while the report manifest refreshed its generation timestamp.
- 2026-09-30: Verified all README-relative links resolve. Recounted the archives: no-thinking has 5,070 pairs, attempts, and result rows with 1,195 correct; medium reasoning has 169 of each with 167 correct.
- 2026-09-30: Confirmed the folder contains 59 files, is approximately 24 MiB, has no Python cache files, and has no binary artifact larger than 2 MiB.
