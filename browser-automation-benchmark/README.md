# Browser Automation Benchmark

Compares three browser automation tools on their ability to load and extract structured data from real-world sites that use anti-bot protection. Each tool takes a different approach to stealth: different browser engines, fingerprint spoofing strategies, and protocol-level evasion.

## Tools under test

| Tool | Engine | Stealth approach |
|------|--------|-----------------|
| **agent-browser** (Vercel Labs) | Chromium via Playwright | No built-in anti-bot. Stock browser with daemon-based session persistence. |
| **camofox-browser** (Camoufox) | Firefox fork | C++-level fingerprint spoofing, Juggler protocol (invisible to page JS), cursor humanization. |
| **Scrapling** (Patchright) | Chromium via Patchright | Stealth-patched Playwright fork. Patches WebDriver flags, navigator overrides. |

## Target sites

| Site | Page type | URL |
|------|-----------|-----|
| X (Twitter) | Post | `https://x.com/jack/status/20` |
| Reddit | Post | `https://www.reddit.com/r/Python/comments/10wxbk8/` |
| LinkedIn | Company | `https://www.linkedin.com/company/microsoft/` |
| Instagram | Profile | `https://www.instagram.com/instagram/` |

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

Each run creates a timestamped directory under `runs/`:

```
runs/2026-03-03_143022/
  artifacts/    # per-attempt logs, HTML, screenshots
  results/      # attempts.json, summary.json, preflight.json
```

Options:

```bash
# Custom run name
python3 scripts/run_benchmark.py --name my-test

# Specific tools or sites only
python3 scripts/run_benchmark.py --tools agent-browser Scrapling --sites x reddit
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

### Summary (headless mode, single cold run per tool/site)

| Tool | Site | Outcome | Ground Truth | Page Size | Time |
|------|------|---------|:------------:|----------:|-----:|
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

### Interpretation

**No single tool wins everywhere.** Each tool has strengths and weaknesses depending on the site's anti-bot stack.

**X (Twitter)** - Camofox and Scrapling both achieve 100% ground truth, correctly extracting Jack's tweet from the `<title>` tag. Agent-browser only manages 33% because it picks up a wrong user handle from related content in the page. Camofox is fastest here (12.2s vs 19.7s for Scrapling), likely due to Firefox's lighter rendering of X's SPA compared to Chromium.

**Reddit** - The hardest site for all tools. Reddit's anti-bot system blocks both Camofox and Scrapling with challenge pages, despite their stealth features. Agent-browser avoids the block but only extracts partial data (33% ground truth). Reddit's detection appears to go beyond browser fingerprinting — it likely uses behavioral signals and IP reputation that no amount of fingerprint spoofing can bypass.

**LinkedIn** - Agent-browser is the only tool that gets through without being blocked. This is likely because LinkedIn's anti-bot is tuned more aggressively against Firefox-based browsers and known automation frameworks. Camofox and Scrapling both receive the full page content (2.2 MB, 100% ground truth on what they extract) but are still classified as blocked due to challenge markers in the HTML. This suggests LinkedIn serves a mix of real content and challenge elements.

**Instagram** - All three tools get partial results with 100% correctness on extracted fields. The missing field is `timestamp`, which Instagram doesn't expose in the initial page load for profile pages. This is a limitation of the test target, not the tools.

**Browser engine matters.** Chromium-based tools (agent-browser, Scrapling) and Firefox-based tools (Camofox) face different challenge profiles. LinkedIn blocks Firefox aggressively. Reddit blocks everything. X is permissive to stealth browsers but trips up stock Chromium on extraction quality.

**Headless mode limits stealth.** These results were collected in headless mode. Camofox's cursor humanization (`humanize=True`) only works in headed mode, so its stealth potential is underrepresented here. Running headed (with a display server) would likely improve Camofox's results on sites that detect headless browsers.

## Output format

Each run produces:

- `results/preflight.json` - dependency checks for each tool
- `results/attempts.json` - detailed per-attempt records with extracted data, ground truth validation, timing, and failure classification
- `results/summary.json` - aggregated metrics per tool/site (success rate, block rate, correctness, timing)
- `artifacts/<tool>_<site>_<n>_<cold|warm>/` - raw logs (`stdout.log`, `stderr.log`, `commands.log`), captured HTML (`page.html`), screenshots (`screen.png`), and per-attempt record (`record.json`)
