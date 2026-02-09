# Testing

## Quick start

```bash
cd data-agent
npm install
npm run build
npm run test:integration
```

This repo uses a lightweight integration harness (no test framework) that runs real Playwright scrapes against a few public, automation-friendly demo sites.

## Integration harness

Command:

```bash
npm run test:integration
# (runs: tsx src/test/integration.ts)
```

What it does:
- Runs a few **fast unit checks** for the new reliability helpers:
  - error classification (`classifyError()`)
  - action-cache persistence
  - artifact bundle helpers
- Runs **browser-backed integration tests** (headless Chromium) against:
  - https://quotes.toscrape.com (quotes list)
  - https://books.toscrape.com (book list)
  - https://httpbin.org/get (JSON endpoint)
  - https://en.wikipedia.org/wiki/Web_scraping (article extraction)

Artifacts:
- Writes JSON outputs + page-stats snapshots to:
  - `data-agent/test-artifacts/`

Note: `test-artifacts/` is gitignored.

## Expected output

A passing run ends with a summary like:
- 7 tests passed
- artifacts path printed

If Playwright browser binaries are missing, install them with:

```bash
npx playwright install chromium
```
