"""Append only new Claude system prompt revisions to the existing timeline.

This script reads the ignored ../system-prompts.md fetched from Anthropic,
reuses ../extract.py for parsing and formatting, and creates the same four
faked-date commits as the original extractor only for revisions whose dated
per-model file does not already exist.
"""

from pathlib import Path
import sys

BASE = Path(__file__).resolve().parents[1]
REPO_ROOT = BASE.parent
SOURCE = BASE / "system-prompts.md"

sys.path.insert(0, str(BASE))
import extract  # noqa: E402


def main() -> None:
    text = SOURCE.read_text(encoding="utf-8")
    entries = list(extract.parse_source(text))
    entries.sort(key=lambda e: (e[2], e[4]))

    created = 0
    for ordinal, (model_name, title, date_obj, content, _src_idx) in enumerate(entries):
        slug = extract.slugify_model(model_name)
        fslug = extract.family_slug(model_name)
        date_str = date_obj.strftime("%Y-%m-%d")
        dated_path = BASE / f"{slug}-{date_str}.md"
        if dated_path.exists():
            continue

        when = date_obj.replace(hour=12, minute=ordinal % 60, second=0)
        header = f"# {model_name} \u2014 {title}\n\n"
        body = header + content.rstrip() + "\n"
        descriptor = f"{model_name} \u2014 {title}"

        writes = [
            (dated_path, body, f"{dated_path.name}: {descriptor}"),
            (BASE / f"{slug}.md", body, f"{slug}.md: {descriptor}"),
            (BASE / f"{fslug}.md", body, f"{fslug}.md: {descriptor}"),
            (BASE / "latest-prompt.md", body, f"latest-prompt.md: {descriptor}"),
        ]

        for path, contents, subject in writes:
            path.write_text(contents, encoding="utf-8")
            extract.git_commit(path, subject, when, REPO_ROOT)

        print(f"created {date_str} {model_name}")
        created += 1

    print(f"Created {created} new prompt revision(s).")


if __name__ == "__main__":
    main()
