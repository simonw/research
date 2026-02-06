/**
 * Live Site Traffic Capture Test
 *
 * Visits Hacker News headlessly, fetches its real JSON API,
 * scrolls, navigates — then runs HAR analysis to extract API endpoints.
 *
 * Uses a local HTTP proxy bridge to work around Chromium's incompatibility
 * with JWT-authenticated egress proxies (ERR_TUNNEL_CONNECTION_FAILED).
 *
 * NOTE: This is a generic *live site* capture smoke-test used to validate
 * HAR capture + analysis against a real site (currently Hacker News).
 */

import { chromium } from 'playwright';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { createServer as createHttpServer, request as httpRequest, type IncomingMessage, type ServerResponse } from 'node:http';
import { connect as netConnect } from 'node:net';
import { analyzeHar, prepareHarSummaryForLLM } from './har-analyzer.js';
import type { RecordingSession, UserAction } from './types.js';

const SEED_URL = 'https://news.ycombinator.com';
const OUTPUT_DIR = join(process.cwd(), 'runs');
const SESSION_ID = `hn-capture-${Date.now()}`;
const SESSION_DIR = join(OUTPUT_DIR, SESSION_ID);
const HAR_PATH = join(SESSION_DIR, 'recording.har');
const SCREENSHOTS_DIR = join(SESSION_DIR, 'screenshots');
const LOCAL_PROXY_PORT = 18923;

/** Parse upstream proxy from environment */
function getUpstreamProxy(): { host: string; port: number; auth: string } | undefined {
  const proxyUrl = process.env.HTTPS_PROXY || process.env.https_proxy;
  if (!proxyUrl) return undefined;
  try {
    const parsed = new URL(proxyUrl);
    const username = decodeURIComponent(parsed.username);
    const password = decodeURIComponent(parsed.password);
    const auth = Buffer.from(`${username}:${password}`).toString('base64');
    return {
      host: parsed.hostname,
      port: parseInt(parsed.port, 10),
      auth,
    };
  } catch {
    return undefined;
  }
}

/**
 * Start a local proxy that forwards to the upstream egress proxy with auth.
 * Chromium connects to this local proxy (no auth needed), and it handles
 * the CONNECT tunnel + Proxy-Authorization header to the upstream.
 */
function startLocalProxy(upstream: { host: string; port: number; auth: string }): Promise<ReturnType<typeof createHttpServer>> {
  return new Promise((resolve) => {
    const server = createHttpServer((req: IncomingMessage, res: ServerResponse) => {
      // Forward HTTP requests to upstream proxy
      const proxyReq = httpRequest({
        host: upstream.host,
        port: upstream.port,
        method: req.method,
        path: req.url,
        headers: {
          ...req.headers,
          'proxy-authorization': `Basic ${upstream.auth}`,
        },
      }, (proxyRes) => {
        res.writeHead(proxyRes.statusCode || 502, proxyRes.headers);
        proxyRes.pipe(res);
      });
      proxyReq.on('error', () => res.destroy());
      req.pipe(proxyReq);
    });

    // Handle CONNECT tunnels (HTTPS)
    server.on('connect', (req: IncomingMessage, clientSocket: any, head: Buffer) => {
      const upstreamSocket = netConnect(upstream.port, upstream.host, () => {
        // Send CONNECT request to upstream with auth
        const connectReq = [
          `CONNECT ${req.url} HTTP/1.1`,
          `Host: ${req.url}`,
          `Proxy-Authorization: Basic ${upstream.auth}`,
          `User-Agent: Mozilla/5.0`,
          '',
          '',
        ].join('\r\n');

        upstreamSocket.write(connectReq);

        // Wait for upstream proxy response
        upstreamSocket.once('data', (data: Buffer) => {
          const response = data.toString();
          if (response.includes('200')) {
            // Tunnel established — tell client it's ready
            clientSocket.write('HTTP/1.1 200 Connection Established\r\n\r\n');
            // Now pipe bidirectionally
            if (head.length > 0) upstreamSocket.write(head);
            upstreamSocket.pipe(clientSocket);
            clientSocket.pipe(upstreamSocket);
          } else {
            clientSocket.write('HTTP/1.1 502 Bad Gateway\r\n\r\n');
            clientSocket.destroy();
            upstreamSocket.destroy();
          }
        });
      });

      upstreamSocket.on('error', () => {
        clientSocket.write('HTTP/1.1 502 Bad Gateway\r\n\r\n');
        clientSocket.destroy();
      });
    });

    server.listen(LOCAL_PROXY_PORT, '127.0.0.1', () => {
      resolve(server);
    });
  });
}

