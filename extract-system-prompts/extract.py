"""Extract per-prompt, per-model, per-family, and latest-prompt files.

Reads system-prompts.md (fetched separately with curl) and walks every
`## Claude <Model>` heading followed by one or more `<section title="...">`
blocks. For each (model, date, content) entry in chronological order this
script writes four files and commits each one separately with a faked
GIT_AUTHOR_DATE / GIT_COMMITTER_DATE derived from the section's date heading:

  1. <slug>-YYYY-MM-DD.md           — per-prompt dated file (created once).
  2. <slug>.md                      — per-model rolling file.
  3. claude-<family>.md             — per-family rolling file (opus/sonnet/haiku).
  4. latest-prompt.md               — firehose of every prompt for every model.

Each commit's subject starts with the exact filename that commit modifies, so
`git log --oneline` is unambiguous about which file any given commit touches.
"""

import os
import re
import subprocess
from datetime import datetime
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE = HERE / "system-prompts.md"

MONTHS = {
    "january": 1, "jan": 1,
    "february": 2, "feb": 2,
    "march": 3, "mar": 3,
    "april": 4, "apr": 4,
    "may": 5,
    "june": 6, "jun": 6,
    "july": 7, "jul": 7,
    "august": 8, "aug": 8,
    "september": 9, "sept": 9, "sep": 9,
    "october": 10, "oct": 10,
    "november": 11, "nov": 11,
    "december": 12, "dec": 12,
}

DATE_RE = re.compile(
    r"^\s*([A-Za-z]+)\s+(\d{1,2})(?:st|nd|rd|th)?,\s+(\d{4})\s*$"
)
MODEL_HEADING_RE = re.compile(r"^## (Claude .+?)\s*$", re.MULTILINE)
SECTION_RE = re.compile(
    r'<section title="([^"]+)">\s*(.*?)\s*</section>',
    re.DOTALL,
)
FAMILY_RE = re.compile(r"^Claude\s+(Opus|Sonnet|Haiku)\b", re.IGNORECASE)


def parse_date(title: str) -> datetime:
    m = DATE_RE.match(title)
    if not m:
        raise ValueError(f"Unrecognized date format: {title!r}")
    month_name, day, year = m.groups()
    month = MONTHS[month_name.lower()]
    return datetime(int(year), month, int(day))


def slugify_model(name: str) -> str:
    # "Claude Opus 4.7" -> "claude-opus-4-7"
    s = name.strip().lower()
    s = s.replace(".", "-")
    s = re.sub(r"\s+", "-", s)
    return s


def family_slug(name: str) -> str:
    # "Claude Opus 4.7" -> "claude-opus"
    m = FAMILY_RE.match(name.strip())
    if not m:
        raise ValueError(f"Unrecognized model family: {name!r}")
    return f"claude-{m.group(1).lower()}"


def parse_source(text: str):
    """Yield (model_name, date_title, date_obj, content, source_index)."""
    headings = list(MODEL_HEADING_RE.finditer(text))
    spans = []
    for i, h in enumerate(headings):
        start = h.end()
        end = headings[i + 1].start() if i + 1 < len(headings) else len(text)
        spans.append((h.group(1), start, end))

    source_index = 0
    for model_name, start, end in spans:
        chunk = text[start:end]
        for sm in SECTION_RE.finditer(chunk):
            title = sm.group(1)
            content = sm.group(2)
            date_obj = parse_date(title)
            yield model_name, title, date_obj, content, source_index
            source_index += 1


def run(cmd, env=None, cwd=None):
    return subprocess.run(
        cmd, cwd=cwd, env=env, check=True, capture_output=True, text=True,
    )


def git_commit(path: Path, subject: str, when: datetime, repo_root: Path):
    iso = when.strftime("%Y-%m-%dT%H:%M:%S")
    env = os.environ.copy()
    env["GIT_AUTHOR_DATE"] = iso
    env["GIT_COMMITTER_DATE"] = iso
    env["GIT_AUTHOR_NAME"] = "Claude"
    env["GIT_AUTHOR_EMAIL"] = "noreply@anthropic.com"
    env["GIT_COMMITTER_NAME"] = "Claude"
    env["GIT_COMMITTER_EMAIL"] = "noreply@anthropic.com"

    rel = path.relative_to(repo_root)
    run(["git", "add", "--", str(rel)], cwd=repo_root)
    run(["git", "commit", "-m", subject], env=env, cwd=repo_root)


def main():
    repo_root = HERE.parent
    text = SOURCE.read_text(encoding="utf-8")
    entries = list(parse_source(text))
    entries.sort(key=lambda e: (e[2], e[4]))

    for ordinal, (model_name, title, date_obj, content, _src_idx) in enumerate(entries):
        slug = slugify_model(model_name)
        fslug = family_slug(model_name)
        date_str = date_obj.strftime("%Y-%m-%d")
        when = date_obj.replace(hour=12, minute=ordinal % 60, second=0)

        header = f"# {model_name} — {title}\n\n"
        body = header + content.rstrip() + "\n"

        descriptor = f"{model_name} — {title}"

        writes = [
            (HERE / f"{slug}-{date_str}.md", body,
             f"{slug}-{date_str}.md: {descriptor}"),
            (HERE / f"{slug}.md", body,
             f"{slug}.md: {descriptor}"),
            (HERE / f"{fslug}.md", body,
             f"{fslug}.md: {descriptor}"),
            (HERE / "latest-prompt.md", body,
             f"latest-prompt.md: {descriptor}"),
        ]

        for path, contents, subject in writes:
            path.write_text(contents, encoding="utf-8")
            git_commit(path, subject, when, repo_root)

        print(f"[{ordinal:02d}] {date_str}  {model_name}")

    print(f"\nProcessed {len(entries)} prompt(s).")


if __name__ == "__main__":
    main()
