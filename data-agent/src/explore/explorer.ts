/**
 * Explorer — AI agent loop for autonomous site exploration.
 *
 * Launches a browser, navigates to a site, and uses an LLM to decide
 * what actions to take. Records all API traffic via HAR and tracks
 * observed API calls for pattern detection.
 *
 * Inspired by Skyvern's agent_step() loop: plan → act → observe → record → evaluate.
 */

import { chromium } from 'playwright';
import { readFileSync } from 'node:fs';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { getEnhancedSnapshot, getSnapshotStats } from '../browser/snapshot.js';
import type {
  ActionWithIntent,
  ExploreDecision,
  ExploreResult,
  LlmProvider,
  ObservedApiCall,
} from '../types.js';

const MAX_STEPS = 30;

interface ExploreOptions {
  task: string;
  url: string;
  llm: LlmProvider;
  chromePath?: string;
  headless?: boolean;
  authStatePath?: string;
  sessionsDir: string;
}

/** Check if a response looks like an API response (JSON, non-trivial). */
function isApiResponse(url: string, status: number, contentType: string | null): boolean {
  // Skip static assets
  if (/\.(css|js|png|jpg|gif|svg|woff|ico|map)(\?|$)/i.test(url)) return false;

  // Skip common tracking/analytics
  const skipDomains = [
    'google-analytics.com', 'googletagmanager.com', 'facebook.com',
    'sentry.io', 'mixpanel.com', 'segment.io', 'hotjar.com',
    'cdn.jsdelivr.net', 'unpkg.com', 'cdnjs.cloudflare.com',
  ];
  try {
    const hostname = new URL(url).hostname;
    if (skipDomains.some(d => hostname.includes(d))) return false;
  } catch { return false; }

  // Must have a reasonable status
  if (status < 200 || status >= 600) return false;

  // Prefer JSON responses but also capture other API-like responses
  const ct = contentType?.toLowerCase() ?? '';
  if (ct.includes('json')) return true;
  if (ct.includes('text/html')) return false;
  if (ct.includes('text/css')) return false;
  if (ct.includes('javascript')) return false;

  // API-like paths
  if (url.includes('/api/') || url.includes('/graphql') || url.includes('/v1/') || url.includes('/v2/')) {
    return true;
  }

  return false;
}

/**
 * Load the explore system prompt from prompts/explore.md
 */
function loadExplorePrompt(): string {
  try {
    // Try relative to the package root
    const promptPath = resolve(import.meta.dirname, '../../prompts/explore.md');
    return readFileSync(promptPath, 'utf-8');
  } catch {
    // Fallback inline prompt
    return 'You are a web exploration agent. Respond with JSON actions.';
  }
}

/**
 * Run the explore agent loop.
 *
 * Launches browser → navigates → agent loop (snapshot → LLM → act → record) → close.
 */
