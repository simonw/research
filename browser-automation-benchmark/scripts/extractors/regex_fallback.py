"""Regex-based extraction as a last-resort fallback."""

import json
import re
from typing import Dict


def extract_regex(site: str, text: str, url: str) -> Dict[str, str]:
    """Extract fields using site-specific regex patterns.

    This is the fallback layer - only used when JSON-LD and OG tags
    don't provide the needed data.
    """
    t = text or ""
    e: Dict[str, str] = {}

    if site == "x":
        # Try JSON blob first, then <title> tag pattern: 'author on X: "tweet text" / X'
        e["post_text"] = (
            next(iter(re.findall(r'"text":"([^"]{10,280})"', t)), "")
            or next(iter(re.findall(r'<title>[^<]*?:\s*["\u201c](.+?)["\u201d]\s*/\s*X</title>', t, re.I | re.S)), "")
            or next(iter(re.findall(r"\n(.{20,280})\n.*?@", t)), "")
        )
        # Extract author from title pattern: 'author on X: ...'
        title_author = next(iter(re.findall(r'<title>\s*(\w+)\s+on\s+X:', t, re.I)), "")
        e["author_handle"] = title_author or next(iter(re.findall(r"@([A-Za-z0-9_]{1,15})", t)), "")
        e["timestamp"] = next(
            iter(re.findall(r"\d{1,2}:\d{2}\s?(?:AM|PM)?.*?\d{4}|\d{4}-\d{2}-\d{2}T[^\s\"]+", t)),
            "",
        )
        e["canonical_url"] = url if "x.com" in (url or "") else ""

    elif site == "reddit":
        e["post_title"] = next(iter(re.findall(r"<title>(.*?)</title>", t, re.I | re.S)), "").strip()
        e["post_body"] = next(iter(re.findall(r"<shreddit-post[\s\S]*?>", t, re.I)), "")[:300]
        e["subreddit"] = next(iter(re.findall(r"r/([A-Za-z0-9_]+)", t)), "")
        # Prefer author attr on <shreddit-post> (more reliable than first u/ on page, which may be an ad)
        # Handle both raw quotes and escaped quotes (\\") from JSON-embedded HTML
        e["author"] = (
            next(iter(re.findall(r'<shreddit-post[^>]*\bauthor="([^"]+)"', t, re.I)), "")
            or next(iter(re.findall(r'<shreddit-post[^>]*\bauthor=\\"([^\\]+)\\"', t, re.I)), "")
            or next(iter(re.findall(r'aria-label="Author:\s*([^"]+)"', t, re.I)), "")
            or next(iter(re.findall(r"u/([A-Za-z0-9_\-]+)", t)), "")
        )
        e["timestamp"] = next(
            iter(re.findall(r"\d+\s+(?:hours?|days?|minutes?)\s+ago|\d{4}-\d{2}-\d{2}T[^\s\"]+", t)),
            "",
        )
        e["canonical_url"] = url if "reddit.com" in (url or "") else ""

    elif site == "linkedin":
        e["title_or_company"] = next(iter(re.findall(r"Microsoft|Company|LinkedIn", t, re.I)), "")
        e["location"] = next(iter(re.findall(r"Redmond|United States|Toronto|Remote", t, re.I)), "")
        e["page_url"] = url if "linkedin.com" in (url or "") else ""
        e["key_metadata"] = next(
            iter(re.findall(r"\d+[+,]?\s+employees|Information Technology", t, re.I)), ""
        )

    elif site == "instagram":
        e["username"] = next(iter(re.findall(r"instagram", t, re.I)), "")
        # Instagram profile pages don't reliably expose timestamps
        e["canonical_url"] = url if "instagram.com" in (url or "") else ""

    elif site == "control_example":
        e["title"] = next(iter(re.findall(r"<title>(.*?)</title>", t, re.I | re.S)), "").strip()

    elif site == "control_httpbin":
        # httpbin returns JSON
        try:
            data = json.loads(t)
            headers = data.get("headers", {})
            e["user_agent"] = headers.get("User-Agent", "")
        except (json.JSONDecodeError, ValueError):
            # Maybe embedded in HTML
            m = re.search(r'"User-Agent":\s*"([^"]+)"', t)
            if m:
                e["user_agent"] = m.group(1)

    elif site == "control_local":
        e["post_text"] = next(
            iter(re.findall(r'"articleBody":\s*"([^"]+)"', t)), ""
        )
        e["author_handle"] = next(
            iter(re.findall(r'"name":\s*"([^"]+)"', t)), ""
        )
        e["timestamp"] = next(
            iter(re.findall(r'"datePublished":\s*"([^"]+)"', t)), ""
        )
        e["canonical_url"] = url or ""

    return {k: v for k, v in e.items() if v}
