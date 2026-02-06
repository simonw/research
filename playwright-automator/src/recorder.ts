/**
 * Browser Recorder — Launch Playwright browser and record all activity.
 *
 * Records:
 * - All network traffic (HAR file)
 * - User actions (clicks, typing, navigation)
 * - Page screenshots at key moments
 * - Cookies and auth headers
 */

import { chromium, type Browser, type BrowserContext, type Page } from 'playwright';
import { mkdirSync, writeFileSync, existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import type { UserAction, RecordingSession, ParsedApiRequest } from './types.js';
import { analyzeHar } from './har-analyzer.js';
import { buildIr } from './ir-builder.js';
import { buildRequestTemplates } from './request-templates.js';

export interface RecorderOptions {
  url: string;
  description: string;
  outputDir: string;
  headless?: boolean;
  timeout?: number;
  /** Optional Playwright storage state to load before recording (auth profile). */
  storageStatePath?: string;
}

/**
 * Launch a Playwright browser with HAR recording and action tracking.
 * The user interacts with the browser manually, then presses Enter to stop.
 */
export async function startRecording(opts: RecorderOptions): Promise<RecordingSession> {
  const sessionId = `run-${Date.now()}-${randomUUID().slice(0, 8)}`;
  const sessionDir = join(opts.outputDir, sessionId);
  const harPath = join(sessionDir, 'recording.har');
  const screenshotsDir = join(sessionDir, 'screenshots');

  mkdirSync(sessionDir, { recursive: true });
  mkdirSync(screenshotsDir, { recursive: true });

  console.log(`\n📂 Session directory: ${sessionDir}`);
  console.log(`🌐 Opening browser to: ${opts.url}`);
  console.log(`📝 Task: ${opts.description}\n`);

  // Launch browser (visible so user can interact)
  const browser = await chromium.launch({
    headless: opts.headless ?? false,
    args: [
      '--disable-blink-features=AutomationControlled',
      '--no-sandbox',
      '--disable-dev-shm-usage',
    ],
  });

  const context = await browser.newContext({
    recordHar: { path: harPath, mode: 'full' },
    viewport: { width: 1280, height: 800 },
    userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    storageState: opts.storageStatePath,
  });

  const page = await context.newPage();
  const actions: UserAction[] = [];
  let screenshotCount = 0;

  // Track navigation
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

  // Inject action recording script into each page (using string to avoid TS issues with browser code)
  await context.addInitScript(`
    window.__recordedActions = window.__recordedActions || [];

    function getBestSelector(el) {
      if (el.id) return '#' + el.id;
      var testId = el.getAttribute('data-testid');
      if (testId) return '[data-testid="' + testId + '"]';
      var ariaLabel = el.getAttribute('aria-label');
      if (ariaLabel) return '[aria-label="' + ariaLabel + '"]';
      var name = el.getAttribute('name');
      if (name) return '[name="' + name + '"]';
      if (el.tagName.toLowerCase() === 'input') {
        var placeholder = el.getAttribute('placeholder');
        if (placeholder) return 'input[placeholder="' + placeholder + '"]';
        var type = el.getAttribute('type');
        if (type) return 'input[type="' + type + '"]';
      }
      var role = el.getAttribute('role');
      if (role) {
        var text = (el.textContent || '').trim().slice(0, 50);
        if (text) return '[role="' + role + '"]';
        return '[role="' + role + '"]';
      }
      if (['A', 'BUTTON', 'SPAN'].indexOf(el.tagName) >= 0) {
        var btnText = (el.textContent || '').trim().slice(0, 50);
        if (btnText) return el.tagName.toLowerCase() + ':has-text("' + btnText + '")';
      }
      var parts = [];
      var cur = el;
      while (cur && cur !== document.body) {
        var tag = cur.tagName.toLowerCase();
        var parent = cur.parentElement;
        if (parent) {
          var siblings = Array.from(parent.children).filter(function(c) { return c.tagName === cur.tagName; });
          if (siblings.length > 1) {
            var idx = siblings.indexOf(cur) + 1;
            parts.unshift(tag + ':nth-of-type(' + idx + ')');
          } else {
            parts.unshift(tag);
          }
        } else {
          parts.unshift(tag);
        }
        cur = parent;
      }
      return parts.join(' > ');
    }

    document.addEventListener('click', function(e) {
      var target = e.target;
      if (!target || !target.tagName) return;
      var selector = getBestSelector(target);
      var text = (target.textContent || '').trim().slice(0, 100);
      window.__recordedActions.push({
        type: 'click',
        timestamp: new Date().toISOString(),
        selector: selector,
        tagName: target.tagName.toLowerCase(),
        text: text,
        url: window.location.href
      });
    }, true);

    document.addEventListener('input', function(e) {
      var target = e.target;
      if (!target || !target.tagName) return;
      var selector = getBestSelector(target);
      var tagName = target.tagName.toLowerCase();
      var inputType = target.type || 'text';
      var value = inputType === 'password' ? '***' : target.value;
      var last = window.__recordedActions[window.__recordedActions.length - 1];
      if (last && last.type === 'type' && last.selector === selector) {
        last.value = value;
        last.timestamp = new Date().toISOString();
      } else {
        window.__recordedActions.push({
          type: 'type',
          timestamp: new Date().toISOString(),
          selector: selector,
          tagName: tagName,
          value: value,
          url: window.location.href
        });
      }
    }, true);

    document.addEventListener('keydown', function(e) {
      if (['Enter', 'Escape', 'Tab'].indexOf(e.key) >= 0) {
        var target = e.target;
        var selector = target ? getBestSelector(target) : 'body';
        window.__recordedActions.push({
          type: 'press',
          timestamp: new Date().toISOString(),
          selector: selector,
          key: e.key,
          url: window.location.href
        });
      }
    }, true);
  `);

  // Navigate to the target URL
  try {
    await page.goto(opts.url, { waitUntil: 'domcontentloaded', timeout: 30000 });
  } catch (e) {
    console.log(`  ⚠️  Initial navigation timed out, but page may still be loading...`);
  }

  // Take initial screenshot
  try {
    await page.screenshot({
      path: join(screenshotsDir, `${screenshotCount++}-initial.png`),
    });
  } catch { /* ignore screenshot errors */ }

  console.log('─────────────────────────────────────────────────');
  console.log('🎬 RECORDING IN PROGRESS');
  console.log('');
  console.log('Interact with the browser to perform your task.');
  console.log('The tool is recording all network traffic and actions.');
  console.log('');
  console.log('When done, come back here and press ENTER to stop recording.');
  console.log('─────────────────────────────────────────────────\n');

  // Wait for user to press Enter
  await waitForEnter();

  console.log('\n⏹️  Stopping recording...');

  // Collect actions from the page
  try {
    const pageActions = await page.evaluate(() => {
      return (window as any).__recordedActions || [];
    });
    for (const action of pageActions) {
      actions.push(action as UserAction);
    }
  } catch {
    // Page might have navigated away
  }

  // Take final screenshot
  try {
    await page.screenshot({
      path: join(screenshotsDir, `${screenshotCount++}-final.png`),
    });
  } catch { /* ignore screenshot errors */ }

  // Get cookies + storage state before closing
  const browserCookies = await context.cookies();
  const cookieMap: Record<string, string> = {};
  for (const c of browserCookies) {
    cookieMap[c.name] = c.value;
  }

  const storageStatePath = join(sessionDir, 'storageState.json');
  try {
    await context.storageState({ path: storageStatePath });
  } catch {
    // ignore
  }

  // Close to flush HAR
  await context.close();
  await browser.close();

  console.log('✅ Browser closed. Processing recording data...\n');

  // Read and analyze HAR file
  let harData = { log: { entries: [] as any[] } };
  if (existsSync(harPath)) {
    try {
      harData = JSON.parse(readFileSync(harPath, 'utf-8'));
    } catch {
      console.log('⚠️  Failed to parse HAR file');
    }
  }

  const analysis = analyzeHar(harData, opts.url);

  // Build deterministic IR for codegen (endpoint catalog + auth signals)
  const ir = buildIr(opts.url, analysis.apiRequests, {
    authMethod: analysis.authMethod,
    cookies: { ...cookieMap, ...analysis.cookies },
    authHeaders: analysis.authHeaders,
  });
  writeFileSync(join(sessionDir, 'ir.json'), JSON.stringify(ir, null, 2), 'utf-8');

  // Build request templates for API replay fallback (non-sensitive headers only)
  const requestTemplates = buildRequestTemplates(analysis.apiRequests, 30);
  writeFileSync(join(sessionDir, 'request-templates.json'), JSON.stringify(requestTemplates, null, 2), 'utf-8');

  // Sort actions by timestamp
  actions.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());

  // Build session
  const session: RecordingSession = {
    id: sessionId,
    url: opts.url,
    description: opts.description,
    startTime: actions[0]?.timestamp ?? new Date().toISOString(),
    endTime: new Date().toISOString(),
    actions,
    apiRequests: analysis.apiRequests,
    cookies: { ...cookieMap, ...analysis.cookies },
    authHeaders: analysis.authHeaders,
    authMethod: analysis.authMethod,
    targetDomain: analysis.targetDomain,
    harFilePath: harPath,
    screenshotsDir,
  };

  // Save session metadata
  writeFileSync(
    join(sessionDir, 'session.json'),
    JSON.stringify(session, null, 2),
    'utf-8',
  );

  // Save actions separately for easy review
  writeFileSync(
    join(sessionDir, 'actions.json'),
    JSON.stringify(actions, null, 2),
    'utf-8',
  );

  // Save auth data
  writeFileSync(
    join(sessionDir, 'auth.json'),
    JSON.stringify({
      authHeaders: analysis.authHeaders,
      cookies: { ...cookieMap, ...analysis.cookies },
      playwrightCookies: browserCookies,
      authMethod: analysis.authMethod,
      targetDomain: analysis.targetDomain,
    }, null, 2),
    'utf-8',
  );

  console.log(`📊 Recording Summary:`);
  console.log(`   Actions recorded: ${actions.length}`);
  console.log(`   API requests captured: ${analysis.apiRequests.length}`);
  console.log(`   Auth method: ${analysis.authMethod}`);
  console.log(`   Cookies: ${Object.keys(cookieMap).length}`);
  console.log(`   HAR file: ${harPath}`);
  console.log(`   Session dir: ${sessionDir}\n`);

  return session;
}

/** Wait for the user to press Enter in the terminal. */
function waitForEnter(): Promise<void> {
  return new Promise((resolve) => {
    const { stdin } = process;
    const wasRaw = stdin.isRaw;

    if (stdin.isTTY) {
      stdin.setRawMode(false);
    }

    stdin.resume();
    stdin.once('data', () => {
      if (stdin.isTTY && wasRaw !== undefined) {
        try { stdin.setRawMode(wasRaw); } catch { /* ignore */ }
      }
      stdin.pause();
      resolve();
    });
  });
}
