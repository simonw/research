Integrating Chromium into a serverless-ready Docker environment enables high-performance remote automation and real-time network inspection. The architecture utilizes [Selkies GStreamer](https://github.com/selkies-project/selkies-gstreamer) for low-latency WebRTC streaming and a Node.js bridge to facilitate Chrome DevTools Protocol (CDP) interactions via [Playwright](https://playwright.dev/). Custom automation wrappers provide visual feedback and cursor animations for script execution, while stealth features like plugin spoofing and webdriver overrides help bypass bot detection. Optimized for deployment on Google Cloud Run, the system maintains session affinity and supports residential proxies for scalable, observable data extraction.

**Key Architecture Features:**
* Real-time network monitoring and response body capture via CDP domains.
* Stealth-focused browser lockdown including user-agent randomization and context menu blocking.
* Fixed iPhone SE viewport resolution to ensure consistent automation and streaming performance.
* Multi-service orchestration using Supervisor to manage Xvfb, the bridge server, and WebRTC components.
