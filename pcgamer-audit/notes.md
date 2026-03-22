# PCGamer Performance Audit - Investigation Notes

## URL Tested
https://www.pcgamer.com/software/kill-the-algorithm-in-your-head-lets-set-up-rss-readers-and-get-news-we-actually-want-in-2026/

## Methodology

1. Used `rodney` (Chrome automation CLI) to launch headless Chromium
2. Added a `network-log` command to rodney using Chrome DevTools Protocol (CDP) Network domain - this captures all network requests with actual transfer sizes (unlike the Resource Timing API which hides cross-origin sizes)
3. Performed three captures:
   - 60-second static capture (no scrolling) - `network-log-60s.json`
   - 120-second capture with progressive scrolling every 6 seconds - `network-log-120s-scroll.json`
   - HTML snapshot of the rendered page - `page.html`

## Key Findings

### HTML Document Size
- The HTML document itself is **1.5 MB** (1,501,673 bytes)
- Of that, **673 KB (44.8%)** is inline `<style>` tags (31 of them)
- Another **439 KB** is inline JavaScript (168 `<script>` blocks)
- The article content is maybe 10-15 KB of text

### Network Requests (60-second static capture)
- **431 total requests** in first 60 seconds
- **5.5 MB transferred** (compressed), **18.8 MB decoded**
- **82.6% of transfers are ad-tech/trackers** (4.6 MB of 5.6 MB)
- 382 of 431 requests completed in the first 10 seconds

### Resource Timing API Limitation
- Initially used `performance.getEntriesByType('resource')` which showed only 0.8 MB
- This is because cross-origin resources don't expose `transferSize` via Resource Timing API without `Timing-Allow-Origin` header
- Had to use CDP Network domain for accurate data

### Why Rodney Needed Modification
- Rodney didn't have any network monitoring capabilities
- Added `network-log` command that:
  - Enables CDP Network domain
  - Listens for NetworkRequestWillBeSent, NetworkResponseReceived, NetworkLoadingFinished, NetworkDataReceived
  - Captures URL, method, resource type, status, MIME type, encoded/decoded sizes, timing, initiator info
  - Optionally includes response headers for cache/compression analysis
  - Outputs JSON for post-processing

### Headless vs Firefox Difference
- Headless Chrome: ~6 MB transferred in 120s
- Firefox reported by user: 38 MB initial, 200 MB+ over 5 minutes
- Likely causes of difference:
  - Video ads don't autoplay in headless mode
  - Ad viewability tracking doesn't fire properly in headless
  - Ad refresh cycles (bordeaux system has 17+ dynamic slot hooks for right-hand rail)
  - Visible viewport triggers different ad targeting
  - JW Player video content may stream in visible browsers

### Things I Tried
- Running Resource Timing API first - didn't show cross-origin sizes
- Capturing with and without scrolling - scrolling added very little extra (155 new URLs, mostly small)
- Analyzing response headers for cache-control and compression
- Examining prebid.js configuration and GPT ad slot setup
- Counting DOM elements and ad containers
