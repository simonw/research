/**
 * Deterministic action cache — skeleton for caching exploration actions.
 *
 * Cache key: hash of { canonicalUrl, taskString, variableKeys }.
 * Cache value: ordered list of deterministic actions.
 *
 * This is the storage skeleton only — replay logic is deferred to Phase 3.
 *
 * Adapted from Stagehand's ActCache:
 * @see https://github.com/browserbase/stagehand/blob/main/packages/core/lib/v3/cache/ActCache.ts
 * Lines L40–L141 — prepareContext, tryReplay, replayCachedActions
 *
 * And Skyvern's step-level retry pattern:
 * @see https://github.com/Skyvern-AI/skyvern/blob/main/skyvern/forge/agent.py
 * Lines L3962–L4010 — handle_failed_step: retry cap, create next step
 */

import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { homedir } from 'node:os';

const CACHE_DIR = join(homedir(), '.data-agent', 'action-cache');

export interface CachedAction {
  action: string;
  selector?: string;
  text?: string;
  url?: string;
  key?: string;
}

export interface ActionCacheEntry {
  cacheKey: string;
  canonicalUrl: string;
  task: string;
  actions: CachedAction[];
  createdAt: string;
  hitCount: number;
  lastHitAt?: string;
}

/**
 * Compute a stable cache key from the URL + task + variable keys.
 */
export function computeCacheKey(
  url: string,
  task: string,
  variableKeys: string[] = [],
): string {
  // Canonicalize URL: strip volatile query params
  const canonical = canonicalizeUrl(url);
  const payload = JSON.stringify({ url: canonical, task: task.toLowerCase().trim(), variableKeys });
  return createHash('sha256').update(payload).digest('hex').slice(0, 16);
}

/**
 * Canonicalize a URL by stripping volatile query params.
 */
function canonicalizeUrl(url: string): string {
  try {
    const u = new URL(url);
    // Strip commonly volatile params
    const volatile = ['_t', '_ts', 'timestamp', 'nonce', 'rand', 'cb', '_'];
    for (const p of volatile) {
      u.searchParams.delete(p);
    }
    // Sort remaining params for stability
    u.searchParams.sort();
    return u.toString();
  } catch {
    return url;
  }
}

/**
 * Try to find a cached action sequence for the given key.
 * Returns null if no cache hit.
 */
export function tryCacheHit(cacheKey: string): ActionCacheEntry | null {
  ensureCacheDir();
  const filePath = join(CACHE_DIR, `${cacheKey}.json`);
  if (!existsSync(filePath)) return null;

  try {
    const entry: ActionCacheEntry = JSON.parse(readFileSync(filePath, 'utf-8'));
    // Update hit count
    entry.hitCount++;
    entry.lastHitAt = new Date().toISOString();
    writeFileSync(filePath, JSON.stringify(entry, null, 2), 'utf-8');
    return entry;
  } catch {
    return null;
  }
}

/**
 * Record a set of actions to the cache.
 */
export function recordActions(
  cacheKey: string,
  canonicalUrl: string,
  task: string,
  actions: CachedAction[],
): void {
  ensureCacheDir();
  const entry: ActionCacheEntry = {
    cacheKey,
    canonicalUrl,
    task,
    actions,
    createdAt: new Date().toISOString(),
    hitCount: 0,
  };
  const filePath = join(CACHE_DIR, `${cacheKey}.json`);
  writeFileSync(filePath, JSON.stringify(entry, null, 2), 'utf-8');
}

function ensureCacheDir(): void {
  if (!existsSync(CACHE_DIR)) {
    mkdirSync(CACHE_DIR, { recursive: true });
  }
}
