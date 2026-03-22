# PCGamer Article Performance Audit

**URL:** https://www.pcgamer.com/software/kill-the-algorithm-in-your-head-lets-set-up-rss-readers-and-get-news-we-actually-want-in-2026/

**Date:** 2026-03-22

**User report:** Page loads 38+ MB initially in Firefox, grows to 200+ MB transferred after 5 minutes of idle time.

## Executive Summary

This article page is extraordinarily bloated. Reading an article about RSS readers — essentially a few thousand words of text — triggers **431 network requests** in the first 60 seconds, transferring **5.5 MB compressed (18.8 MB decoded)** in headless Chrome. In a real browser with visible ads and video, the numbers would be substantially higher.

**82.6% of all transferred bytes are ad-tech and tracking scripts.** The page contacts at least **54 distinct third-party domains** for advertising, analytics, identity resolution, and user tracking.

## The Numbers

| Metric | Value |
|--------|-------|
| HTML document size | 1.5 MB (1,501,673 bytes) |
| Inline CSS in HTML | 673 KB (44.8% of HTML) |
| Inline JavaScript in HTML | 439 KB |
| External script tags | 83 |
| Total `<script>` blocks | 168 |
| Iframes | 21 |
| Ad containers (Bordeaux) | 54 |
| Google Ad containers | 11 |
| Hawk affiliate widgets | 34 |
| GPT ad slots | 6 |
| Prebid bidders per slot | 10-12 |
| Total DOM nodes | 1,627 |
| Page height | 22,685 px (28.4 viewport heights) |
| JS heap usage | 64 MB |

## Where The Weight Comes From

### 1. The HTML Document Itself: 1.5 MB

The raw HTML is 1.5 MB for a text article. This is because:

- **673 KB (44.8%) is inline CSS** across 31 `<style>` tags. The CSS appears to include the entire site's stylesheet inlined into the page, rather than served as a cached external file.
- **439 KB is inline JavaScript** across 168 `<script>` blocks. This includes ad configuration, analytics setup, tracking pixels, and various initialization code.
- **6 JSON-LD structured data blocks** for SEO
- The actual article text content is roughly 10-15 KB

The HTML alone, compressed via Brotli at ~5.7:1 ratio, transfers as ~247 KB — but that's still enormous for what's essentially a blog post.

### 2. Ad-Tech and Tracking: 82.6% of All Transfers

In a 60-second capture, **326 of 431 requests (75.6%) went to ad-tech/tracking services**, accounting for **4.6 MB of 5.6 MB transferred (82.6%)**. Decoded (decompressed), that's **15.6 MB of 19.3 MB**.

The top ad-tech categories by transfer size:

| Category | Requests | Transfer (KB) | Decoded (KB) |
|----------|----------|--------------|--------------|
| Google Ads/GPT (DoubleClick, googlesyndication) | 61 | 1,254 | 3,004 |
| Google Tag Manager (3 separate containers) | 15 | 569 | 1,732 |
| Hawky (Future's commerce/affiliate platform) | 11 | 535 | 2,392 |
| Bordeaux (Future's ad management system) | 17 | 345 | 1,091 |
| DoubleVerify (ad verification) | 31 | 322 | 1,550 |
| Permutive (data management platform) | 8 | 251 | 1,210 |
| Privacy/consent management | 11 | 209 | 1,062 |
| TikTok Analytics | 5 | 154 | 631 |
| Facebook Pixel | 3 | 153 | 579 |
| WebContentAssessor | 1 | 104 | 355 |
| Marfeel SDK | 3 | 90 | 369 |
| Amazon Publisher Services | 8 | 89 | 361 |
| JW Player | 2 | 70 | 209 |
| Plus 41 more ad-tech vendors... | ... | ... | ... |

### 3. The Advertising Supply Chain

The page runs a complex programmatic advertising setup:

**Prebid.js v10.23.0** runs header bidding auctions for each ad slot. Each of the 3 active ad units simultaneously solicits bids from **10-12 demand partners**:
- Criteo, The Trade Desk (TTD), AppNexus/Xandr, Ogury, PubMatic, TripleLift, Rubicon/Magnite, AMX, Index Exchange

The bidder timeout is 3,000ms — the page waits up to 3 seconds for all bidders to respond before serving ads.

**Google Publisher Tags (GPT)** manages 6 ad slots:
- Top leaderboard: 970x250, 970x90, 728x90, 980x240
- MPU1 (sidebar): 300x600, 300x250
- MPU2 (sidebar): 300x600, 300x250
- Sponsored-by: 6x6
- Skin: 1x1
- Overlay: 1x1

**Bordeaux** (Future PLC's proprietary ad management) creates 54 DOM containers, including 17 dynamic right-hand rail slot hooks that can load ads as the user scrolls.

**Amazon Publisher Services (APS)** runs a parallel bidding process alongside Prebid.

### 4. Tracking and Identity Resolution

The page participates in an extensive identity/cookie-syncing ecosystem. These iframe-based syncs happen on every page load:

| Service | Purpose |
|---------|---------|
| Amazon Ads (iu3) | Ad identity sync |
| Google reCAPTCHA | Bot detection |
| DoubleClick partner pixels | Cookie sync |
| Criteo sync frame | Cookie sync |
| Index Exchange (ixmatch) | Cookie sync |
| Rubicon/Magnite usync | Cookie sync |
| AppNexus/Xandr async usersync | Cookie sync |
| Presage/Ogury user-sync | Cookie sync |
| TripleLift sync | Cookie sync |
| PubMatic user sync | Cookie sync |
| AMX isync | Cookie sync |

Additionally, these tracking/analytics scripts load:
- **3 Google Tag Manager containers** (GTM-WHLXGS3C, GTM-WWBWRXL, GTM-T788TZFM)
- Facebook Pixel (2 separate pixel IDs: 248805657506588, 1422285942000622)
- TikTok Analytics
- Scorecard Research
- Lotame (DMP)
- DotMetrics
- BrandMetrics
- ML314/Weborama
- Permutive (DMP with a 249 KB script — the single largest third-party script)

### 5. The Cascade Effect

The **bordeaux.js** script alone triggers **109 downstream requests** (844 KB). It's the #1 initiator of network activity.

The waterfall of script-triggered-script loading:

| Initiator Script | Downstream Requests | Transfer |
|-----------------|-------------------|----------|
| bordeaux.js (Future ad manager) | 109 | 845 KB |
| The HTML page itself (parser) | 99 | 1,522 KB |
| webcontentassessor.com script | 35 | 1,368 KB |
| Google Tag Manager | 6 | 185 KB |
| TikTok analytics | 4 | 42 KB |
| Privacy/consent manager | 7 | 60 KB |

### 6. Duplicate/Redundant Requests

Several resources are fetched multiple times:

| URL | Times Fetched | Total KB |
|-----|--------------|----------|
| googlesyndication simgad (ad image) | 2x | 378 KB |
| Google activeview ufs_web_display.js | 4x | 293 KB |
| DoubleVerify dv-measurements.js | 3x | 285 KB |
| Google abg_lite.js | 4x | 34 KB |
| Rubicon usync.js | 2x | 11 KB |

### 7. Compression

| Encoding | Requests | Transfer KB | Decoded KB | Ratio |
|----------|----------|------------|------------|-------|
| Brotli (br) | 130 | 2,580 | 11,174 | 4.3x |
| Gzip | 105 | 1,631 | 5,535 | 3.4x |
| Zstandard | 5 | 593 | 1,813 | 3.1x |
| None (uncompressed) | 191 | 788 | 740 | 0.9x |

191 requests (44%) have no compression at all, though most of these are small tracking pixels and sync requests.

### 8. Caching

| Cache Setting | Request Count |
|--------------|--------------|
| No cache / no-store | 121 (28%) |
| Short cache (<1 hour) | 39 (9%) |
| Long cache (>1 hour) | 166 (39%) |
| Unknown/missing | 105 (24%) |

28% of requests explicitly disable caching, meaning they must be re-fetched on every page load. Combined with the 24% with no cache headers, over half the requests have poor or no caching.

## Why Firefox Shows 38 MB+ Initial and 200 MB+ Over Time

Our headless Chrome capture shows ~6 MB transfer in 120 seconds. The user's Firefox experience of 38 MB+ initial and 200 MB+ over 5 minutes is explained primarily by the **JW Player video carousel**.

### The Smoking Gun: JW Player Video Carousel

By forcing JW Player to initialize in headless Chrome, we discovered a carousel titled "PC Gamer In-article Player Playlist (THIS IS CAROUSEL)" containing **5 autoplaying videos** with 5 quality renditions each. We confirmed the actual file sizes via HTTP HEAD requests:

| Quality | Video 1 (59s) | Video 2 (53s) | Video 3 (44s) | Video 4 (50s) | Video 5 (59s) | Total |
|---------|--------------|--------------|--------------|--------------|--------------|-------|
| 180p | 3.8 MB | 2.8 MB | 2.4 MB | 3.0 MB | 3.0 MB | **15.0 MB** |
| 270p | 5.3 MB | 4.1 MB | 3.6 MB | 4.3 MB | 4.2 MB | **21.5 MB** |
| 406p | 8.2 MB | 5.3 MB | 4.8 MB | 5.9 MB | 5.9 MB | **30.1 MB** |
| 720p | 18.5 MB | 12.2 MB | 10.7 MB | 13.6 MB | 13.2 MB | **68.2 MB** |
| 1080p | 36.8 MB | 24.2 MB | 21.6 MB | 27.8 MB | 25.9 MB | **136.3 MB** |

**Total across all renditions: 271.1 MB**

In a real browser, the player autoplays and cycles through the playlist. Each video also triggers a VAST pre-roll ad request (we captured a 52 KB VAST response from `securepubads.g.doubleclick.net/gampad/ads`), which loads a video advertisement before each content video.

The math for 200+ MB over 5 minutes:
- **~6 MB** — Page, scripts, ads, tracking (first load)
- **~68-136 MB** — Video carousel content (720p or 1080p, all 5 videos = ~4.4 minutes total)
- **~10-75 MB** — Video pre-roll ads (5 videos × 2-15 MB per ad creative)
- **~1-5 MB** — Ongoing ad refresh, tracking beacons, cookie syncing

**Note:** The videos could not actually play in headless Chromium (codec/sandbox limitations in the container environment), but we confirmed JW Player initialized, made the VAST ad request, and began fetching the MP4 files — it just immediately failed at the decoder stage. The 5 Media requests to `videos-cloudfront.jwpsrv.com` appeared in the network log with only 0.1-0.4 KB each (aborted downloads).

### Other Contributing Factors

1. **Video pre-roll ads (VAST/VPAID):** Each of the 5 videos triggers a VAST ad request. Video ad creatives are typically 2-15 MB each.

2. **Ad refresh cycles:** Bordeaux creates 17 dynamic right-hand rail ad slot hooks. As the user scrolls or time passes, these refresh with new ads — each triggering a Prebid auction across 10-12 bidders.

3. **Viewability tracking:** DoubleVerify and other ad verification services continuously fire tracking pixels. In headless mode, most ads aren't rendered visually, so this tracking doesn't fully trigger.

4. **Continuous beacon/sync traffic:** Even in headless, we see periodic requests every 30-60 seconds to DoubleVerify, Permutive, Newsroom.bi, and various cookie-sync services.

5. **Hawk commerce widgets:** 34 Hawk affiliate elements load product data, images, and tracking. With scrolling in a real browser, more of these lazy-load.

6. **Recirculation/infinite scroll:** The page likely loads additional article recommendations and their images as the user scrolls, each with their own set of tracking.

## The Actual Article Content

Stripping away all the overhead, the actual editorial content of this page is:

- ~10-15 KB of article text
- ~4 article images (one hero image at 89 KB, a few smaller inline images)
- Total useful content: probably **~150 KB**

That means **the overhead-to-content ratio is approximately 37:1** in the headless capture (5.6 MB / 150 KB), and likely **over 250:1** in the real Firefox experience (38 MB / 150 KB).

## Third-Party Domain Inventory

The page contacts **82 unique domains** referenced in the HTML, and the network capture shows connections to many more. Here are all the ad-tech/tracking services identified:

**Ad Serving & Management:** Google Ads/GPT, DoubleClick, Bordeaux (Future), Google Publisher Tags, Adnami

**Header Bidding (Prebid.js):** Criteo, The Trade Desk, AppNexus/Xandr, Ogury, PubMatic, TripleLift, Rubicon/Magnite, AMX, Index Exchange, Amazon Publisher Services

**Ad Verification:** DoubleVerify, WebContentAssessor

**Data Management Platforms:** Permutive, Lotame

**Analytics/Tracking:** Google Tag Manager (x3), Google Analytics, Facebook Pixel (x2), TikTok Analytics, Scorecard Research, DotMetrics, BrandMetrics, ML314/Weborama, Newsroom.bi, StudioStack

**Identity/Cookie Sync:** ID5, LiveRamp/ATS, EUID, Zeotap, Demdex/Adobe, BidSwitch, Eyeota, AdForm, RichAudience

**SSP/Exchange Partners:** PubMatic, OpenX, SmartAdServer, Casale/MediaMath, AdMixer, 33Across, Outbrain

**Other:** Marfeel SDK, Freyr (Future), Champagne (Future), Privacy Manager, Viafoura (comments), JW Player, Skimlinks, Presage/Ogury, BTLoader, DNS-Finder, P-N.io, cpx.to

## Performance Metrics

| Metric | Value |
|--------|-------|
| Time to First Byte (TTFB) | 676 ms |
| DOM Interactive | 855 ms |
| DOM Content Loaded | 1,408 ms |
| Page Load Complete | 2,009 ms |
| JavaScript Heap | 64 MB |

The initial page load time of ~2 seconds is deceptively fast because many scripts load asynchronously. The real performance impact is the ongoing CPU and network consumption from hundreds of scripts running simultaneously.

## Files in This Audit

- `README.md` — This report
- `notes.md` — Investigation notes and methodology
- `network-log-60s.json` — Full network capture (60s, no scrolling, with response headers)
- `network-log-120s-scroll.json` — Network capture with scrolling simulation (120s)
- `rodney.diff` — Changes made to rodney to add `network-log` command
- `analyze.py` — Analysis script used to process the network captures
