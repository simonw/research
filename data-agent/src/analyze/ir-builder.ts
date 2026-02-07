/**
 * IR Builder — Build a deterministic Intermediate Representation from API requests.
 *
 * Normalizes paths, scores endpoints by usefulness, and creates stable IDs.
 */

import crypto from 'node:crypto';
import type { ParsedApiRequest, EndpointGroup, EndpointVariant, RunIr, HttpMethod } from '../types.js';
import { IR_VERSION } from '../types.js';
import { extractResponseSchema, renderSchemaForLLM } from './response-schema.js';

export function normalizePath(pathWithQuery: string): string {
  const path = pathWithQuery.split('?')[0] ?? pathWithQuery;
  return path
    .replace(/\/[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}/gi, '/{uuid}')
    .replace(/\/[0-9]+/g, '/{id}')
    .replace(/\/[A-Za-z0-9_-]{20,}/g, '/{token}');
}

function isJsonLike(contentType?: string, body?: string): boolean {
  const ct = (contentType || '').toLowerCase();
  if (ct.includes('application/json') || ct.includes('text/json')) return true;
  if (!body) return false;
  const t = body.trim();
  return t.startsWith('{') || t.startsWith('[') || t.startsWith('for (;;);{') || t.startsWith('for (;;);[');
}

function isHtmlLike(contentType?: string, body?: string): boolean {
  const ct = (contentType || '').toLowerCase();
  if (ct.includes('text/html') || ct.includes('application/xhtml')) return true;
  if (!body) return false;
  const t = body.trim().toLowerCase();
  return t.startsWith('<!doctype html') || t.startsWith('<html');
}

function isApiLike(url: string): boolean {
  const u = url.toLowerCase();
  return u.includes('/api/') || u.includes('/graphql') || u.includes('/v1/') || u.includes('/v2/') || u.includes('/rpc/') || u.includes('/ajax/');
}

function stripPseudoHeaders(h?: Record<string, string>): Record<string, string> | undefined {
  if (!h) return undefined;
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(h)) {
    if (k.toLowerCase().startsWith(':')) continue;
    out[k] = v;
  }
  return out;
}

function trunc(s?: string, n = 2000): string | undefined {
  if (!s) return undefined;
  if (s.length <= n) return s;
  return s.slice(0, n) + '\n... [truncated]';
}

const TELEMETRY_PATTERNS = [
  '/collect', '/track', '/rgstr', '/analytics', '/log/', '/beacon',
  '/ping', '/metrics', '/telemetry', '/sentry', '/error-report',
];

function scoreEndpoint(group: {
  method: string;
  pathPattern: string;
  callCount: number;
  isJsonLike: boolean;
  isHtmlLike: boolean;
  isApiLike: boolean;
  sample?: ParsedApiRequest;
}): { score: number; reasons: string[] } {
  let score = 0;
  const reasons: string[] = [];

  if (group.isJsonLike) { score += 50; reasons.push('json_like:+50'); }
  if (group.isApiLike) { score += 20; reasons.push('api_like:+20'); }
  if (['POST', 'PUT', 'PATCH', 'DELETE'].includes(group.method)) { score += 10; reasons.push('mutating:+10'); }
  if (group.callCount > 1) { score += Math.min(20, group.callCount); reasons.push(`frequency:+${Math.min(20, group.callCount)}`); }
  if (group.isHtmlLike) { score -= 80; reasons.push('html_like:-80'); }

  const body = group.sample?.responseBody;
  if (body) {
    const t = body.trim();
    if (t.startsWith('[')) { score += 10; reasons.push('array_response:+10'); }
    if (t.includes('edges') && t.includes('page_info')) { score += 10; reasons.push('graphql_pagination_hints:+10'); }
    if (t.includes('next') && t.includes('cursor')) { score += 5; reasons.push('cursor_hints:+5'); }
    if (t.includes('"items"') || t.includes('"data"') || t.includes('"results"')) { score += 15; reasons.push('data_list:+15'); }
    if (t === '{"success":true}' || t === '{}' || t === 'ok' || t === '""') { score -= 40; reasons.push('trivial_response:-40'); }
  }

  const pathLower = group.pathPattern.toLowerCase();
  if (TELEMETRY_PATTERNS.some(p => pathLower.includes(p))) { score -= 30; reasons.push('telemetry_path:-30'); }

  return { score, reasons };
}

function stableId(method: string, domain: string, pathPattern: string): string {
  const h = crypto.createHash('sha256').update(`${method} ${domain}${pathPattern}`).digest('hex');
  return h.slice(0, 12);
}

