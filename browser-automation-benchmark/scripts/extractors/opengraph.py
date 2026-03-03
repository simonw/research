"""Extract data from Open Graph meta tags (og:title, og:url, etc.)."""

import re
from typing import Dict


def _parse_meta_tags(text: str) -> Dict[str, str]:
    """Extract all og: and meta property/name tags."""
    tags: Dict[str, str] = {}
    for match in re.finditer(
        r'<meta\s+(?:[^>]*?)'
        r'(?:property|name)=["\']([^"\']+)["\']'
        r'[^>]*?content=["\']([^"\']*)["\']'
        r'[^>]*/?>',
        text,
        re.I | re.S,
    ):
        tags[match.group(1).lower()] = match.group(2)

    # Also handle reversed attribute order: content before property
    for match in re.finditer(
        r'<meta\s+(?:[^>]*?)'
        r'content=["\']([^"\']*)["\']'
        r'[^>]*?(?:property|name)=["\']([^"\']+)["\']'
        r'[^>]*/?>',
        text,
        re.I | re.S,
    ):
        key = match.group(2).lower()
        if key not in tags:
            tags[key] = match.group(1)

    return tags


def extract_opengraph(site: str, text: str, url: str) -> Dict[str, str]:
    """Extract site-specific fields from OG and meta tags."""
    if not text:
        return {}

    tags = _parse_meta_tags(text)
    fields: Dict[str, str] = {}

    # Also grab <title> tag
    title_match = re.search(r"<title[^>]*>(.*?)</title>", text, re.I | re.S)
    html_title = title_match.group(1).strip() if title_match else ""

    og_title = tags.get("og:title", "")
    og_desc = tags.get("og:description", "")
    og_url = tags.get("og:url", "")
    og_type = tags.get("og:type", "")
    twitter_title = tags.get("twitter:title", "")
    twitter_desc = tags.get("twitter:description", "")

    if site == "x":
        fields["post_text"] = og_desc or twitter_desc
        # Try extracting from <title> pattern: 'author on X: "tweet text" / X'
        if not fields.get("post_text") and html_title:
            title_match = re.search(r':\s*["\u201c](.+?)["\u201d]\s*/\s*X', html_title)
            if title_match:
                fields["post_text"] = title_match.group(1)
        # Author from twitter:creator or twitter:site or <title>
        creator = tags.get("twitter:creator", "")
        if creator:
            fields["author_handle"] = creator.lstrip("@")
        elif html_title:
            author_match = re.search(r'^(\w+)\s+on\s+X:', html_title)
            if author_match:
                fields["author_handle"] = author_match.group(1)
        fields["canonical_url"] = og_url or url if "x.com" in (url or "") else ""

    elif site == "reddit":
        fields["post_title"] = og_title or twitter_title or html_title
        fields["post_body"] = og_desc or twitter_desc
        fields["canonical_url"] = og_url or (url if "reddit.com" in (url or "") else "")
        # Subreddit from og:url
        if og_url:
            m = re.search(r"r/([A-Za-z0-9_]+)", og_url)
            if m:
                fields["subreddit"] = m.group(1)

    elif site == "linkedin":
        fields["title_or_company"] = og_title or twitter_title or html_title
        fields["page_url"] = og_url or (url if "linkedin.com" in (url or "") else "")

    elif site == "instagram":
        fields["username"] = og_title or twitter_title
        fields["canonical_url"] = og_url or (url if "instagram.com" in (url or "") else "")

    elif site == "control_example":
        fields["title"] = og_title or html_title

    elif site == "control_httpbin":
        pass  # httpbin returns JSON, not HTML with meta tags

    elif site == "control_local":
        fields["post_text"] = og_desc
        fields["canonical_url"] = og_url

    return {k: v for k, v in fields.items() if v}
