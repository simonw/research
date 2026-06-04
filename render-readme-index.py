#!/usr/bin/env python3
"""Render README.md to the single HTML index page for GitHub Pages."""

import html
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path


def title_from_markdown(markdown):
    for line in markdown.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return "research"


def render_github_markdown(markdown):
    payload = {
        "text": markdown,
        "mode": "gfm",
        "context": os.environ.get("GITHUB_REPOSITORY", "simonw/research"),
    }
    headers = {
        "Accept": "application/vnd.github+json",
        "Content-Type": "application/json",
        "User-Agent": "simonw-research-pages-build",
        "X-GitHub-Api-Version": "2022-11-28",
    }
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"

    request = urllib.request.Request(
        "https://api.github.com/markdown",
        data=json.dumps(payload).encode("utf-8"),
        headers=headers,
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return response.read().decode("utf-8")
    except urllib.error.HTTPError as ex:
        message = ex.read().decode("utf-8", errors="replace")
        raise SystemExit(f"GitHub Markdown API failed: {ex.code} {message}") from ex


def page_template(title, body):
    escaped_title = html.escape(title, quote=True)
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{escaped_title}</title>
  <style>
    body {{
      margin: 0;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      line-height: 1.5;
      color: #1f2328;
      background: #ffffff;
    }}
    main {{
      box-sizing: border-box;
      max-width: 980px;
      margin: 0 auto;
      padding: 40px 24px;
    }}
    a {{ color: #0969da; text-decoration: none; }}
    a:hover {{ text-decoration: underline; }}
    h1, h2, h3 {{ line-height: 1.25; }}
    h1 {{ padding-bottom: 0.3em; border-bottom: 1px solid #d8dee4; }}
    h2 {{ margin-top: 1.5em; padding-bottom: 0.3em; border-bottom: 1px solid #d8dee4; }}
    h3 {{ margin-top: 1.4em; }}
    code {{
      padding: 0.2em 0.4em;
      border-radius: 6px;
      background: #f6f8fa;
      font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
      font-size: 85%;
    }}
    blockquote {{
      margin-left: 0;
      padding-left: 1em;
      color: #59636e;
      border-left: 0.25em solid #d0d7de;
    }}
    @media (max-width: 700px) {{
      main {{ padding: 24px 16px; }}
    }}
  </style>
</head>
<body>
  <main>
{body}
  </main>
</body>
</html>
"""


def main():
    if len(sys.argv) != 3:
        raise SystemExit("Usage: render-readme-index.py README.md output.html")

    input_path = Path(sys.argv[1])
    output_path = Path(sys.argv[2])
    markdown = input_path.read_text(encoding="utf-8")
    rendered = render_github_markdown(markdown)
    output_path.write_text(
        page_template(title_from_markdown(markdown), rendered),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
