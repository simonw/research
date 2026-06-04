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
- Root cause: JW Player video carousel with 5 videos (68-136 MB at 720p/1080p) + VAST pre-roll video ads

### Forcing Video Player Initialization
- Added `--autoplay-policy=no-user-gesture-required` to Chrome launch flags in rodney
- JW Player initialized successfully and fetched playlist from `cdn.jwplayer.com/v2/playlists/egqep2zS`
- Player made VAST ad request to `securepubads.g.doubleclick.net/gampad/ads` (52 KB response)
- Started fetching MP4 from `videos-cloudfront.jwpsrv.com` but decoder failed in headless/container environment
- Confirmed video file sizes via HTTP HEAD requests: 15-136 MB depending on quality level
- 5 videos totaling 271.1 MB across all renditions

### Things I Tried
- Running Resource Timing API first - didn't show cross-origin sizes
- Capturing with and without scrolling - scrolling added very little extra (155 new URLs, mostly small)
- Analyzing response headers for cache-control and compression
- Examining prebid.js configuration and GPT ad slot setup
- Counting DOM elements and ad containers
- Forcing JW Player initialization and playback via JS
- Adding autoplay Chrome flag to enable video in headless mode
- HTTP HEAD requests to check video file sizes directly
