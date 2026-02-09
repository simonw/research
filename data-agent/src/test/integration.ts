/**
 * Integration test harness — no test framework needed.
 *
 * Runs scraping flows against 4 public demo sites using Playwright directly
 * (not the full LLM pipeline) and asserts that outputs are valid.
 *
 * Usage:
 *   npx tsx src/test/integration.ts
 *   npm run test:integration
 *
 * Sites tested:
 *   1. quotes.toscrape.com  — paginated quotes list
 *   2. books.toscrape.com   — book catalog with prices
 *   3. httpbin.org           — JSON API endpoints
 *   4. wikipedia.org         — article content extraction
 *
 * Each test:
 *   - Launches a headless browser
 *   - Navigates and extracts data
 *   - Validates output (exists, valid JSON, expected item count)
 *   - Writes artifacts to test-artifacts/
 */

import { chromium } from 'playwright';
import { existsSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { waitForNetworkSettle } from '../browser/network-settle.js';
import { getPageStats } from '../browser/page-stats.js';
import { initArtifactBundle, writeArtifact } from '../browser/artifacts.js';
import { computeCacheKey, recordActions, tryCacheHit } from '../browser/action-cache.js';
import { classifyError } from '../validate/validator.js';

const ARTIFACTS_DIR = resolve(import.meta.dirname, '../../test-artifacts');

interface TestResult {
  name: string;
  site: string;
  passed: boolean;
  itemCount: number;
  durationMs: number;
  error?: string;
}

// ── Helpers ──────────────────────────────────────────────────────────

function assert(condition: boolean, message: string): void {
  if (!condition) throw new Error(`Assertion failed: ${message}`);
}

function assertGte(actual: number, expected: number, label: string): void {
  if (actual < expected) {
    throw new Error(`${label}: expected >= ${expected}, got ${actual}`);
  }
}

// ── Test cases ───────────────────────────────────────────────────────

/**
 * Test 1: quotes.toscrape.com — scrape quotes from the first page.
 */
async function testQuotesToScrape(artifactDir: string): Promise<TestResult> {
  const start = Date.now();
  const browser = await chromium.launch({ headless: true });

  try {
    const page = await browser.newPage();
    await page.goto('https://quotes.toscrape.com/', { waitUntil: 'domcontentloaded', timeout: 30_000 });
    await waitForNetworkSettle(page, { quietMs: 300, timeoutMs: 5000 });

    // Collect page stats
    const stats = await getPageStats(page);
    writeArtifact(artifactDir, 'page-stats-quotes.json', stats);
    assert(!stats.isLikelyBlocked, `quotes.toscrape.com should not be blocked: ${stats.blockReason}`);

    // Extract quotes
    const quotes = await page.evaluate(() => {
      const items: Array<{ text: string; author: string; tags: string[] }> = [];
      document.querySelectorAll('.quote').forEach((el) => {
        const text = el.querySelector('.text')?.textContent?.trim() ?? '';
        const author = el.querySelector('.author')?.textContent?.trim() ?? '';
        const tags: string[] = [];
        el.querySelectorAll('.tag').forEach((t) => {
          const tag = t.textContent?.trim();
          if (tag) tags.push(tag);
        });
        items.push({ text, author, tags });
      });
      return items;
    });

    const outputPath = join(artifactDir, 'output-quotes.json');
    writeFileSync(outputPath, JSON.stringify(quotes, null, 2));

    assert(quotes.length > 0, 'Should extract at least 1 quote');
    assertGte(quotes.length, 5, 'Quote count');
    assert(quotes[0].text.length > 10, 'Quote text should be non-trivial');
    assert(quotes[0].author.length > 0, 'Quote author should be non-empty');

    return { name: 'quotes.toscrape.com', site: 'https://quotes.toscrape.com/', passed: true, itemCount: quotes.length, durationMs: Date.now() - start };
  } catch (err) {
    return { name: 'quotes.toscrape.com', site: 'https://quotes.toscrape.com/', passed: false, itemCount: 0, durationMs: Date.now() - start, error: (err as Error).message };
  } finally {
    await browser.close();
  }
}

/**
 * Test 2: books.toscrape.com — scrape book titles and prices.
 */
async function testBooksToscrape(artifactDir: string): Promise<TestResult> {
  const start = Date.now();
  const browser = await chromium.launch({ headless: true });

  try {
    const page = await browser.newPage();
    await page.goto('https://books.toscrape.com/', { waitUntil: 'domcontentloaded', timeout: 30_000 });
    await waitForNetworkSettle(page, { quietMs: 300, timeoutMs: 5000 });

    const stats = await getPageStats(page);
    writeArtifact(artifactDir, 'page-stats-books.json', stats);
    assert(!stats.isLikelyBlocked, `books.toscrape.com should not be blocked: ${stats.blockReason}`);

    // Extract books
    const books = await page.evaluate(() => {
      const items: Array<{ title: string; price: string; rating: string; inStock: boolean }> = [];
      document.querySelectorAll('article.product_pod').forEach((el) => {
        const title = el.querySelector('h3 a')?.getAttribute('title') ?? '';
        const price = el.querySelector('.price_color')?.textContent?.trim() ?? '';
        const ratingEl = el.querySelector('.star-rating');
        const rating = ratingEl ? Array.from(ratingEl.classList).find(c => c !== 'star-rating') || '' : '';
        const inStock = (el.querySelector('.instock')?.textContent?.trim() || '').includes('In stock');
        items.push({ title, price, rating, inStock });
      });
      return items;
    });

    const outputPath = join(artifactDir, 'output-books.json');
    writeFileSync(outputPath, JSON.stringify(books, null, 2));

    assertGte(books.length, 10, 'Book count');
    assert(books[0].title.length > 0, 'Book title should be non-empty');
    assert(books[0].price.startsWith('£'), 'Book price should start with £');

    return { name: 'books.toscrape.com', site: 'https://books.toscrape.com/', passed: true, itemCount: books.length, durationMs: Date.now() - start };
  } catch (err) {
    return { name: 'books.toscrape.com', site: 'https://books.toscrape.com/', passed: false, itemCount: 0, durationMs: Date.now() - start, error: (err as Error).message };
  } finally {
    await browser.close();
  }
}

/**
 * Test 3: httpbin.org — JSON API endpoint.
 */
async function testHttpbin(artifactDir: string): Promise<TestResult> {
  const start = Date.now();
  const browser = await chromium.launch({ headless: true });

  try {
    const page = await browser.newPage();

    // Use Playwright's route interception to capture the JSON response
    let apiJson: unknown = null;
    page.on('response', async (response) => {
      if (response.url().includes('/get') && response.status() === 200) {
        try {
          apiJson = await response.json();
        } catch { /* non-json */ }
      }
    });

    await page.goto('https://httpbin.org/get', { waitUntil: 'domcontentloaded', timeout: 30_000 });
    await waitForNetworkSettle(page, { quietMs: 300, timeoutMs: 5000 });

    const stats = await getPageStats(page);
    writeArtifact(artifactDir, 'page-stats-httpbin.json', stats);

    // Also try extracting the JSON from the page body directly
    const bodyText = await page.evaluate(() => document.body?.innerText || '');
    let parsedFromPage: unknown = null;
    try {
      parsedFromPage = JSON.parse(bodyText);
    } catch { /* page might render HTML wrapper */ }

    const output = apiJson || parsedFromPage;
    assert(output !== null, 'Should capture JSON from httpbin.org/get');

    const outputPath = join(artifactDir, 'output-httpbin.json');
    writeFileSync(outputPath, JSON.stringify(output, null, 2));

    const keys = Object.keys(output as Record<string, unknown>);
    assertGte(keys.length, 3, 'httpbin response key count');
    assert(keys.includes('headers') || keys.includes('url'), 'httpbin response should have headers or url');

    return { name: 'httpbin.org', site: 'https://httpbin.org/get', passed: true, itemCount: keys.length, durationMs: Date.now() - start };
  } catch (err) {
    return { name: 'httpbin.org', site: 'https://httpbin.org/get', passed: false, itemCount: 0, durationMs: Date.now() - start, error: (err as Error).message };
  } finally {
    await browser.close();
  }
}

/**
 * Test 4: wikipedia.org — extract article summary.
 */
async function testWikipedia(artifactDir: string): Promise<TestResult> {
  const start = Date.now();
  const browser = await chromium.launch({ headless: true });

  try {
    const page = await browser.newPage();
    await page.goto('https://en.wikipedia.org/wiki/Web_scraping', { waitUntil: 'domcontentloaded', timeout: 30_000 });
    await waitForNetworkSettle(page, { quietMs: 300, timeoutMs: 5000 });

    const stats = await getPageStats(page);
    writeArtifact(artifactDir, 'page-stats-wikipedia.json', stats);
    assert(!stats.isLikelyBlocked, `wikipedia.org should not be blocked: ${stats.blockReason}`);

    // Extract article content
    const article = await page.evaluate(() => {
      const title = document.querySelector('#firstHeading')?.textContent?.trim() ?? '';
      const paragraphs: string[] = [];
      document.querySelectorAll('#mw-content-text .mw-parser-output > p').forEach((el) => {
        const text = el.textContent?.trim();
        if (text && text.length > 50) paragraphs.push(text);
      });

      const toc: string[] = [];
      document.querySelectorAll('.toc a .toctext, .vector-toc-text').forEach((el) => {
        const text = el.textContent?.trim();
        if (text) toc.push(text);
      });

      const links: Array<{ text: string; href: string }> = [];
      document.querySelectorAll('#mw-content-text a[href^="/wiki/"]').forEach((el) => {
        const text = el.textContent?.trim() ?? '';
        const href = el.getAttribute('href') ?? '';
        if (text && href && !href.includes(':')) {
          links.push({ text, href: `https://en.wikipedia.org${href}` });
        }
      });

      return { title, paragraphs: paragraphs.slice(0, 5), toc: toc.slice(0, 10), links: links.slice(0, 20) };
    });

    const outputPath = join(artifactDir, 'output-wikipedia.json');
    writeFileSync(outputPath, JSON.stringify(article, null, 2));

    assert(article.title.toLowerCase().includes('scraping'), 'Wikipedia article title should contain "scraping"');
    assertGte(article.paragraphs.length, 1, 'Wikipedia paragraph count');
    assert(article.paragraphs[0].length > 100, 'First paragraph should be substantial');

    return { name: 'wikipedia.org', site: 'https://en.wikipedia.org/wiki/Web_scraping', passed: true, itemCount: article.paragraphs.length + article.links.length, durationMs: Date.now() - start };
  } catch (err) {
    return { name: 'wikipedia.org', site: 'https://en.wikipedia.org/wiki/Web_scraping', passed: false, itemCount: 0, durationMs: Date.now() - start, error: (err as Error).message };
  } finally {
    await browser.close();
  }
}

// ── Unit tests for new modules ───────────────────────────────────────

/**
 * Test the error classifier with synthetic inputs.
 */
function testErrorClassifier(): TestResult {
  const start = Date.now();
  try {
    // Missing browser
    const r1 = classifyError("Executable doesn't exist at /foo/chrome", '');
    assert(r1[0].errorClass === 'missing_browser', 'Should classify missing browser');
    assert(r1[0].autoFixable === true, 'Missing browser should be auto-fixable');

    // Blocked page
    const r2 = classifyError('HTTP 403 Forbidden', '');
    assert(r2[0].errorClass === 'blocked_page', 'Should classify 403 as blocked');

    // Selector timeout
    const r3 = classifyError('waiting for selector ".foo" timeout', '');
    assert(r3[0].errorClass === 'selector_timeout', 'Should classify selector timeout');

    // JSON parse error
    const r4 = classifyError('SyntaxError: Unexpected token < in JSON', '');
    assert(r4[0].errorClass === 'json_parse', 'Should classify JSON parse error');

    // Rate limited
    const r5 = classifyError('HTTP 429 Too Many Requests', '');
    assert(r5[0].errorClass === 'rate_limited', 'Should classify rate limit');

    // Unknown
    const r6 = classifyError('something unexpected', '');
    assert(r6[0].errorClass === 'unknown', 'Should fall back to unknown');

    return { name: 'error-classifier', site: 'unit-test', passed: true, itemCount: 6, durationMs: Date.now() - start };
  } catch (err) {
    return { name: 'error-classifier', site: 'unit-test', passed: false, itemCount: 0, durationMs: Date.now() - start, error: (err as Error).message };
  }
}

/**
 * Test the action cache skeleton.
 */
function testActionCache(): TestResult {
  const start = Date.now();
  try {
    const key = computeCacheKey('https://example.com/page?_t=123', 'get items', ['id']);
    assert(key.length === 16, 'Cache key should be 16 hex chars');

    // Same URL with volatile params stripped should produce same key
    const key2 = computeCacheKey('https://example.com/page?_t=456', 'get items', ['id']);
    assert(key === key2, 'Cache keys should be stable after stripping volatile params');

    // Different task should produce different key
    const key3 = computeCacheKey('https://example.com/page', 'get other items', ['id']);
    assert(key3 !== key, 'Different tasks should produce different keys');

    // Record and retrieve
    recordActions(key, 'https://example.com/page', 'get items', [
      { action: 'click', selector: '.btn' },
      { action: 'type', selector: '#search', text: 'test' },
    ]);

    const hit = tryCacheHit(key);
    assert(hit !== null, 'Should find cached actions');
    assert(hit!.actions.length === 2, 'Should have 2 cached actions');
    assert(hit!.hitCount === 1, 'Hit count should be 1');

    return { name: 'action-cache', site: 'unit-test', passed: true, itemCount: 3, durationMs: Date.now() - start };
  } catch (err) {
    return { name: 'action-cache', site: 'unit-test', passed: false, itemCount: 0, durationMs: Date.now() - start, error: (err as Error).message };
  }
}

/**
 * Test the artifact bundle helpers.
 */
function testArtifactBundle(): TestResult {
  const start = Date.now();
  try {
    const testDir = join(ARTIFACTS_DIR, 'test-bundle-' + Date.now());
    const bundle = initArtifactBundle(testDir);
    assert(existsSync(testDir), 'Artifact dir should be created');
    assert(bundle.sessionDir === testDir, 'Bundle sessionDir should match');

    writeArtifact(testDir, 'test.json', { hello: 'world' });
    assert(existsSync(join(testDir, 'test.json')), 'Written artifact should exist');

    // Clean up
    rmSync(testDir, { recursive: true, force: true });

    return { name: 'artifact-bundle', site: 'unit-test', passed: true, itemCount: 2, durationMs: Date.now() - start };
  } catch (err) {
    return { name: 'artifact-bundle', site: 'unit-test', passed: false, itemCount: 0, durationMs: Date.now() - start, error: (err as Error).message };
  }
}

// ── Main runner ──────────────────────────────────────────────────────

async function main(): Promise<void> {
  console.log('=== data-agent Integration Test Harness ===\n');

  // Clean and create artifacts directory
  if (existsSync(ARTIFACTS_DIR)) {
    rmSync(ARTIFACTS_DIR, { recursive: true, force: true });
  }
  mkdirSync(ARTIFACTS_DIR, { recursive: true });

  const results: TestResult[] = [];

  // Run unit tests first (no browser needed)
  console.log('--- Unit Tests ---\n');

  for (const unitTest of [testErrorClassifier, testActionCache, testArtifactBundle]) {
    const result = unitTest();
    results.push(result);
    const icon = result.passed ? 'PASS' : 'FAIL';
    const time = `${result.durationMs}ms`;
    console.log(`  [${icon}] ${result.name} (${time})${result.error ? ` — ${result.error}` : ''}`);
  }

  // Run browser integration tests
  console.log('\n--- Browser Integration Tests ---\n');

  const browserTests = [
    { name: 'quotes.toscrape.com', fn: testQuotesToScrape },
    { name: 'books.toscrape.com', fn: testBooksToscrape },
    { name: 'httpbin.org', fn: testHttpbin },
    { name: 'wikipedia.org', fn: testWikipedia },
  ];

  for (const test of browserTests) {
    console.log(`  Running: ${test.name}...`);
    const result = await test.fn(ARTIFACTS_DIR);
    results.push(result);
    const icon = result.passed ? 'PASS' : 'FAIL';
    const time = `${(result.durationMs / 1000).toFixed(1)}s`;
    console.log(`  [${icon}] ${test.name} — ${result.itemCount} items (${time})${result.error ? ` — ${result.error}` : ''}`);
  }

  // Summary
  console.log('\n--- Summary ---\n');

  const passed = results.filter(r => r.passed).length;
  const failed = results.filter(r => !r.passed).length;
  const totalTime = results.reduce((sum, r) => sum + r.durationMs, 0);

  console.log(`  Total: ${results.length} tests, ${passed} passed, ${failed} failed`);
  console.log(`  Time: ${(totalTime / 1000).toFixed(1)}s`);
  console.log(`  Artifacts: ${ARTIFACTS_DIR}`);

  // Write summary artifact
  writeFileSync(
    join(ARTIFACTS_DIR, 'test-summary.json'),
    JSON.stringify({ results, totalTime, passed, failed, timestamp: new Date().toISOString() }, null, 2),
  );

  if (failed > 0) {
    console.log('\nFailed tests:');
    for (const r of results.filter(r => !r.passed)) {
      console.log(`  - ${r.name}: ${r.error}`);
    }
    process.exit(1);
  }

  console.log('\nAll tests passed!');
}

main().catch((err) => {
  console.error('Test harness fatal error:', err);
  process.exit(1);
});