async function main() {
  console.log('╔══════════════════════════════════════════════════╗');
  console.log('║   Live Site Traffic Capture Test                 ║');
  console.log('║   Target: Hacker News (news.ycombinator.com)    ║');
  console.log('╚══════════════════════════════════════════════════╝\n');

  mkdirSync(SCREENSHOTS_DIR, { recursive: true });

  console.log(`📂 Session dir: ${SESSION_DIR}`);
  console.log(`🌐 Target: ${SEED_URL}\n`);

  // Set up local proxy bridge if upstream proxy is configured
  const upstream = getUpstreamProxy();
  let localProxy: ReturnType<typeof createHttpServer> | undefined;
  let proxyConfig: { server: string } | undefined;

  if (upstream) {
    console.log(`🔌 Upstream proxy: ${upstream.host}:${upstream.port}`);
    console.log('🔌 Starting local proxy bridge...');
    localProxy = await startLocalProxy(upstream);
    proxyConfig = { server: `http://127.0.0.1:${LOCAL_PROXY_PORT}` };
    console.log(`✅ Local proxy bridge running on port ${LOCAL_PROXY_PORT}\n`);
  }

  // Launch browser
  console.log('🚀 Launching browser...');
  const browser = await chromium.launch({
    headless: true,
    proxy: proxyConfig,
    args: [
      '--disable-blink-features=AutomationControlled',
      '--no-sandbox',
      '--disable-dev-shm-usage',
      '--disable-gpu',
      '--single-process',
    ],
  });

  const context = await browser.newContext({
    recordHar: { path: HAR_PATH, mode: 'full' },
    viewport: { width: 1280, height: 800 },
    userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    ignoreHTTPSErrors: true,
  });

  const page = await context.newPage();
  const actions: UserAction[] = [];

  // Track navigations
  page.on('framenavigated', (frame) => {
    if (frame === page.mainFrame()) {
      const url = frame.url();
      if (url && url !== 'about:blank') {
        actions.push({
          type: 'navigate',
          timestamp: new Date().toISOString(),
          url,
          description: `Navigated to ${url}`,
        });
        console.log(`  🔗 Navigate: ${url}`);
      }
    }
  });

  // Navigate to HN
  console.log('\n📡 Loading news.ycombinator.com...');
  try {
    await page.goto(SEED_URL, { waitUntil: 'networkidle', timeout: 30000 });
    console.log('  ✅ Page loaded');
  } catch (e: any) {
    console.log(`  ⚠️  Navigation issue: ${e.message?.slice(0, 120)}`);
  }

  await page.waitForTimeout(2000);

  // Screenshot homepage
  try {
    await page.screenshot({ path: join(SCREENSHOTS_DIR, '01-homepage.png') });
    console.log('📸 Screenshot: homepage');
  } catch { console.log('  ⚠️  Screenshot failed'); }

  // Fetch HN JSON API by navigating to the API URLs directly.
  // (page.evaluate(fetch()) doesn't work for cross-origin through the proxy bridge)
  // Using a separate page so HN homepage stays loaded in the main page.
  console.log('\n📡 Fetching HN API endpoints (direct navigation)...');
  const apiPage = await context.newPage();

  try {
    // Helper: navigate to API URL and parse response
    async function fetchApi(url: string): Promise<any> {
      await apiPage.goto(url, { waitUntil: 'networkidle', timeout: 15000 });
      const text = await apiPage.textContent('body');
      return JSON.parse(text || '{}');
    }

    // Top stories
    const topStoryIds = await fetchApi('https://hacker-news.firebaseio.com/v0/topstories.json');
    console.log(`  ✅ /v0/topstories.json → ${topStoryIds.length} story IDs`);
    actions.push({
      type: 'api-fetch', timestamp: new Date().toISOString(),
      description: `Fetched topstories (${topStoryIds.length} IDs)`,
      url: 'https://hacker-news.firebaseio.com/v0/topstories.json',
    });

    // Fetch first 5 story details
    const storyIds = topStoryIds.slice(0, 5);
    for (const id of storyIds) {
      const story = await fetchApi(`https://hacker-news.firebaseio.com/v0/item/${id}.json`);
      console.log(`  ✅ /v0/item/${id}.json → "${(story?.title ?? '').slice(0, 55)}" (${story?.score} pts, ${story?.descendants ?? 0} comments)`);
      actions.push({
        type: 'api-fetch', timestamp: new Date().toISOString(),
        description: `Fetched story ${id}: ${(story?.title ?? '').slice(0, 60)}`,
        url: `https://hacker-news.firebaseio.com/v0/item/${id}.json`,
      });
    }

    // Best stories
    const bestIds = await fetchApi('https://hacker-news.firebaseio.com/v0/beststories.json');
    console.log(`  ✅ /v0/beststories.json → ${bestIds.length} IDs`);
    actions.push({
      type: 'api-fetch', timestamp: new Date().toISOString(),
      description: `Fetched beststories (${bestIds.length} IDs)`,
      url: 'https://hacker-news.firebaseio.com/v0/beststories.json',
    });

    // New stories
    const newIds = await fetchApi('https://hacker-news.firebaseio.com/v0/newstories.json');
    console.log(`  ✅ /v0/newstories.json → ${newIds.length} IDs`);
    actions.push({
      type: 'api-fetch', timestamp: new Date().toISOString(),
      description: `Fetched newstories (${newIds.length} IDs)`,
      url: 'https://hacker-news.firebaseio.com/v0/newstories.json',
    });

    // User profile from first story's author
    const firstStory = storyIds[0] ? await fetchApi(`https://hacker-news.firebaseio.com/v0/item/${storyIds[0]}.json`) : null;
    const author = firstStory?.by;
    if (author) {
      const user = await fetchApi(`https://hacker-news.firebaseio.com/v0/user/${author}.json`);
      console.log(`  ✅ /v0/user/${author}.json → karma: ${user?.karma}`);
      actions.push({
        type: 'api-fetch', timestamp: new Date().toISOString(),
        description: `Fetched user ${author}`,
        url: `https://hacker-news.firebaseio.com/v0/user/${author}.json`,
      });
    }
  } catch (e: any) {
    console.log(`  ⚠️  API fetch error: ${e.message?.slice(0, 150)}`);
  }

  // Close API page, switch back to main HN page
  await apiPage.close();

  // Scroll
  console.log('\n📜 Scrolling...');
  for (let i = 0; i < 3; i++) {
    try {
      await page.evaluate(() => window.scrollBy(0, 600));
      actions.push({ type: 'scroll', timestamp: new Date().toISOString(), description: `Scroll ${i + 1}` });
      await page.waitForTimeout(1000);
    } catch { break; }
  }

  // Click "More"
  console.log('\n🖱️  Clicking "More" for page 2...');
  try {
    const moreLink = await page.$('a.morelink');
    if (moreLink) {
      await moreLink.click();
      actions.push({ type: 'click', timestamp: new Date().toISOString(), description: 'Clicked "More" for page 2' });
      await page.waitForTimeout(3000);
      console.log('  ✅ Page 2 loaded');
      try { await page.screenshot({ path: join(SCREENSHOTS_DIR, '02-page2.png') }); console.log('📸 Screenshot: page 2'); } catch {}
    } else {
      console.log('  No "More" link found');
    }
  } catch (e: any) {
    console.log(`  ⚠️  ${e.message?.slice(0, 100)}`);
  }

  // Click a comments link
  console.log('\n🖱️  Clicking story comments...');
  try {
    const commentLink = await page.$('td.subtext a:last-child');
    if (commentLink) {
      const text = await commentLink.textContent();
      console.log(`  Found: "${text?.trim()}"`);
      await commentLink.click();
      actions.push({ type: 'click', timestamp: new Date().toISOString(), description: `Clicked comments: ${text?.trim()}` });
      await page.waitForTimeout(3000);
      try { await page.screenshot({ path: join(SCREENSHOTS_DIR, '03-comments.png') }); console.log('📸 Screenshot: comments'); } catch {}
    } else {
      console.log('  No comment link found');
    }
  } catch (e: any) {
    console.log(`  ⚠️  ${e.message?.slice(0, 100)}`);
  }

  // Cookies
  console.log('\n🍪 Collecting cookies...');
  const browserCookies = await context.cookies();
  const cookieMap: Record<string, string> = {};
  for (const c of browserCookies) cookieMap[c.name] = c.value;
  console.log(`  Found ${Object.keys(cookieMap).length} cookies`);

  // Flush HAR
  console.log('\n⏹️  Closing browser...');
  await context.close();
  await browser.close();

  // Stop local proxy
  if (localProxy) {
    localProxy.close();
    console.log('  Local proxy stopped');
  }

  // Analyze HAR
  console.log('\n📊 Analyzing HAR file...');
  let harData = { log: { entries: [] as any[] } };
  if (existsSync(HAR_PATH)) {
    try {
      const raw = readFileSync(HAR_PATH, 'utf-8');
      harData = JSON.parse(raw);
      console.log(`  HAR file size: ${Math.round(raw.length / 1024)} KB`);
      console.log(`  Total HAR entries: ${harData.log.entries.length}`);
    } catch (e: any) {
      console.log(`  ⚠️  Parse failed: ${e.message}`);
    }
  } else {
    console.log('  ⚠️  No HAR file!');
  }

  const analysis = analyzeHar(harData, SEED_URL);

  // Build session
  const session: RecordingSession = {
    id: SESSION_ID,
    url: SEED_URL,
    description: 'Get a list of top posts from Hacker News with titles, scores, and comment counts',
    startTime: actions[0]?.timestamp ?? new Date().toISOString(),
    endTime: new Date().toISOString(),
    actions,
    apiRequests: analysis.apiRequests,
    cookies: { ...cookieMap, ...analysis.cookies },
    authHeaders: analysis.authHeaders,
    authMethod: analysis.authMethod,
    targetDomain: analysis.targetDomain,
    harFilePath: HAR_PATH,
    screenshotsDir: SCREENSHOTS_DIR,
  };

  // Save files
  writeFileSync(join(SESSION_DIR, 'session.json'), JSON.stringify(session, null, 2));
  writeFileSync(join(SESSION_DIR, 'actions.json'), JSON.stringify(actions, null, 2));
  writeFileSync(join(SESSION_DIR, 'auth.json'), JSON.stringify({
    authHeaders: analysis.authHeaders,
    cookies: { ...cookieMap, ...analysis.cookies },
    authMethod: analysis.authMethod,
    targetDomain: analysis.targetDomain,
  }, null, 2));

  const llmSummary = prepareHarSummaryForLLM(analysis.apiRequests);
  writeFileSync(join(SESSION_DIR, 'llm-summary.txt'), llmSummary);

  // Print results
  console.log('\n═══════════════════════════════════════════════════');
  console.log('📊 CAPTURE RESULTS');
  console.log('═══════════════════════════════════════════════════');
  console.log(`  Actions recorded:      ${actions.length}`);
  console.log(`  API requests captured:  ${analysis.apiRequests.length}`);
  console.log(`  Auth method:           ${analysis.authMethod}`);
  console.log(`  Auth headers:          ${Object.keys(analysis.authHeaders).length}`);
  console.log(`  Cookies:               ${Object.keys(cookieMap).length}`);
  console.log(`  Domains seen:          ${analysis.allDomains.length}`);
  console.log('');

  const byDomain: Record<string, typeof analysis.apiRequests> = {};
  for (const req of analysis.apiRequests) {
    if (!byDomain[req.domain]) byDomain[req.domain] = [];
    byDomain[req.domain].push(req);
  }

  console.log('📡 API Endpoints by Domain:');
  for (const [domain, reqs] of Object.entries(byDomain).sort((a, b) => b[1].length - a[1].length)) {
    console.log(`\n  ${domain} (${reqs.length} requests):`);
    const seen = new Set<string>();
    for (const req of reqs) {
      const normalized = req.path.replace(/\/\d+/g, '/{id}').split('?')[0];
      const key = `${req.method} ${normalized}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const sizeStr = req.responseSize ? ` (${req.responseSize}B)` : '';
      const bodyPreview = req.responseBody
        ? ` → ${req.responseBody.slice(0, 80).replace(/\n/g, ' ')}...`
        : '';
      console.log(`    ${req.method} ${normalized} → ${req.status}${sizeStr}${bodyPreview}`);
    }
  }

  console.log('\n═══════════════════════════════════════════════════');
  console.log('📁 OUTPUT FILES:');
  console.log('═══════════════════════════════════════════════════');
  console.log(`  Session dir:   ${SESSION_DIR}/`);
  console.log(`  ├── recording.har`);
  console.log(`  ├── session.json`);
  console.log(`  ├── actions.json`);
  console.log(`  ├── auth.json`);
  console.log(`  ├── llm-summary.txt`);
  console.log(`  └── screenshots/`);
  console.log('');
  console.log('⚠️  No Gemini API key — script generation skipped.');
  console.log('   The captured data above is ready for an LLM to generate');
  console.log('   a Playwright automation script.\n');
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
