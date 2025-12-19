# Chromium Browser Context Limits Research Summary

## Overview
Analysis of the performance and stability limits of running multiple isolated `BrowserContexts` within a single Chromium instance.

## Key Findings
- **Isolation**: Each context provides full cookie/storage isolation with minimal overhead compared to full browser instances.
- **Resource Usage**: 
  - Memory: ~10-50MB per empty context; 100-300MB per active page.
  - CPU: Contention increases linearly; Playwright claims support for up to 100 concurrent contexts.
- **Practical Limits**: Stable operations are typically observed with 10-20 contexts per browser instance on standard hardware. Beyond 50 contexts, system RAM and CPU core count become significant bottlenecks.

## Recommendations
- Use "Incognito" contexts for stateless sessions to avoid disk I/O overhead.
- Monitor PSS (Proportional Set Size) rather than RSS for accurate memory measurement of shared Chromium processes.
- Implement a pool of browser instances if more than 50-100 concurrent sessions are required.
