/**
 * Network-quiet DOM settle helper.
 *
 * Waits until the page has zero in-flight network requests for a configurable
 * quiet window. Ignores WebSocket/EventSource connections and sweeps stalled
 * requests so we never deadlock on slow third-party scripts.
 *
 * Adapted from Stagehand's waitForDomNetworkQuiet():
 * @see https://github.com/browserbase/stagehand/blob/main/packages/core/lib/v3/handlers/handlerUtils/actHandlerUtils.ts
 * Lines L537–L679 — Network-quiet DOM settle (bounded, ignores WebSocket/SSE, stall sweeper)
 */

import type { Page } from 'playwright';

export interface NetworkSettleOptions {
  /** Quiet window in ms — how long zero requests must hold (default 500). */
  quietMs?: number;
  /** Hard timeout bound in ms — return even if still noisy (default 5000). */
  timeoutMs?: number;
  /** Stall threshold in ms — requests older than this are force-resolved (default 2000). */
  stallThresholdMs?: number;
}

/**
 * Wait for the page's network to become quiet (zero in-flight requests)
 * for at least `quietMs` milliseconds.
 *
 * - Ignores WebSocket and EventSource connections (they never "complete").
 * - Stall sweeper: any request older than `stallThresholdMs` is force-counted
 *   as done so we never deadlock on slow third-party scripts.
 * - Hard timeout bound: returns after `timeoutMs` even if still noisy.
 */
export async function waitForNetworkSettle(
  page: Page,
  options: NetworkSettleOptions = {},
): Promise<void> {
  const {
    quietMs = 500,
    timeoutMs = 5000,
    stallThresholdMs = 2000,
  } = options;

  return new Promise<void>((resolve) => {
    const inflight = new Map<string, number>(); // url → start timestamp
    let quietTimer: ReturnType<typeof setTimeout> | null = null;
    let hardTimer: ReturnType<typeof setTimeout> | null = null;
    let settled = false;

    const cleanup = () => {
      if (settled) return;
      settled = true;
      if (quietTimer) clearTimeout(quietTimer);
      if (hardTimer) clearTimeout(hardTimer);
      page.removeListener('request', onRequest);
      page.removeListener('requestfinished', onRequestDone);
      page.removeListener('requestfailed', onRequestDone);
      resolve();
    };

    const sweepStalled = () => {
      const now = Date.now();
      for (const [url, startTime] of inflight) {
        if (now - startTime > stallThresholdMs) {
          inflight.delete(url);
        }
      }
    };

    const checkQuiet = () => {
      if (settled) return;
      sweepStalled();
      if (inflight.size === 0) {
        if (!quietTimer) {
          quietTimer = setTimeout(cleanup, quietMs);
        }
      } else {
        if (quietTimer) {
          clearTimeout(quietTimer);
          quietTimer = null;
        }
      }
    };

    const isIgnored = (url: string): boolean => {
      // Ignore WebSocket and EventSource URLs
      if (url.startsWith('ws://') || url.startsWith('wss://')) return true;
      // Ignore data: URLs
      if (url.startsWith('data:')) return true;
      return false;
    };

    const onRequest = (request: { url: () => string; resourceType: () => string }) => {
      const url = request.url();
      if (isIgnored(url)) return;
      // Ignore websocket resource type (Playwright marks them)
      if (request.resourceType() === 'websocket' || request.resourceType() === 'eventsource') return;
      inflight.set(url, Date.now());
      if (quietTimer) {
        clearTimeout(quietTimer);
        quietTimer = null;
      }
    };

    const onRequestDone = (request: { url: () => string }) => {
      inflight.delete(request.url());
      checkQuiet();
    };

    page.on('request', onRequest);
    page.on('requestfinished', onRequestDone);
    page.on('requestfailed', onRequestDone);

    // Hard timeout — never wait forever
    hardTimer = setTimeout(cleanup, timeoutMs);

    // If network is already quiet, start the quiet timer immediately
    checkQuiet();
  });
}
