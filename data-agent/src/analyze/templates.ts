/**
 * Request Template Builder — Build sanitized request templates for API replay.
 *
 * Deliberately excludes sensitive headers (auth/cookies).
 */

import type { ParsedApiRequest, RequestTemplate } from '../types.js';

const SENSITIVE_HEADER_PREFIXES = [
  'authorization', 'cookie', 'x-api-key', 'api-key', 'apikey',
  'x-auth', 'x-token', 'x-csrf', 'csrf', 'xsrf',
];

function isSensitiveHeader(name: string): boolean {
  const n = name.toLowerCase();
  if (n.startsWith(':')) return true;
  return SENSITIVE_HEADER_PREFIXES.some(p => n === p || n.startsWith(p));
}

function trunc(s?: string, n = 4000): string | undefined {
  if (!s) return undefined;
  return s.length <= n ? s : s.slice(0, n) + '\n... [truncated]';
}

function stableId(method: string, url: string): string {
  const u = new URL(url);
  const key = `${method} ${u.host}${u.pathname}`;
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) >>> 0;
  return `rt_${h.toString(16)}`;
}

export function buildRequestTemplates(apiRequests: ParsedApiRequest[], maxTemplates = 30): RequestTemplate[] {
  const scored = apiRequests.map(r => {
    const ct = (r.responseContentType || '').toLowerCase();
    const json = ct.includes('json') || (r.responseBody || '').trim().startsWith('{') || (r.responseBody || '').trim().startsWith('[');
    const mut = ['POST', 'PUT', 'PATCH', 'DELETE'].includes(r.method);
    let score = 0;
    if (json) score += 10;
    if (mut) score += 2;
    if (r.url.includes('/graphql')) score += 2;
    return { r, score };
  }).sort((a, b) => b.score - a.score);

  const out: RequestTemplate[] = [];
  const seen = new Set<string>();

  for (const { r } of scored) {
    if (out.length >= maxTemplates) break;
    const id = stableId(r.method, r.url);
    if (seen.has(id)) continue;
    seen.add(id);

    const safeHeaders: Record<string, string> = {};
    for (const [k, v] of Object.entries(r.requestHeaders || {})) {
      if (isSensitiveHeader(k)) continue;
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
