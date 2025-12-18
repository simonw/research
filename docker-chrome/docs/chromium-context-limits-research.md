# Chromium Browser Context Limits Research

## Purpose
Determine the practical concurrency limits and resource overhead of running multiple browser contexts within a single Chromium instance to inform scaling strategies.

## Key Findings
- **Concurrency Limits:** There is no architectural "hard limit" in Chromium. Practical limits are dictated by system resources (RAM/CPU).
- **Rule of Thumb:**
  - **Conservative:** 10-20 contexts per instance.
  - **Aggressive:** Up to 50-100 contexts (Playwright's target).
- **Resource Overhead:**
  - **Empty Context:** ~10-50MB RAM.
  - **Active Context (with page):** 100MB-300MB+ depending on content.
- **Bottlenecks:** RAM is the primary constraint. CPU becomes a bottleneck if pages are JS-heavy.

## Benchmarking
A custom measurement script (`measurement-script.js`) confirmed that context creation is lightweight (~ms), but memory usage grows linearly.

## Conclusion
Scaling should be based on available RAM (allocating ~200MB per expected active session) rather than an arbitrary count limit.
