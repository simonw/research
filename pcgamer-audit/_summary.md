A performance audit of the March 2026 PCGamer article on RSS readers reveals severe page bloat, with over 82% of network traffic and transferred bytes traced to ad-tech, tracking, and programmatic advertising scripts. Despite the core content consisting of just 10-15 KB of text and a handful of images (~150 KB total), the page triggers over 431 network requests and 5.5 MB of transfer (18.8 MB decoded) within 60 seconds—ballooning to 200+ MB in Firefox due to autoplay video carousels and continuous ad/analytics refreshes. The site's heavy inlined styles/JavaScript, extensive third-party integrations, and JW Player video playlists account for the massive resource overhead, leading to an overhead-to-content ratio of at least 37:1. Tools such as [Prebid.js](https://prebid.org/) and [JW Player](https://www.jwplayer.com/) are central to the programmatic ad and video experience, while Future PLC’s proprietary scripts (e.g., bordeaux.js) contribute substantial downstream activity.

**Key Findings:**
- 82.6% of data transferred is non-content (ads, tracking, identity, analytics)
- HTML includes 673 KB of inline CSS and 439 KB of inline JavaScript—far above best practices
- JW Player video carousel (with multiple autoplaying tracks) accounts for the largest single traffic source (up to 136 MB for 1080p renditions)
- Poor caching: Over 50% of requests are non-cacheable or lack cache headers
- 200+ MB can be transferred in 5 minutes on a modern browser, mostly due to video, ad refresh, and ongoing tracker activity