export function buildIr(
  seedUrl: string,
  apiRequests: ParsedApiRequest[],
  auth: { authMethod: string; cookies: Record<string, string>; authHeaders: Record<string, string> },
): RunIr {
  let seedDomain = '';
  try { seedDomain = new URL(seedUrl).hostname; } catch {}

  const byKey = new Map<string, ParsedApiRequest[]>();
  for (const r of apiRequests) {
    const method = r.method as HttpMethod;
    const pathPattern = normalizePath(r.path);
    const key = `${method} ${r.domain}${pathPattern}`;
    const arr = byKey.get(key) || [];
    arr.push(r);
    byKey.set(key, arr);
  }

  const endpoints: EndpointGroup[] = [];
  for (const [, reqs] of byKey.entries()) {
    const sample = reqs[0];
    const method = sample.method as HttpMethod;
    const pathPattern = normalizePath(sample.path);
    const jsonLike = isJsonLike(sample.responseContentType, sample.responseBody);
    const htmlLike = isHtmlLike(sample.responseContentType, sample.responseBody);
    const apiLikeFlag = isApiLike(sample.url);

    const { score, reasons } = scoreEndpoint({
      method, pathPattern, callCount: reqs.length,
      isJsonLike: jsonLike, isHtmlLike: htmlLike, isApiLike: apiLikeFlag, sample,
    });

    const variants: EndpointVariant[] = reqs.slice(0, 2).map(r => ({
      exampleUrl: r.url,
      status: r.status,
      contentType: r.responseContentType,
      requestBodySample: trunc(r.requestBody, 1200),
      responseBodySample: trunc(r.responseBody, 2000),
      requestHeadersSample: stripPseudoHeaders(r.requestHeaders),
    }));

    endpoints.push({
      id: stableId(method, sample.domain, pathPattern),
      method, domain: sample.domain, pathPattern,
      callCount: reqs.length, score, scoreReasons: reasons,
      isHtmlLike: htmlLike, isJsonLike: jsonLike, isApiLike: apiLikeFlag,
      variants,
    });
  }

  endpoints.sort((a, b) => b.score - a.score || b.callCount - a.callCount || a.id.localeCompare(b.id));

  return {
    irVersion: IR_VERSION, seedUrl, seedDomain,
    createdAt: new Date().toISOString(),
    auth: {
      authMethod: auth.authMethod,
      cookieNames: Object.keys(auth.cookies).sort(),
      authHeaderNames: Object.keys(auth.authHeaders).sort(),
    },
    endpoints,
  };
}

/** Standard browser headers to exclude from auth-header extraction. */
const STANDARD_HEADERS = new Set([
  'accept', 'accept-encoding', 'accept-language',
  'content-type', 'content-length',
  'cookie', 'host', 'origin', 'referer',
  'user-agent', 'connection', 'keep-alive',
  'cache-control', 'pragma', 'priority',
  'sec-ch-ua', 'sec-ch-ua-mobile', 'sec-ch-ua-platform',
  'sec-fetch-dest', 'sec-fetch-mode', 'sec-fetch-site', 'sec-fetch-user',
  'upgrade-insecure-requests', 'dnt', 'te',
]);

function extractAuthHeaders(headers: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(headers)) {
    const kl = k.toLowerCase();
    if (kl.startsWith(':') || kl.startsWith('sec-') || STANDARD_HEADERS.has(kl)) continue;
    if (kl === 'authorization') {
      out[k] = v.slice(0, 60) + (v.length > 60 ? '...' : '');
    } else {
      out[k] = v.slice(0, 80) + (v.length > 80 ? '...' : '');
    }
  }
  return out;
}

export function summarizeIrForLLM(ir: RunIr, maxEndpoints = 25): string {
  const lines: string[] = [];
  lines.push(`IR v${ir.irVersion} seed=${ir.seedDomain}`);
  lines.push(`AuthMethod=${ir.auth.authMethod} cookies=${ir.auth.cookieNames.length} authHeaders=[${ir.auth.authHeaderNames.join(', ')}]`);
  lines.push('Top endpoints:');

  for (const [i, ep] of ir.endpoints.slice(0, maxEndpoints).entries()) {
    lines.push(`- score=${ep.score} calls=${ep.callCount} ${ep.method} ${ep.domain}${ep.pathPattern} json=${ep.isJsonLike} api=${ep.isApiLike} html=${ep.isHtmlLike}`);

    const charLimit = i < 5 ? 8000 : i < 15 ? 4000 : 2000;

    if (i < 15 && ep.variants.length > 0) {
      const v = ep.variants[0];
      lines.push(`  url: ${v.exampleUrl?.slice(0, 200)}`);

      if (v.requestHeadersSample) {
        const authHdrs = extractAuthHeaders(v.requestHeadersSample);
        if (Object.keys(authHdrs).length > 0) {
          lines.push(`  auth-headers: ${JSON.stringify(authHdrs)}`);
        }
      }

      if (v.responseBodySample) {
        if (i < 10 && v.responseBodySample.length > 500) {
          const schema = extractResponseSchema(v.responseBodySample);
          if (schema) {
            lines.push(`  response-schema:\n${renderSchemaForLLM(schema, 2)}`);
          } else {
            lines.push(`  response: ${v.responseBodySample.slice(0, charLimit)}${v.responseBodySample.length > charLimit ? '...' : ''}`);
          }
        } else {
          lines.push(`  response: ${v.responseBodySample.slice(0, charLimit)}${v.responseBodySample.length > charLimit ? '...' : ''}`);
        }
      }
    }
  }
  return lines.join('\n');
}
