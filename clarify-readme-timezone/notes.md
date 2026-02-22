# Notes: README Timezone Investigation

## Goal

Confirm what time zone the times shown in the README headings use.

## Approach

1. Read the cog script in the root README.md that generates the headings
2. Trace how dates are obtained and formatted
3. Check actual git commit timezone offsets

## Findings

### How the dates are generated

The root `README.md` uses a `cog` script (inline Python between `[[[cog` / `]]]` markers) to auto-generate the list of research projects.

For each subdirectory, it runs:

```
git log --diff-filter=A --follow --format=%aI --reverse -- <dirname>
```

This retrieves the **author date** in ISO 8601 strict format (`%aI`), which includes the timezone offset of the commit author's local time. The `--reverse` flag means the first line is the oldest (first) commit.

The date string is parsed with:

```python
commit_date = datetime.fromisoformat(date_str.replace('Z', '+00:00'))
```

This preserves whatever timezone offset was embedded in the git author date.

The date is then formatted with:

```python
commit_date.strftime('%Y-%m-%d %H:%M')
```

This outputs the date/time **in the original timezone of the commit** — it does NOT convert to UTC or any other common timezone.

### Actual timezone offsets in the data

Across all directories:
- **30 commits** have offset `+00:00` (UTC)
- **9 commits** have offset `-08:00` (PST / US Pacific Standard Time)

So the times in the README headings are a **mix of UTC and PST** depending on how/where each commit was authored:

- `-08:00` commits appear to be from local machine operations (Pacific time)
- `+00:00` commits appear to be from GitHub web interface or CI/automated processes

### Examples

| Directory | Git author date | Offset | README shows |
|-----------|----------------|--------|--------------|
| webmcp-chrome-demo | 2026-02-22T10:20:23-08:00 | PST | 2026-02-22 10:20 |
| blog-header-alignment | 2026-02-19T13:56:21-08:00 | PST | 2026-02-19 13:56 |
| sqlite-chronicle-vs-history-json | 2026-02-15T16:31:50+00:00 | UTC | 2026-02-15 16:31 |
| go-rod-cli | 2026-02-09T19:10:21+00:00 | UTC | 2026-02-09 19:10 |

### Conclusion

The times are **not in a consistent timezone**. They reflect the git author date's original timezone, which is a mix of UTC (+00:00) and US Pacific Standard Time (-08:00).

### Resolution

Updated the cog script to normalize all times to UTC using `commit_date.astimezone(timezone.utc)` and added a note "*Times shown are in UTC.*" above the generated listing.
