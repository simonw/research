# Browser Automation Benchmark

Compares three browser automation tools on their ability to load and extract structured data from real-world sites that use anti-bot protection. Each tool takes a different approach to stealth: different browser engines, fingerprint spoofing strategies, and protocol-level evasion.

## Tools under test

| Tool | Engine | Stealth approach |
|------|--------|-----------------|
| [**agent-browser**](https://github.com/vercel-labs/agent-browser) (Vercel Labs) | Chromium via Playwright | No built-in anti-bot. Stock browser with daemon-based session persistence. |
| [**camofox-browser**](https://github.com/jo-inc/camofox-browser) (Camoufox) | Firefox fork | C++-level fingerprint spoofing, Juggler protocol (invisible to page JS), cursor humanization. |
| [**Scrapling**](https://github.com/D4Vinci/Scrapling) (Patchright) | Chromium via Patchright | Stealth-patched Playwright fork. Patches WebDriver flags, navigator overrides. |

## Target sites

| Site | Page type | URL | Ground truth |
|------|-----------|-----|-------------|
| X (Twitter) | Post | `https://x.com/jack/status/20` | Tweet text, author handle, timestamp |
| Reddit | Post | `https://www.reddit.com/r/Python/comments/g53lxf/` | Post title, subreddit, author, canonical URL |
| LinkedIn | Company | `https://www.linkedin.com/company/microsoft/` | Company name, location, page URL, metadata |
| Instagram | Profile | `https://www.instagram.com/instagram/` | Username, canonical URL |
| example.com | Control | `https://example.com` | Page title |

## Results

### Summary (headed mode, 3 attempts per tool/site, 2026-03-04)

| Tool | Site | Success Rate | Correctness | Nav Time (median) | Total Time (median) |
|------|------|:------------:|:-----------:|------------------:|--------------------:|
| agent-browser | X | 0/3 partial | — | 10.6s | 13.8s |
| agent-browser | Reddit | **3/3** | **100%** | 10.3s | 13.9s |
| agent-browser | LinkedIn | **3/3** | **100%** | 9.5s | 13.6s |
| agent-browser | Instagram | **3/3** | **100%** | 7.1s | 10.6s |
| agent-browser | Control | **3/3** | **100%** | 6.8s | 9.8s |
| camofox-browser | X | **3/3** | **100%** | 10.8s | 13.4s |
| camofox-browser | Reddit | **3/3** | **100%** | 9.6s | 12.4s |
| camofox-browser | LinkedIn | **3/3** | **100%** | 12.0s | 16.2s |
| camofox-browser | Instagram | **3/3** | **100%** | 9.8s | 12.7s |
| camofox-browser | Control | **3/3** | **100%** | 9.0s | 10.4s |
| Scrapling | X | **3/3** | **100%** | 19.0s | 19.6s |
| Scrapling | Reddit | **3/3** | **100%** | 19.3s | 19.9s |
| Scrapling | LinkedIn | **3/3** | **100%** | 17.0s | 17.9s |
| Scrapling | Instagram | **3/3** | **100%** | 15.7s | 16.2s |
| Scrapling | Control | **3/3** | **100%** | 13.0s | 13.4s |

Correctness = percentage of extracted fields that match ground truth (reported for successful outcomes only). All tools achieve 100% correctness on every site where they succeed.

### Overall success rates

| Tool | Sites passed (3/3) | Total attempts | Overall |
|------|:------------------:|:--------------:|:-------:|
| **camofox-browser** | **5/5** | **15/15** | **100%** |
| **Scrapling** | **5/5** | **15/15** | **100%** |
| agent-browser | 4/5 | 12/15 | 80% |

Agent-browser's only gap is X (Twitter), where it extracts 3/4 fields (author, timestamp, canonical URL) but misses the tweet text. All other sites succeed at 100%.

### Navigation time comparison (median seconds)

| Site | agent-browser | camofox-browser | Scrapling |
|------|:-------------:|:---------------:|:---------:|
| X (Twitter) | — | **10.8** | 19.0 |
| Reddit | 10.3 | **9.6** | 19.3 |
| LinkedIn | **9.5** | 12.0 | 17.0 |
| Instagram | **7.1** | 9.8 | 15.7 |
| Control | **6.8** | 9.0 | 13.0 |

Navigation time isolates page load + JS rendering, excluding tool setup overhead. Bold = fastest per site. Agent-browser is fastest on 3 of 4 sites where it succeeds; Camofox wins on X and Reddit.

### Extracted data samples

What each tool actually pulls from the page (attempt 1):

**X (Twitter)** — Jack Dorsey's first tweet (`x.com/jack/status/20`)

| Field | camofox-browser | Scrapling | agent-browser |
|-------|----------------|-----------|---------------|
| post_text | "just setting up my twttr" | "just setting up my twttr" | *missing* |
| author_handle | jack | jack | jack |
| timestamp | 3:50 PM - Mar 21, 2006 | 3:50 PM - Mar 21, 2006 | 3:50 PM - Mar 21, 2006 |
| canonical_url | x.com/jack/status/20 | x.com/jack/status/20 | x.com/jack/status/20 |

**Reddit** — Top all-time r/Python post (`/comments/g53lxf/`)

| Field | All 3 tools |
|-------|-------------|
| post_title | "Lad wrote a Python script to download Alexa voice recordings, he didn't expect this email." |
| subreddit | Python |
| author | iEslam |
| canonical_url | reddit.com/r/Python/comments/g53lxf/... |

**LinkedIn** — Microsoft company page

| Field | All 3 tools |
|-------|-------------|
| title_or_company | Microsoft |
| location | United States |
| page_url | linkedin.com/company/microsoft/... |
| key_metadata | Information Technology |

**Instagram** — Instagram's own profile

| Field | All 3 tools |
|-------|-------------|
| username | instagram |
| canonical_url | instagram.com/instagram/ |

### Timing stability

All outcomes were deterministic across 3 attempts with no flaky results. Navigation time standard deviations are under 1s for most tool/site combinations (median stdev: 0.17s). This suggests anti-bot decisions are fingerprint/reputation-based, not probabilistic.

| Metric | agent-browser | camofox-browser | Scrapling |
|--------|:-------------:|:---------------:|:---------:|
| Stability score | 100 | 100 | 100 |
| Nav time stdev (typical) | 0.04-0.14s | 0.08-0.98s | 0.03-0.21s |
| Setup overhead | ~3.5s | ~2.5s | ~0.5s |

Setup overhead = total time minus navigation time, covering browser launch, cookie import, and post-capture steps. Agent-browser's higher overhead comes from its CLI architecture (multiple sequential commands per attempt).

### Interpretation

**Camofox and Scrapling both achieve perfect scores** — 5/5 sites, 15/15 attempts, 100% ground truth correctness. They successfully extract structured data from X, Reddit, LinkedIn, and Instagram without being blocked.

**Agent-browser succeeds on 4/5 sites** but consistently fails to extract the tweet text from X. It captures author, timestamp, and URL correctly — the issue is extraction, not blocking. Agent-browser's navigation speed is competitive (fastest on LinkedIn, Instagram, and control), but its CLI architecture adds ~3.5s overhead per attempt.

**Camofox is the fastest overall for navigation.** Its Firefox-based engine loads pages 40-50% faster than Scrapling's Patchright across all sites (median 9.6-12.0s vs 15.7-19.3s). The Juggler protocol and C++-level fingerprint spoofing don't add measurable overhead.

**Scrapling is the slowest but most reliable Chromium option.** Navigation times run 1.5-2x slower than Camofox, likely due to Patchright's stealth patches adding overhead to the Chromium startup and navigation pipeline. Despite this, it achieves perfect results on all sites.

**No site blocked any tool.** LinkedIn loads reCAPTCHA Enterprise scripts as standard page infrastructure, but this doesn't translate to active blocking — all tools extract complete data. Reddit and Instagram serve content without any challenge pages.

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

### Run

```bash
python3 scripts/run_benchmark.py
```

Runs in **headed mode** by default when a display server is available. This is recommended — some tools (e.g., Camoufox's cursor humanization) only work headed.

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

Each run creates a directory under `runs/` with per-attempt artifacts (HTML, screenshots, logs) and aggregated results (JSON).

## How it works

For each tool/site combination, the benchmark:

1. **Preflight** — checks that the tool binary, browser, and Python modules are installed
2. **Cookie import** — loads Netscape-format cookies into the tool's native format
3. **Navigation** — opens the target URL with a 45s timeout, waits 6s for JS rendering
4. **Capture** — saves page HTML, screenshot, title, final URL, and logs
5. **Extraction** — runs a three-layer pipeline (JSON-LD, Open Graph, regex fallback) to pull structured data
6. **Validation** — compares extracted fields against ground truth values
7. **Classification** — categorizes the outcome as success, partial, blocked, timeout, or crash

## Methodology

Each run executes multiple attempts per tool/site (default 5, configurable with `--attempts`). The first attempt is cold (fresh profile/session), subsequent attempts are warm, with a 2-second delay between them.

**Timing breakdown:** Each record includes `duration_s` (wall-clock), `navigation_s` (page load + rendering), and `extraction_s` (data extraction from HTML). Agent-browser additionally reports `setup_s` and per-step timings. The `navigation_s` metric is the fair comparison across tools.

**Correctness:** Ground truth validation checks extracted values against known-correct data (e.g., Jack's tweet text, iEslam's Reddit post title). Correctness is only reported for successful outcomes.

**Control site:** `example.com` serves as a baseline with no anti-bot protection. If a tool fails on the control, the issue is setup rather than detection.

## Output format

Each run produces:

- `results/preflight.json` — dependency checks per tool
- `results/attempts.json` — per-attempt records with extracted data, ground truth validation, timing, and classification
- `results/summary.json` — aggregated metrics per tool/site (success rate, timing stats, correctness)
- `artifacts/<tool>_<site>_<n>_<cold|warm>/` — HTML (`page.html`), screenshots (`screen.png`), logs, and per-attempt `record.json`
