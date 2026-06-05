# Notes

## 2026-06-05

- Started in `/Users/simon/Dropbox/dev/research/extract-system-prompts`.
- Existing `extract.py` parses an ignored `system-prompts.md` file and writes four files per prompt revision: dated per-model, rolling per-model, rolling family, and `latest-prompt.md`.
- Important constraint: running `extract.py` directly would replay every old prompt and create duplicate commits, because it always commits every parsed entry.
- Existing latest extracted prompt is `Claude Opus 4.7 - April 16, 2026`.
- Fetched the live markdown from `https://platform.claude.com/docs/en/release-notes/system-prompts.md` into ignored `system-prompts.md`.
- Existing parser found 27 prompt revisions. The previous README recorded 26.
- New parsed entry: `Claude Opus 4.8 - May 28, 2026`.
- Added `extract_incremental.py` to skip existing dated prompt files and create normal four-file timeline commits only for missing revisions.
- Ran the helper with `uv run python 2026-06-05-claude-system-prompts-refresh/extract_incremental.py`.
- It created four faked-date commits for `Claude Opus 4.8 - May 28, 2026`: dated file, rolling model file, Opus family file, and `latest-prompt.md`.
- Re-ran the helper after the new dated file existed; it reported `Created 0 new prompt revision(s).`
- Updated the root README counts from 26 to 27 prompt revisions and from 104 to 108 faked-date commits.
