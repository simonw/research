"""Extract data from JSON-LD (<script type="application/ld+json">)."""

import json
import re
from typing import Any, Dict, List


def _find_json_ld_blocks(text: str) -> List[Dict[str, Any]]:
    """Parse all JSON-LD blocks from HTML."""
    blocks = []
    for match in re.finditer(
        r'<script[^>]*type=["\']application/ld\+json["\'][^>]*>(.*?)</script>',
        text,
        re.I | re.S,
    ):
        try:
            data = json.loads(match.group(1))
            if isinstance(data, list):
                blocks.extend(data)
            else:
                blocks.append(data)
        except (json.JSONDecodeError, ValueError):
            continue
    return blocks


def _get_nested(obj: Any, *keys: str) -> str:
    """Safely traverse nested dicts/lists to find a string value."""
    for key in keys:
        if isinstance(obj, dict):
            obj = obj.get(key, "")
        elif isinstance(obj, list) and obj:
            obj = obj[0]
            if isinstance(obj, dict):
                obj = obj.get(key, "")
            else:
                return ""
        else:
            return ""
    return str(obj) if obj else ""


def extract_json_ld(site: str, text: str, url: str) -> Dict[str, str]:
    """Extract site-specific fields from JSON-LD."""
    if not text:
        return {}

    blocks = _find_json_ld_blocks(text)
    if not blocks:
        return {}

    fields: Dict[str, str] = {}

    if site == "x":
        for block in blocks:
            schema_type = block.get("@type", "")
            # Twitter/X uses SocialMediaPosting or Article
            if schema_type in ("SocialMediaPosting", "Article", "NewsArticle", "BlogPosting"):
                fields["post_text"] = fields.get("post_text") or block.get("articleBody", "") or block.get("text", "")
                author = block.get("author", {})
                if isinstance(author, dict):
                    handle = author.get("additionalName", "") or author.get("name", "")
                    fields["author_handle"] = handle.lstrip("@")
                elif isinstance(author, list) and author:
                    handle = author[0].get("additionalName", "") or author[0].get("name", "")
                    fields["author_handle"] = handle.lstrip("@")
                fields["timestamp"] = block.get("datePublished", "") or block.get("dateCreated", "")
                fields["canonical_url"] = block.get("url", "")
            # Also check for WebPage type
            if schema_type == "WebPage":
                fields["canonical_url"] = fields.get("canonical_url") or block.get("url", "")

    elif site == "reddit":
        for block in blocks:
            schema_type = block.get("@type", "")
            if schema_type in ("DiscussionForumPosting", "Article", "SocialMediaPosting", "Comment"):
                fields["post_title"] = block.get("headline", "") or block.get("name", "")
                fields["post_body"] = (block.get("articleBody", "") or block.get("text", ""))[:300]
                author = block.get("author", {})
                if isinstance(author, dict):
                    fields["author"] = (author.get("name", "") or author.get("url", "").split("/")[-1]).lstrip("u/")
                fields["timestamp"] = block.get("datePublished", "") or block.get("dateCreated", "")
                fields["canonical_url"] = block.get("url", "")
                # Extract subreddit from URL or forum
                forum = block.get("isPartOf", {})
                if isinstance(forum, dict):
                    forum_url = forum.get("url", "")
                    m = re.search(r"r/([A-Za-z0-9_]+)", forum_url)
                    if m:
                        fields["subreddit"] = m.group(1)
                if not fields.get("subreddit"):
                    canon = fields.get("canonical_url", "")
                    m = re.search(r"r/([A-Za-z0-9_]+)", canon)
                    if m:
                        fields["subreddit"] = m.group(1)

    elif site == "linkedin":
        for block in blocks:
            schema_type = block.get("@type", "")
            if schema_type in ("Organization", "Corporation", "Company"):
                fields["title_or_company"] = block.get("name", "")
                addr = block.get("address", {})
                if isinstance(addr, dict):
                    fields["location"] = addr.get("addressLocality", "") or addr.get("addressRegion", "")
                elif isinstance(addr, list) and addr:
                    fields["location"] = addr[0].get("addressLocality", "") or addr[0].get("addressRegion", "")
                fields["page_url"] = block.get("url", "")
                employees = block.get("numberOfEmployees", "")
                if isinstance(employees, dict):
                    fields["key_metadata"] = f"{employees.get('value', '')} employees"
                elif employees:
                    fields["key_metadata"] = str(employees)

    elif site == "instagram":
        for block in blocks:
            schema_type = block.get("@type", "")
            if schema_type in ("ProfilePage", "Person", "Organization"):
                fields["username"] = (
                    block.get("alternateName", "")
                    or block.get("name", "")
                    or _get_nested(block, "mainEntity", "alternateName")
                )
                # Instagram profile pages don't reliably expose timestamps
                fields["canonical_url"] = block.get("url", "")

    elif site == "control_local":
        for block in blocks:
            fields["post_text"] = block.get("articleBody", "") or block.get("text", "")
            author = block.get("author", {})
            if isinstance(author, dict):
                fields["author_handle"] = author.get("name", "")
            fields["timestamp"] = block.get("datePublished", "")
            fields["canonical_url"] = block.get("url", "")

    elif site == "control_example":
        for block in blocks:
            fields["title"] = block.get("name", "")

    return {k: v for k, v in fields.items() if v}
