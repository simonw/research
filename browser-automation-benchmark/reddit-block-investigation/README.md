# Reddit "Block" Investigation

## Summary

The benchmark previously classified Reddit as **blocked (0/3)** for all three browser automation tools. Investigation revealed this was **not** anti-bot blocking — the target Reddit post had been deleted/removed.

## Findings

### The target post is dead

The benchmark targeted `https://www.reddit.com/r/Python/comments/10wxbk8/whats_everyone_working_on_this_week/`, a weekly recurring thread from January 2023. This post has been deleted or removed by moderators.

Evidence:
- The post's JSON API returns `{"message": "Not Found", "error": 404}`
- The old.reddit.com version shows "page not found" and "archived"
- The new Reddit frontend renders a `shreddit-forbidden` div — this is how Shreddit displays "content not found", not an anti-bot signal

### Reddit is NOT blocking these tools

Curl and browser tests against live Reddit content show no blocking:

| Target | Result |
|--------|--------|
| Reddit homepage | Loads fine, no `shreddit-forbidden` |
| r/Python subreddit | Loads fine, content present |
| Live r/Python post (`1rjsq84`) | Full content, no blocking |
| Live r/Python post JSON API | Full data returned |
| Dead target post (`10wxbk8`) | `shreddit-forbidden`, 404 |

### What `shreddit-forbidden` actually means

Reddit's new frontend ("Shreddit") uses `<shreddit-forbidden>` as a web component to render "this content is unavailable." It appears for:
- Deleted posts
- Removed posts
- Posts from quarantined/banned subreddits (when not logged in)
- Possibly private subreddit content

It does **not** indicate anti-bot detection. The benchmark's block-pattern detector matched this string and incorrectly classified it as a site block.

## Fix

Replaced the dead target URL with a live, high-visibility post:

| | Old | New |
|---|---|---|
| URL | `.../comments/10wxbk8/...` | `.../comments/g53lxf/...` |
| Title | "What's everyone working on this week?" | "Lad wrote a python script to download Alexa voice recordings..." |
| Status | Deleted (404) | Live (top all-time r/Python, 12.3k upvotes) |
| Age | Jan 2023 (~3 years) | April 2020 (~6 years, stable) |
| Type | Self-post (weekly thread) | Link post |

Also renamed the `content-forbidden` failure reason to `content-unavailable` for clarity — it represents genuinely missing content, not anti-bot blocking.

## Impact on benchmark results

With the corrected URL, Reddit should show **success** for the browser automation tools, not "blocked." The previous README analysis about Reddit using "IP/reputation-based" blocking was incorrect — the benchmark was simply requesting content that no longer exists.
