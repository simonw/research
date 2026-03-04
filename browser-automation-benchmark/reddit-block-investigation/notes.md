# Reddit Block Investigation — Notes

## 2026-03-03: Initial investigation

### Problem
The benchmark classifies Reddit as "blocked" 0/3 for all three tools. The HTML contains a `shreddit-forbidden` div, and the failure reason is `content-forbidden`. The working hypothesis was that Reddit's anti-bot system blocks all browser automation tools via IP reputation + reCAPTCHA Enterprise v3.

### Testing the target URL

**Target post:** `https://www.reddit.com/r/Python/comments/10wxbk8/whats_everyone_working_on_this_week/`

Curl tests against the target post:

1. **HTML fetch:** Returns `shreddit-forbidden` in the HTML body, page title is generic "Reddit - The heart of the internet" — no post content.
2. **JSON API (`.json` suffix):** Returns `{"message": "Not Found", "error": 404}`.
3. **API endpoint:** Returns `{"message": "Forbidden", "error": 403}`.
4. **old.reddit.com version:** Shows "page not found" and "archived".

### Testing Reddit in general (not blocked)

1. **Reddit homepage** (`https://www.reddit.com`): No `shreddit-forbidden`, loads fine.
2. **r/Python subreddit** (`https://www.reddit.com/r/Python/`): No `shreddit-forbidden`, content present.
3. **A live r/Python post** (`https://www.reddit.com/r/Python/comments/1rjsq84/...`): No `shreddit-forbidden`, full content loads.
4. **JSON API for live post:** Full data returned (title, author, score, comments).

### Root cause

The target post (`10wxbk8`, "What's everyone working on this week?", Jan 2023) is a **deleted weekly recurring thread** from over 3 years ago. Reddit's `shreddit-forbidden` div is how the new Shreddit frontend renders "content not found" — it is **not** an anti-bot mechanism.

### Key insight

`shreddit-forbidden` does NOT mean "you are blocked by anti-bot detection." It means "this content is unavailable" (deleted, removed, or otherwise inaccessible). The benchmark was testing against a dead URL.

### Fix

Replace the target URL with a live, high-visibility post:
- **New URL:** `https://www.reddit.com/r/Python/comments/g53lxf/lad_wrote_a_python_script_to_download_alexa_voice/`
- Top all-time r/Python post (12,339 upvotes, 133 comments)
- Live since April 2020 (~6 years)
- Confirmed accessible via JSON API and HTML
- Link post (no selftext body)

### What changed in config
- Replaced Reddit URL
- Removed `post_body` and `timestamp` from expected fields (link post has no selftext; timestamps not reliably exposed in Shreddit frontend)
- Updated ground truth to match new post
- Renamed `content-forbidden` failure reason to `content-unavailable` for clarity
