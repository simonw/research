# README Timezone Clarification

## Problem

The times shown in the root README.md project headings (e.g., `2026-02-22 10:20`) did not specify a timezone and were inconsistent — some were in UTC and others in US Pacific time, depending on how each commit was authored.

## Investigation

The root README uses a `cog` script to auto-generate the project listing. For each subdirectory it runs:

```
git log --diff-filter=A --follow --format=%aI --reverse -- <dirname>
```

The `%aI` format returns the git **author date** in ISO 8601 format with the original timezone offset. The script parsed the date and formatted it with `strftime('%Y-%m-%d %H:%M')` without converting to a common timezone.

Across all 39 directories in the repo:

- **30** had author dates at `+00:00` (UTC) — from GitHub web merges or CI
- **9** had author dates at `-08:00` (US Pacific Standard Time) — from local commits

This meant the displayed times were a mix of two timezones with no indication of which was which.

## Fix

Two changes were made to the root `README.md`:

1. **Added a note** above the generated listing: *"Times shown are in UTC."*

2. **Updated the cog script** to normalize all commit dates to UTC using `datetime.astimezone(timezone.utc)`. The key change:

```python
date_formatted = commit_date.astimezone(timezone.utc).strftime('%Y-%m-%d %H:%M')
```

Previously PST dates like `2026-02-22T10:20:23-08:00` would display as `2026-02-22 10:20`. Now they correctly display as `2026-02-22 18:20` (UTC).
