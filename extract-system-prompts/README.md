# Claude system prompts as a git timeline

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

Anthropic publishes the history of system prompts used on claude.ai and the mobile apps at <https://platform.claude.com/docs/en/release-notes/system-prompts>. That page is a single monolithic markdown document grouped by model, and each model lists one or more dated revisions.

This folder extracts that document into a set of files and a git history shaped so that `git log`, `git diff`, and `git blame` become the primary interface for exploring how those prompts have evolved.

Browse the per-family history pages on GitHub:

- [`claude-opus.md` history](https://github.com/simonw/research/commits/main/extract-system-prompts/claude-opus.md) — Opus 3 → Opus 4.7
- [`claude-sonnet.md` history](https://github.com/simonw/research/commits/main/extract-system-prompts/claude-sonnet.md) — Sonnet 3.5 → Sonnet 4.6
- [`claude-haiku.md` history](https://github.com/simonw/research/commits/main/extract-system-prompts/claude-haiku.md) — Haiku 3 → Haiku 4.5
- [`latest-prompt.md` history](https://github.com/simonw/research/commits/main/extract-system-prompts/latest-prompt.md) — every prompt across every model

## What you get

For every `<section title="...">` block in the source document the extractor produces four artifacts:

1. **`<model-slug>-YYYY-MM-DD.md`** — one file per prompt revision. Created exactly once, in a commit whose author + committer date is the date from the heading. Good for permalinking a specific prompt at a specific moment.
2. **`<model-slug>.md`** — one file per model, overwritten with the latest prompt for that model on every new date. Run `git log -p extract-system-prompts/claude-sonnet-4-5.md` to watch a single model evolve.
3. **`claude-<family>.md`** — one file per model family (`claude-opus.md`, `claude-sonnet.md`, `claude-haiku.md`), overwritten with every prompt in that family in chronological order. Useful for watching how the Opus line has evolved from Opus 3 through Opus 4.7.
4. **`latest-prompt.md`** — a single file that receives every prompt for every model in chronological order. `git log -p extract-system-prompts/latest-prompt.md` is the full firehose.

Every commit is authored by `Claude <noreply@anthropic.com>` with both `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE` pinned to the prompt's own date. The commit subject always starts with the exact filename that commit modifies (e.g. `claude-opus.md: Claude Opus 4.7 — April 16, 2026`), so `git log --oneline` is unambiguous even when four commits share a faked timestamp.

## Numbers

- 14 models (Opus 3 through Opus 4.7, plus Sonnet and Haiku variants)
- 3 model families (Opus, Sonnet, Haiku)
- 26 prompt revisions
- 104 faked-date commits (4 per revision)
- Earliest: 2024-07-12 (Sonnet 3.5, Opus 3, Haiku 3)
- Latest: 2026-04-16 (Opus 4.7)

## How it works

`extract.py` parses `system-prompts.md` with two regexes: `^## (Claude .+)$` to find model headings and `<section title="([^"]+)">(.*?)</section>` to find dated blocks. Dates are parsed permissively to accept both `July 31st, 2025` and `August 5, 2025` style headings as well as abbreviated month names like `Sept` or `Nov`.

Entries are sorted by `(date, source_index)` so same-day revisions (e.g. the three August 5, 2025 prompts for Opus 4.1 / Opus 4 / Sonnet 4) land in the same order they appear in Anthropic's document. The minute field of the faked commit time is set to the ordinal index so git's topological order matches.

For each entry the script writes the per-prompt file, the per-model file, the per-family file, and `latest-prompt.md`, and runs four separate `git commit` invocations — one per file — with the faked date set via environment variables.

## Browsing the history

Show every prompt, newest first:

```bash
git log --pretty=format:"%ad  %s" --date=short -- extract-system-prompts/latest-prompt.md
```

Watch the Opus family evolve from 3 to 4.7:

```bash
git log --pretty=format:"%ad  %s" --date=short -- extract-system-prompts/claude-opus.md
```

Diff a specific model between two revisions:

```bash
git log --pretty=format:"%h %ad %s" --date=short \
    -- extract-system-prompts/claude-sonnet-4-5.md
# pick two hashes, then:
git diff <old>..<new> -- extract-system-prompts/claude-sonnet-4-5.md
```

Blame a particular line within a specific revision (each per-prompt file was only ever written once, so blame pins to the prompt's own date):

```bash
git blame extract-system-prompts/claude-opus-4-7-2026-04-16.md
```

## What's in this folder

- `extract.py` — the parser + commit script.
- `notes.md` — running notes from the investigation.
- `README.md` — this file.
- `.gitignore` — keeps `system-prompts.md` (the source dump) out of the commit, per the project AGENTS.md rule against checking in full copies of fetched material.
- 26 `claude-*-YYYY-MM-DD.md` per-prompt files.
- 14 `claude-*.md` per-model files.
- 3 `claude-opus.md` / `claude-sonnet.md` / `claude-haiku.md` per-family files.
- `latest-prompt.md` — the firehose.

## Reproducing

```bash
cd extract-system-prompts
curl -sS https://platform.claude.com/docs/en/release-notes/system-prompts.md \
    -o system-prompts.md
python3 extract.py
```

The script is idempotent in the sense that re-running it on the same source document will produce four new commits per entry (identical content, new SHAs). If you want to regenerate from scratch, reset the branch first.
