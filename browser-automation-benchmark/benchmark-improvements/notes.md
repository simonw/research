# Benchmark Improvement Investigation Notes

## 2026-03-03

### What I reviewed

- Full benchmark script `scripts/run_benchmark.py` (979 lines)
- Existing results from sanity run (12 attempts, mostly partial/blocked)
- Three tool repos studied in depth:
  - Scrapling (D4Vinci/Scrapling) - Python, Patchright/Chromium-based stealth
  - agent-browser (vercel-labs/agent-browser) - Rust CLI + Node daemon, stock Playwright
  - camofox-browser (jo-inc/camofox-browser) - Camoufox Firefox fork, C++ level anti-detect

### Key findings from repo analysis

**Scrapling**
- Uses Patchright (stealth-patched Playwright fork) for Chromium automation
- Has `StealthySession` API that accepts cookies directly and user_data_dir for profiles
- Stealth features: patched WebDriver flags, navigator overrides, etc.
- No screenshot capture in the API - benchmark can't get screenshots from Scrapling runs
- The benchmark only captures `response.text` and `response.url`, missing title

**agent-browser**
- No built-in anti-bot - relies on external providers (Browserbase, Kernel) or custom binaries
- Daemon architecture means session persistence is natural
- Has `--state` flag for storage state import (how cookies are loaded)
- The benchmark uses `--headed` mode which is correct for fair comparison
- Snapshot system uses accessibility tree - could be used for smarter extraction

**camofox-browser**
- Firefox-based with C++ level fingerprint spoofing - hardest for sites to detect
- Uses Juggler protocol (invisible to page JS, unlike CDP)
- Has `humanize: true` option for realistic mouse movement
- Auto-dismisses cookie consent dialogs
- Framework hydration wait logic (readyState + network quiet + rAF)
- The benchmark launches raw Playwright against the Camoufox binary but doesn't use any of these features

### Problems identified in current benchmark

1. **Extraction regex is too brittle** - Scrapling gets 16-33% completeness because the regex patterns assume specific HTML structures that differ across tools' rendering
2. **No Reddit cookies** - wait, there ARE reddit cookies... but Reddit still blocks camofox. The cookie format may not map correctly to reddit.com domains
3. **Camoufox not using its stealth features** - benchmark launches via `p.firefox.launch(executable_path=...)` with no humanize, no proxy, no geo-matching
4. **No warm-up navigation** - tools go directly to target without building browsing history
5. **Hardcoded 6-second wait is arbitrary** - should wait for network idle or specific DOM elements
6. **Cookie expiry dates are in the past for some** - some cookie timestamps have already expired
7. **Scrapling missing screenshot/title capture** - unlike other tools
8. **No fingerprint detection test** - no baseline like Sannysoft or CreepJS
9. **Test order is deterministic** - always runs agent-browser first, then camofox, then Scrapling. Sites can rate-limit the IP
10. **No network condition normalization** - results vary with connection quality

### Modules built (phase 1)

- `scripts/config.py` - Centralized all constants (URLS, COOKIES_RAW, BLOCK_PAT, pattern lists, GROUND_TRUTH)
- `scripts/extractors/__init__.py` - Layered extraction: JSON-LD → OG meta → regex fallback
- `scripts/extractors/json_ld.py` - JSON-LD structured data extractor
- `scripts/extractors/opengraph.py` - Open Graph meta tag extractor
- `scripts/extractors/regex_fallback.py` - Site-specific regex patterns (last resort)
- `scripts/runners/__init__.py` - Placeholder for future runner modularization

### Integration work (phase 2)

