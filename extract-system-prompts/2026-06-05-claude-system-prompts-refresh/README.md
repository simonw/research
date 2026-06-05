# Claude System Prompts Refresh, 2026-06-05

This refresh fetched Anthropic's current `system-prompts.md` document from:

`https://platform.claude.com/docs/en/release-notes/system-prompts.md`

The existing `extract.py` parser still worked, but running it directly would
have replayed all existing prompts and created duplicate commits. I added
`extract_incremental.py`, which reuses `extract.py` and skips any revision whose
dated output file already exists.

## Result

- Parsed prompt revisions in current source: 27
- Previously documented prompt revisions: 26
- New revision found: `Claude Opus 4.8 - May 28, 2026`
- Timeline commits added: 4

The new generated files are:

- `claude-opus-4-8-2026-05-28.md`
- `claude-opus-4-8.md`

The rolling files updated by the faked-date commits are:

- `claude-opus.md`
- `latest-prompt.md`

The root `README.md` was updated to report 15 models, 27 prompt revisions, 108
faked-date commits, and latest prompt date `2026-05-28`.

## Reproduce

```bash
cd extract-system-prompts
curl -sS https://platform.claude.com/docs/en/release-notes/system-prompts.md \
    -o system-prompts.md
uv run python 2026-06-05-claude-system-prompts-refresh/extract_incremental.py
```

`system-prompts.md` is ignored and should not be committed.
