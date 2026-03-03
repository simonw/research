# Browser Automation Benchmark: Improvements & Results

This investigation improved the browser automation benchmark that compares **agent-browser**, **camofox-browser** (Camoufox), and **Scrapling** across four target sites (X, Reddit, LinkedIn, Instagram).

## What changed

### Centralized configuration (`scripts/config.py`)
All duplicated constants (URLs, cookies, patterns, ground truth) were moved to a shared config module. The main script now imports from config instead of defining its own copies. The `BLOCK_PAT` regex was refined to avoid false positives on normal SPA noscript messages (e.g. "enable javascript").

### Layered extraction (`scripts/extractors/`)
Replaced the brittle regex-only `extract()` function with a three-layer extraction pipeline:

1. **JSON-LD** - Parses `<script type="application/ld+json">` blocks (structured, tool-agnostic)
2. **Open Graph** - Falls back to `og:title`, `og:description`, `og:url` meta tags
3. **Regex** - Last resort site-specific patterns

Also added **title-based extraction** for X/Twitter, which is a SPA that doesn't include JSON-LD or OG tags. The tweet text appears only in the `<title>` tag (e.g. `jack on X: "just setting up my twttr" / X`), which the old regex missed entirely.

### Ground truth validation
Each extraction result is now validated against known ground truth values defined in `config.py`. The `build_record()` function calls `validate_ground_truth()` to add a `ground_truth` field with per-field pass/fail and an overall `correctness_pct`. The `summarize()` function uses real ground-truth correctness instead of just checking whether fields are non-empty.

### Soft-block and login-redirect detection
`classify_page()` now detects:
- **Login redirects** - final URL contains `/login`, `/challenge`, `/consent`, etc.
- **Soft blocks** - page loaded but content degraded (page too small + missing required DOM elements)

### Native Camoufox library
Switched from launching raw Playwright against the Camoufox binary to using the native `camoufox.sync_api.Camoufox` context manager. This enables C++-level fingerprint spoofing and cursor humanization that the benchmark should be testing.

### Scrapling empty page fix
- Increased wait from 3000ms to 6000ms (matching other tools)
- Added a `page_action` callback that captures `page.content()` directly as a fallback
- Tries `response.html` and `response.body` attributes as alternatives to `response.text`

### Headless fallback
The script detects whether a display server is available (`DISPLAY`/`WAYLAND_DISPLAY`) and falls back to headless mode automatically. All three tools respect this setting.

### Simplified run model
Removed the `--mode sanity/full` distinction. The benchmark now runs 1 attempt per tool/site combination (12 total: 3 tools x 4 sites).

## Results (headless mode, no display server)

| Tool | Site | Outcome | Ground Truth | Page Size | Duration |
|------|------|---------|:------------:|----------:|---------:|
| agent-browser | x | partial | 33.3% | 476 KB | 14.8s |
| agent-browser | reddit | partial | 33.3% | 190 KB | 9.4s |
| agent-browser | linkedin | **success** | 50.0% | 1.4 MB | 13.8s |
| agent-browser | instagram | partial | 100.0% | 1.0 MB | 12.3s |
| camofox-browser | x | **success** | **100.0%** | 828 KB | 12.2s |
| camofox-browser | reddit | blocked | 66.7% | 400 KB | 10.6s |
| camofox-browser | linkedin | blocked | 100.0% | 2.2 MB | 14.7s |
| camofox-browser | instagram | partial | 100.0% | 1.6 MB | 11.8s |
| Scrapling | x | **success** | **100.0%** | 925 KB | 19.7s |
| Scrapling | reddit | blocked | 66.7% | 401 KB | 14.5s |
| Scrapling | linkedin | blocked | 100.0% | 2.2 MB | 18.2s |
| Scrapling | instagram | partial | 100.0% | 1.6 MB | 16.0s |

### Key improvements vs previous run

| Metric | Before | After |
|--------|--------|-------|
| Scrapling page captures | 0 bytes (all sites) | 401 KB - 2.2 MB |
| X tweet extraction | CSS junk (`font-display: 'swap'`) | Correct (`just setting up my twttr`) |
| Ground truth validation | Not implemented | Per-field pass/fail + correctness_pct |
| Camofox stealth features | None (raw Playwright) | humanize + native fingerprint spoofing |

### Observations

- **Camofox and Scrapling achieve 100% ground truth on X** - the layered extractor correctly parses the tweet from the `<title>` tag
- **Reddit blocks camofox and Scrapling** - anti-bot challenges detected in page content, even in headless mode
- **LinkedIn blocks camofox and Scrapling** - but agent-browser succeeds (likely because it uses Chromium, which LinkedIn doesn't challenge as aggressively)
- **Instagram is partial across all tools** - the `timestamp` field is missing because Instagram profiles don't expose timestamps in the initial page load
- **agent-browser on X is partial** - it extracts `author_handle=feta_verse` (wrong user, likely from related content) instead of `jack`
- **Headless mode** limits the stealth effectiveness of all tools, particularly camofox which relies on headed mode for cursor humanization

## Files modified

| File | Changes |
|------|---------|
| `scripts/run_benchmark.py` | Import config/extractors, delete duplicated constants and old extract(), enhance classify_page(), wire ground truth, rewrite camofox script, fix scrapling wait, add headless fallback, simplify run model |
| `scripts/extractors/__init__.py` | Fix `Any` import order, change relative import to absolute |
| `scripts/extractors/opengraph.py` | Add title-based extraction for X/Twitter |
| `scripts/extractors/regex_fallback.py` | Add title-based extraction for X/Twitter |
| `scripts/runners/__init__.py` | Replace broken imports with docstring-only stub |
| `scripts/config.py` | Refine BLOCK_PAT to remove false-positive triggers |