1. **Fixed `extractors/__init__.py`**: Moved `from typing import Any` to line 2 (was at line 76 after the function that used it)
2. **Fixed `runners/__init__.py`**: Replaced broken imports (referencing non-existent modules) with docstring-only stub
3. **Fixed relative import**: Changed `from ..config import GROUND_TRUTH` to `from config import GROUND_TRUTH` since `scripts/` is on sys.path
4. **Integrated config.py**: Replaced all duplicated constants in run_benchmark.py with imports from config
5. **Replaced old extract()**: Deleted the inline regex-only extractor, now imports layered extractor from `scripts.extractors`
6. **Enhanced classify_page()**: Added `site` and `final_url` params; checks for login redirects (LOGIN_REDIRECT_PAT) and soft blocks (SOFT_BLOCK_INDICATORS)
7. **Wired ground truth**: `build_record()` now calls `validate_ground_truth()`, adding `ground_truth` field to every record. `summarize()` uses real ground-truth-based `correctness_pct` instead of duplicate completeness check
8. **Switched camofox to native Camoufox**: Using `camoufox.sync_api.Camoufox` with humanize=True instead of raw `playwright.sync_api`
9. **Fixed Scrapling empty page**: Increased wait to 6000ms, added `page_action` callback for direct `page.content()` capture, tries `response.html`/`response.body` as fallbacks
10. **Refined BLOCK_PAT**: Removed "enable javascript" (matches normal SPA noscript tags, not anti-bot)
11. **Added headless fallback**: Detects DISPLAY environment and falls back to headless mode when no display server is available

### Key extraction improvement: X/Twitter title parsing

X (Twitter) pages are heavy SPAs that don't include JSON-LD or OG meta tags in the initial HTML. The tweet text is only in the `<title>` tag: `jack on X: "just setting up my twttr" / X`. Added regex patterns in both `opengraph.py` and `regex_fallback.py` to parse this format:
- `<title>\s*(\w+)\s+on\s+X:` → author handle
- `:\s*[""](.+?)[""]\s*/\s*X</title>` → tweet text

Before: camofox/scrapling extracted CSS junk (`font-display: 'swap'`) as post_text
After: correctly extracts "just setting up my twttr" → 100% ground truth

### Sanity run results comparison

| Tool | Site | Before | After | GT% |
|------|------|--------|-------|-----|
| camofox | x | 0B page, CSS junk | 928KB, correct text | 100% |
| scrapling | x | 0B page | 912KB, correct text | 100% |
| agent-browser | linkedin | success | success | 50% |
| camofox | linkedin | 0B page | 2.2MB page, blocked | 100% |
| scrapling | linkedin | 0B page | 2.2MB page, blocked | 100% |
| scrapling | reddit | 0B page | 401KB, blocked | 66.7% |
| scrapling | instagram | 0B page | 1.6MB, partial | 100% |

### Benchmark simplification

Removed `--mode` flag and `attempt_plan()` function. The benchmark now always runs 1 attempt per tool/site pair (12 total: 3 tools x 4 sites). The sanity/full distinction with 10 warm repeats was unnecessary overhead.

### Final benchmark results (12 runs, headless mode)

| Tool | Site | Outcome | GT% | Page Size | Duration |
|------|------|---------|-----|-----------|----------|
| agent-browser | x | partial | 33.3% | 476KB | 14.8s |
| agent-browser | reddit | partial | 33.3% | 190KB | 9.4s |
| agent-browser | linkedin | **success** | 50.0% | 1.4MB | 13.8s |
| agent-browser | instagram | partial | 100.0% | 1.0MB | 12.3s |
| camofox-browser | x | **success** | **100.0%** | 828KB | 12.2s |
| camofox-browser | reddit | blocked | 66.7% | 400KB | 10.6s |
| camofox-browser | linkedin | blocked | 100.0% | 2.2MB | 14.7s |
| camofox-browser | instagram | partial | 100.0% | 1.6MB | 11.8s |
| Scrapling | x | **success** | **100.0%** | 925KB | 19.7s |
| Scrapling | reddit | blocked | 66.7% | 401KB | 14.5s |
| Scrapling | linkedin | blocked | 100.0% | 2.2MB | 18.2s |
| Scrapling | instagram | partial | 100.0% | 1.6MB | 16.0s |

Key observations:
- Camofox and Scrapling both achieve 100% ground truth on X (was CSS garbage before)
- All tools now produce non-empty HTML captures (Scrapling was 0 bytes before)
- Reddit and LinkedIn block camofox and scrapling (anti-bot challenges detected)
- agent-browser is the only tool that succeeds on LinkedIn
- Instagram is partial across all tools (missing timestamp field)
