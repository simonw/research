# Browser Automation Benchmark

Compares three browser automation tools on their ability to load and extract structured data from real-world sites that use anti-bot protection. Each tool takes a different approach to stealth: different browser engines, fingerprint spoofing strategies, and protocol-level evasion.

## Tools under test

| Tool | Engine | Stealth approach |
|------|--------|-----------------|
| [**agent-browser**](https://github.com/vercel-labs/agent-browser) (Vercel Labs) | Chromium via Playwright | No built-in anti-bot. Stock browser with daemon-based session persistence. |
| [**camofox-browser**](https://github.com/jo-inc/camofox-browser) (Camoufox) | Firefox fork | C++-level fingerprint spoofing, Juggler protocol (invisible to page JS), cursor humanization. |
| [**Scrapling**](https://github.com/D4Vinci/Scrapling) (Patchright) | Chromium via Patchright | Stealth-patched Playwright fork. Patches WebDriver flags, navigator overrides. |

## Target sites

| Site | Page type | URL |
|------|-----------|-----|
| X (Twitter) | Post | `https://x.com/jack/status/20` |
| Reddit | Post | `https://www.reddit.com/r/Python/comments/g53lxf/` |
| LinkedIn | Company | `https://www.linkedin.com/company/microsoft/` |
| Instagram | Profile | `https://www.instagram.com/instagram/` |
| example.com | Control | `https://example.com` |

## Quick start

### Prerequisites

```bash
pip install playwright patchright scrapling camoufox
agent-browser install
python3 -m patchright install chromium
```

### Cookies (optional)

For authenticated access, place Netscape-format cookie files in `cookies/`:

```
cookies/x.txt
cookies/reddit.txt
cookies/linkedin.txt
cookies/instagram.txt
```

Each file has one cookie per line in Netscape/curl format:

```
.x.com	TRUE	/	TRUE	1806505040	auth_token	<value>
```

### Run

```bash
python3 scripts/run_benchmark.py
```

By default the benchmark runs in **headed (headful) mode** when a display server is available. This is the recommended mode — some tools (e.g., Camoufox's cursor humanization) only work headed, and headed mode more closely resembles real browser usage to anti-bot systems.

To force headless mode:

```bash
python3 scripts/run_benchmark.py --headless
```

Each run creates a timestamped directory under `runs/`:

```
runs/2026-03-03_143022/
  artifacts/    # per-attempt logs, HTML, screenshots
  results/      # attempts.json, summary.json, preflight.json
```

Screenshots are captured at the end of every run (including failures), so you can visually inspect whether a browser was blocked or served a challenge page.

Options:

```bash
# Custom run name
python3 scripts/run_benchmark.py --name my-test

# Specific tools or sites only
python3 scripts/run_benchmark.py --tools agent-browser Scrapling --sites x reddit

# Multiple attempts (default: 5)
python3 scripts/run_benchmark.py --attempts 3

# Headless mode
python3 scripts/run_benchmark.py --headless

# Compare headed vs headless modes
python3 scripts/run_benchmark.py --compare-modes --attempts 3
```

## How it works

For each tool/site combination, the benchmark:

1. **Preflight** - checks that the tool binary, browser, and Python modules are installed
2. **Cookie import** - loads Netscape-format cookies from `cookies/<site>.txt` into the tool's native format (storage state for agent-browser, `context.add_cookies()` for Camoufox, `StealthySession(cookies=...)` for Scrapling)
3. **Navigation** - opens the target URL with a 45s timeout, waits 6s for JS rendering
4. **Capture** - saves page HTML, screenshot, title, final URL, and logs
5. **Extraction** - runs a three-layer pipeline to pull structured data from the HTML:
   - **JSON-LD** (`<script type="application/ld+json">`) - most reliable when present
   - **Open Graph** (`og:title`, `og:description` meta tags) - good fallback
   - **Regex** - site-specific patterns as last resort
6. **Validation** - compares extracted fields against ground truth values (e.g., "just setting up my twttr" for Jack's tweet)
7. **Classification** - categorizes the outcome as success, partial, blocked, timeout, or crash

## Results

### Summary (headed mode, 3 attempts per tool/site, 2026-03-04)

| Tool | Site | Outcome | Success Rate | Correctness* | Nav Time (median) | Total Time (median) |
|------|------|---------|:------------:|:------------:|------------------:|--------------------:|
| agent-browser | x | partial | 0/3 | — | 10.6s | 13.8s |
| agent-browser | reddit | **success** | **3/3** | **100.0%** | 10.3s | 13.9s |
| agent-browser | linkedin | **success** | **3/3** | **100.0%**‡ | 9.5s | 13.6s |
| agent-browser | instagram | **success** | **3/3** | **100.0%** | 7.1s | 10.6s |
| agent-browser | control | **success** | **3/3** | **100.0%** | 6.8s | 9.8s |
| camofox-browser | x | **success** | **3/3** | **100.0%** | 10.8s | 13.4s |
| camofox-browser | reddit | **success** | **3/3** | **100.0%** | 9.6s | 12.4s |
| camofox-browser | linkedin | **success** | **3/3** | **100.0%**‡ | 12.0s | 16.2s |
| camofox-browser | instagram | **success** | **3/3** | **100.0%** | 9.8s | 12.7s |
| camofox-browser | control | **success** | **3/3** | **100.0%** | 9.0s | 10.4s |
| Scrapling | x | **success** | **3/3** | **100.0%** | 19.0s | 19.6s |
| Scrapling | reddit | **success** | **3/3** | **100.0%** | 19.3s | 19.9s |
| Scrapling | linkedin | **success** | **3/3** | **100.0%**‡ | 17.0s | 17.9s |
| Scrapling | instagram | **success** | **3/3** | **100.0%** | 15.7s | 16.2s |
| Scrapling | control | **success** | **3/3** | **100.0%** | 13.0s | 13.4s |

\* Correctness reported only for successful outcomes. Based on ground truth validation (e.g., "just setting up my twttr" for Jack's tweet).
‡ LinkedIn loads reCAPTCHA scripts as standard page infrastructure (not an active challenge). All expected fields were extracted successfully despite the passive block pattern in HTML.

### Navigation time comparison (median, successful sites only)

| Site | agent-browser | camofox-browser | Scrapling |
|------|:-------------:|:---------------:|:---------:|
| X (Twitter) | partial | **10.8s** | 19.0s |
| Reddit | 10.3s | **9.6s** | 19.3s |
| LinkedIn | **9.5s** | 12.0s | 17.0s |
| Instagram | **7.1s** | 9.8s | 15.7s |
| Control | **6.8s** | 9.0s | 13.0s |

Navigation time isolates page load + rendering, excluding agent-browser's CLI setup overhead (~1.4s) and post-capture steps. See Methodology for details.

### Interpretation

**No single tool wins everywhere.** Each tool has strengths and weaknesses depending on the site's anti-bot stack. All tools pass the control site (example.com) at 3/3, confirming correct setup.

**X (Twitter)** — Camofox and Scrapling both achieve 3/3 success with 100% ground truth, correctly extracting Jack's tweet text, author handle, and canonical URL. Agent-browser extracts 3/4 fields (author, timestamp, canonical URL) but consistently misses the tweet text, resulting in a "partial" classification. Camofox's navigation is fastest (median 10.8s vs 19.0s for Scrapling).

**Reddit** — All three tools achieve 3/3 success with 100% correctness, extracting post title, subreddit, author, and canonical URL from a live r/Python post. Earlier benchmark runs incorrectly classified Reddit as "blocked" — investigation revealed the prior target post had been deleted (see `reddit-block-investigation/README.md`). The `shreddit-forbidden` div that triggered block detection was Reddit's "content not found" page, not anti-bot blocking.

**LinkedIn** — All three tools achieve 3/3 success with 100% ground truth correctness. LinkedIn loads reCAPTCHA Enterprise scripts as standard page infrastructure, but does not actively block any of the tools — all expected fields (company name, location, page URL, metadata) are extracted successfully.

**Instagram** — All three tools achieve 3/3 success with 100% correctness. Agent-browser is the fastest here (nav median 7.1s), with full-document capture picking up JSON-LD data from `<head>`.

**Timing patterns** — agent-browser's total times include ~1.4s setup overhead (session prime + cookie import) and per-step CLI round-trips, but its navigation time is competitive. Scrapling consistently runs 1.5-2x slower on navigation than Camofox across all sites. Camofox is the fastest overall for navigation-heavy tasks.

**Consistency is high.** All outcomes were deterministic across 3 attempts — no flaky results. Standard deviations on navigation time are all under 1s. This suggests anti-bot decisions are primarily fingerprint/reputation-based, not probabilistic.

### Block analysis

**LinkedIn (false positive — now fixed):** LinkedIn loads `recaptcha` scripts as standard page infrastructure on every page load. The benchmark's block-pattern detector previously matched the word "captcha" in the HTML and classified the page as blocked *before* checking whether extraction succeeded. The classification logic has been reordered: extraction success now takes priority over passive block patterns. The `block_signals` field in each record documents what signals were present (e.g., `"block_pattern_in_html"`) even when the outcome is success, preserving transparency.

**Reddit (stale URL — now fixed):** The prior benchmark target (`/comments/10wxbk8/`) was a deleted weekly thread from January 2023. Reddit's `shreddit-forbidden` div is how the Shreddit frontend renders "content not found" — it is not an anti-bot signal. Replacing the URL with a live post (`/comments/g53lxf/`) resolved the issue entirely. See `reddit-block-investigation/` for full details.

## Methodology

### Multiple runs and statistics

By default the benchmark runs 5 attempts per tool/site combination (`--attempts 5`). The first attempt is cold (fresh profile/session), subsequent attempts are warm. A 2-second delay separates attempts. Summary statistics include mean, median, stdev, IQR, min, max, and p95 for timing metrics. Success rate is reported as a fraction (e.g., `4/5`).

### Timing breakdown

Each record includes separate timing fields:
- **`duration_s`** — wall-clock time from start to finish (includes all overhead)
- **`navigation_s`** — time spent on page navigation and rendering (the apples-to-apples metric for comparing tools)
- **`extraction_s`** — time spent running the extraction pipeline on captured HTML
- **`setup_s`** (agent-browser only) — time spent on session setup (prime-state + cookie import)
- **`step_timings`** (agent-browser only) — per-CLI-step timing breakdown

Agent-browser's CLI architecture requires multiple sequential commands where Camoufox and Scrapling use a single script. The `navigation_s` metric isolates the page load + wait time, making it the fair comparison across tools. `duration_s` reflects total end-to-end time including agent-browser's daemon overhead (which is sub-ms after the first call).

### Correctness reporting

Ground truth correctness is only reported for successful outcomes. Blocked pages often contain partial real content mixed with challenge elements, making correctness metrics misleading. The summary includes both `correctness_pct` (all runs) and `correctness_success_only_pct` (successful runs only).

### Control site

The benchmark includes `example.com` as a control site with no anti-bot protection. This establishes a baseline: if a tool fails on the control site, the issue is with the tool's setup rather than anti-bot detection.

### Content capture

Agent-browser captures full document HTML (`document.documentElement.outerHTML`) rather than just `<body>`, ensuring JSON-LD and Open Graph tags in `<head>` are available for extraction — matching how Camoufox (`page.content()`) and Scrapling (`response.text`) capture content.

### Headed vs headless comparison

Use `--compare-modes` to run the full benchmark in both headed and headless modes. Results are saved in separate subdirectories with a `mode_comparison.json` summarizing success rate and navigation time differences per tool/site.

## Output format

Each run produces:

- `results/preflight.json` - dependency checks for each tool
- `results/attempts.json` - detailed per-attempt records with extracted data, ground truth validation, timing breakdown, and failure classification
- `results/summary.json` - aggregated metrics per tool/site (success rate as fraction, timing stats with mean/median/p95, correctness for successful runs only)
- `artifacts/<tool>_<site>_<n>_<cold|warm>/` - raw logs (`stdout.log`, `stderr.log`, `commands.log`), captured HTML (`page.html`), screenshots (`screen.png`), and per-attempt record (`record.json`)

When using `--compare-modes`, each mode gets its own `headed/` and `headless/` subdirectory, plus a top-level `mode_comparison.json`.
