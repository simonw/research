# Notes

## Plan

1. Fetch https://platform.claude.com/docs/en/release-notes/system-prompts.md with curl.
2. Parse the doc into a list of `(model, date, content)` entries — each `## Claude <Model>` heading followed by one or more `<section title="...">` blocks.
3. Sort entries chronologically (oldest first). Break ties using source-document order so same-date prompts land in a stable sequence.
4. For each entry, with faked `GIT_AUTHOR_DATE` / `GIT_COMMITTER_DATE`:
   - Write `<model-slug>-YYYY-MM-DD.md` and commit it (create-only).
   - Overwrite `<model-slug>.md` with that entry's prompt and commit it.
   - Overwrite `claude-<family>.md` (opus/sonnet/haiku) and commit it.
   - Overwrite `latest-prompt.md` and commit it.
5. This produces a git history where `git log <file>` on any per-model or per-family file shows the evolution of that slice of the prompt corpus over time, and `git log latest-prompt.md` shows every prompt across all models in chronological order.

## Observations while reading the source

- Model sections are H2 headings: `## Claude Opus 4.7`, `## Claude Sonnet 4.6`, etc.
- Each model may contain multiple `<section title="Month Day, Year">...</section>` blocks.
- Dates use varied formats: `April 16, 2026`, `July 31st, 2025`, `Sept 9th, 2024`, `Oct 22, 2024`, `Feb 24th, 2025`. Ordinal suffixes (`st`/`nd`/`rd`/`th`) and abbreviated month names (`Sept`, `Feb`, `Oct`, `Nov`) both appear.
- Content is literal markdown with escaped angle brackets like `\<claude_behavior\>`. Writing it verbatim so git diffs show the real edits, not my reformatting.
- The doc bolds changes between versions with `**...**`. Leaving that as-is so per-model diffs remain readable.

## Same-date ties

Several dates appear on multiple models:

- `August 5, 2025` — Opus 4.1, Opus 4, Sonnet 4
- `July 31st, 2025` — Opus 4, Sonnet 4
- `May 22nd, 2025` — Opus 4, Sonnet 4
- `January 18, 2026` — Opus 4.5, Haiku 4.5, Sonnet 4.5
- `November 19, 2025` — Haiku 4.5, Sonnet 4.5
- `October 22, 2024` — Sonnet 3.5 (`Oct 22nd, 2024`) and Haiku 3.5 (`Oct 22, 2024`)
- `July 12th, 2024` — Sonnet 3.5, Opus 3, Haiku 3

Using the source document order as a secondary sort key and encoding it into the commit time (12:00, 12:01, …) so commits on the same day come out in a deterministic order.

## Follow-up: family files and unambiguous commit subjects

- Added per-family rolling files: `claude-opus.md`, `claude-sonnet.md`, `claude-haiku.md`. Each receives every prompt in that family as a separate faked-date commit, so `git log -p claude-opus.md` tracks the Opus family across versions (3 → 4 → 4.1 → 4.5 → 4.6 → 4.7).
- Family derived from a simple regex on the model name: `Claude (Opus|Sonnet|Haiku)\b` → `claude-<family>`.
- Commit subjects now start with the literal filename being touched, followed by the model + date, e.g. `claude-opus-4-7-2026-04-16.md: Claude Opus 4.7 — April 16, 2026`. That makes `git log --oneline` unambiguous even when four commits land on the same faked timestamp.
- Count: four commits per entry × 26 entries = 104 faked-date commits.

## Decisions

- Folder path for outputs: `/home/user/research/extract-system-prompts/` (this folder).
- Do NOT commit the downloaded `system-prompts.md` — per AGENTS.md, don't check in full copies of fetched material. `.gitignore` keeps it out of commits.
- Four commits per prompt: per-prompt dated file, per-model rolling file, per-family rolling file, `latest-prompt.md`. All share the same faked timestamp so they cluster cleanly in `git log --date-order`.
- Author + committer set to `Claude <noreply@anthropic.com>` for the faked commits so they're clearly distinguishable from the real working commits.

## What went wrong / what I learned

- First pass I forgot that `<section>` tags aren't properly nested inside the document's own XML-escaped content — they're plain `<section>...</section>` blocks while the prompt bodies use escaped forms like `\<claude_behavior\>`. A simple regex on `<section title="...">` and `</section>` is safe.
- `git commit --date` sets only the author date; the committer date needs `GIT_COMMITTER_DATE` env var. Setting both so `git log --pretty=fuller` shows the faked date in both fields.
- Regenerating required `git reset --hard` back to the original branch tip before re-running, then force-pushing. Since the downloaded `system-prompts.md` is gitignored (never tracked), it survived the reset and the script could re-run without re-fetching.