export async function explore(options: ExploreOptions): Promise<ExploreResult> {
  const { task, url, llm, chromePath, headless = false, authStatePath, sessionsDir } = options;

  // Create session directory
  const sessionId = `session-${Date.now()}`;
  const sessionDir = join(sessionsDir, sessionId);
  mkdirSync(sessionDir, { recursive: true });

  const harPath = join(sessionDir, 'recording.har');

  console.log(`  Session: ${sessionDir}`);
  console.log(`  HAR: ${harPath}`);

  // Launch browser
  const launchOptions: Record<string, unknown> = { headless };
  if (chromePath) {
    launchOptions.executablePath = chromePath;
  }

  const browser = await chromium.launch(launchOptions);

  const contextOptions: Record<string, unknown> = {
    recordHar: { path: harPath, mode: 'full' },
  };
  if (authStatePath) {
    contextOptions.storageState = authStatePath;
  }

  const context = await browser.newContext(contextOptions);
  const page = await context.newPage();

  const actions: ActionWithIntent[] = [];
  const apisSeen: ObservedApiCall[] = [];

  // Track all API responses
  page.on('response', (r) => {
    const respUrl = r.url();
    const status = r.status();
    const contentType = r.headers()['content-type'] ?? null;

    if (isApiResponse(respUrl, status, contentType)) {
      apisSeen.push({
        url: respUrl,
        method: r.request().method(),
        status,
        contentType: contentType ?? undefined,
        timestamp: Date.now(),
      });
    }
  });

  // Navigate to starting URL
  console.log(`  Navigating to ${url}...`);
  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30_000 });
  await page.waitForLoadState('networkidle').catch(() => {});

  const systemPrompt = loadExplorePrompt();

  // Agent loop
  for (let step = 0; step < MAX_STEPS; step++) {
    try {
      // 1. OBSERVE: Take compact accessibility snapshot
      const { tree: snapshot, refs: refMap } = await getEnhancedSnapshot(page, {
        interactive: true,
        cursor: true,
        compact: true,
      });

      const stats = getSnapshotStats(snapshot, refMap);
      console.log(`  Step ${step + 1}: snapshot ${stats.refs} refs, ~${stats.tokens} tokens`);

      // 2. PLAN: Ask LLM what to do next
      const userPrompt = buildExplorePrompt(task, snapshot, actions, apisSeen);
      const decision = await llm.generateJson<ExploreDecision>(systemPrompt, userPrompt);

      console.log(`    Action: ${decision.done ? 'DONE' : decision.action} — ${decision.reasoning.slice(0, 80)}`);

      if (decision.done) break;

      // 3. ACT: Execute the decision
      if (decision.action === 'click' && decision.ref) {
        const refData = refMap[decision.ref];
        if (refData) {
          const locator = buildLocator(page, refData);
          await locator.click({ timeout: 10_000 }).catch((err: Error) => {
            console.log(`    Click failed: ${err.message.slice(0, 60)}`);
          });
        } else {
          console.log(`    Ref ${decision.ref} not found in snapshot`);
        }
      } else if (decision.action === 'type' && decision.ref && decision.text) {
        const refData = refMap[decision.ref];
        if (refData) {
          const locator = buildLocator(page, refData);
          await locator.fill(decision.text, { timeout: 10_000 }).catch((err: Error) => {
            console.log(`    Type failed: ${err.message.slice(0, 60)}`);
          });
        }
      } else if (decision.action === 'scroll') {
        await page.evaluate(() => window.scrollBy(0, 500));
      } else if (decision.action === 'navigate' && decision.url) {
        await page.goto(decision.url, { waitUntil: 'domcontentloaded', timeout: 30_000 });
      } else if (decision.action === 'wait') {
        await page.waitForTimeout(2000);
      } else if (decision.action === 'press' && decision.key) {
        await page.keyboard.press(decision.key);
      }

      // Wait for network to settle after action
      await page.waitForLoadState('networkidle').catch(() => {});
      await page.waitForTimeout(500);

      // 4. RECORD: Store action with intent metadata
      actions.push({
        step,
        action: decision.action ?? 'wait',
        ref: decision.ref,
        text: decision.text,
        url: decision.url,
        key: decision.key,
        reasoning: decision.reasoning,
        intention: decision.intention,
        confidence: decision.confidence,
        timestamp: Date.now(),
      });
    } catch (err) {
      console.log(`    Step ${step + 1} error: ${err instanceof Error ? err.message.slice(0, 80) : String(err)}`);
    }
  }

  // Close browser and save HAR
  await context.close();
  await browser.close();

  // Save session metadata
  const sessionMeta = {
    id: sessionId,
    task,
    url,
    actions,
    apisSeen,
    createdAt: new Date().toISOString(),
  };
  writeFileSync(join(sessionDir, 'session.json'), JSON.stringify(sessionMeta, null, 2));

  console.log(`  Explore complete: ${actions.length} actions, ${apisSeen.length} APIs observed`);

  return { harPath, actions, apisSeen, sessionDir };
}

/**
 * Build the user prompt for an explore step.
 */
function buildExplorePrompt(
  task: string,
  snapshot: string,
  actions: ActionWithIntent[],
  apisSeen: ObservedApiCall[],
): string {
  const parts: string[] = [];

  parts.push(`## Task\n${task}\n`);

  parts.push(`## Current Page Snapshot\n${snapshot}\n`);

  if (actions.length > 0) {
    parts.push('## Actions Taken So Far');
    for (const a of actions.slice(-10)) {
      parts.push(`${a.step + 1}. ${a.action}${a.ref ? ` @${a.ref}` : ''}${a.text ? ` "${a.text}"` : ''} — ${a.intention}`);
    }
    parts.push('');
  }

  if (apisSeen.length > 0) {
    parts.push('## API Calls Observed');
    // Deduplicate by URL pattern
    const seen = new Map<string, { method: string; status: number; count: number }>();
    for (const api of apisSeen) {
      const key = `${api.method} ${api.url.split('?')[0]}`;
      const existing = seen.get(key);
      if (existing) {
        existing.count++;
      } else {
        seen.set(key, { method: api.method, status: api.status, count: 1 });
      }
    }
    for (const [pattern, info] of seen) {
      parts.push(`- ${pattern} (${info.status}) x${info.count}`);
    }
    parts.push('');
  }

  parts.push(`Step ${actions.length + 1}: What should I do next? Respond with JSON.`);

  return parts.join('\n');
}

/**
 * Build a Playwright locator from ref data.
 */
function buildLocator(page: import('playwright').Page, refData: { selector: string; role: string; name?: string; nth?: number }) {
  // For cursor-interactive elements, use CSS selector directly
  if (refData.role === 'clickable' || refData.role === 'focusable') {
    return page.locator(refData.selector);
  }

  // For ARIA elements, use getByRole
  const roleOptions: { name?: string; exact?: boolean } = {};
  if (refData.name) {
    roleOptions.name = refData.name;
    roleOptions.exact = true;
  }

  let locator = page.getByRole(refData.role as any, roleOptions);

  if (refData.nth !== undefined) {
    locator = locator.nth(refData.nth);
  }

  return locator;
}
