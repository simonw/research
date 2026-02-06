/**
 * Build request templates suitable for API replay.
 *
 * We intentionally avoid persisting sensitive header values (auth/cookies).
 * Instead we persist:
 * - url + method
 * - a minimal set of non-sensitive headers that improve parity
 * - request body samples (truncated)
 */

import type { ParsedApiRequest } from './types.js';

export interface RequestTemplate {
  id: string;
  method: string;
  url: string;
  /** Header *names* or safe header values. Do not include Cookie/Authorization. */
  headers: Record<string, string>;
  body?: string;
  contentTypeHint?: string;
}

const SENSITIVE_HEADER_PREFIXES = [
  'authorization',
  'cookie',
  'x-api-key',
  'api-key',
  'apikey',
  'x-auth',
  'x-token',
  'x-csrf',
  'csrf',
  'xsrf',
];

function isSensitiveHeader(name: string): boolean {
  const n = name.toLowerCase();
  if (n.startsWith(':')) return true; // h2 pseudo-headers
  return SENSITIVE_HEADER_PREFIXES.some((p) => n === p || n.startsWith(p));
}

function trunc(s?: string, n=4000): string | undefined {
  if (!s) return undefined;
  return s.length <= n ? s : (s.slice(0, n) + '\n... [truncated]');
}

function stableId(method: string, url: string): string {
  const u = new URL(url);
  const key = `${method} ${u.host}${u.pathname}`;
  // low-tech stable id without crypto dep
  let h = 0;
  for (let i=0;i<key.length;i++) h = (h*31 + key.charCodeAt(i)) >>> 0;
  return `rt_${h.toString(16)}`;
}

export function buildRequestTemplates(apiRequests: ParsedApiRequest[], maxTemplates=30): RequestTemplate[] {
  // pick highest-signal requests: JSON-ish and non-static (already filtered)
  const scored = apiRequests.map((r) => {
    const ct = (r.responseContentType || '').toLowerCase();
    const json = ct.includes('json') || (r.responseBody || '').trim().startsWith('{') || (r.responseBody || '').trim().startsWith('[');
    const mut = ['POST','PUT','PATCH','DELETE'].includes(r.method);
    let score = 0;
    if (json) score += 10;
    if (mut) score += 2;
    if (r.url.includes('/graphql')) score += 2;
    return { r, score };
  }).sort((a,b) => b.score - a.score);

  const out: RequestTemplate[] = [];
  const seen = new Set<string>();
  for (const { r } of scored) {
    if (out.length >= maxTemplates) break;
    const id = stableId(r.method, r.url);
    if (seen.has(id)) continue;
    seen.add(id);

    const safeHeaders: Record<string,string> = {};
    for (const [k,v] of Object.entries(r.requestHeaders || {})) {
      if (isSensitiveHeader(k)) continue;
      // keep small headers only
      if (v.length > 200) continue;
      safeHeaders[k] = v;
    }

    out.push({
      id,
      method: r.method,
      url: r.url,
      headers: safeHeaders,
      body: trunc(r.requestBody, 4000),
      contentTypeHint: r.responseContentType,
    });
  }

  return out;
}
