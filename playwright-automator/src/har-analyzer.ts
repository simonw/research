/**
 * HAR Analyzer — Parse and analyze HAR files to extract API endpoints.
 *
 * Inspired by unbrowse-openclaw's har-parser.ts, adapted for script generation.
 * Focuses on identifying the most useful API endpoints for automation.
 */

import type { HarEntry, ParsedApiRequest } from './types.js';
import { extractResponseSchema, renderSchemaForLLM } from './response-schema.js';

/** Static asset extensions to skip. */
const STATIC_EXTS = [
  '.css', '.js', '.mjs', '.png', '.jpg', '.jpeg', '.gif', '.svg',
  '.woff', '.woff2', '.ttf', '.eot', '.ico', '.map', '.webp', '.avif',
];

/** Third-party domains to skip. */
const SKIP_DOMAINS = [
  'google-analytics.com', 'analytics.google.com', 'googletagmanager.com',
  'googlesyndication.com', 'doubleclick.net', 'google.com', 'googleapis.com',
  'gstatic.com', 'facebook.com', 'connect.facebook.net', 'fbcdn.net',
  'mixpanel.com', 'segment.io', 'segment.com', 'amplitude.com',
  'hotjar.com', 'clarity.ms', 'sentry.io', 'newrelic.com', 'nr-data.net',
  'datadoghq.com', 'bugsnag.com', 'launchdarkly.com', 'fullstory.com',
  'intercom.io', 'stripe.com', 'onetrust.com', 'cookielaw.org',
  'cdn.jsdelivr.net', 'unpkg.com', 'cdnjs.cloudflare.com',
  'posthog.com', 'heapanalytics.com', 'plausible.io',
  'twitter.com', 'platform.twitter.com', 'ads-twitter.com',
  'tiktok.com', 'tiktokcdn.com', 'byteoversea.com',
  'appsflyer.com', 'branch.io', 'adjust.com',
  'cdn-cgi', 'challenges.cloudflare.com',
];

/** Path prefixes to skip. */
const SKIP_PATHS = [
  '/cdn-cgi/', '/_next/static/', '/__nextjs', '/sockjs-node/',
  '/favicon', '/manifest.json', '/robots.txt', '/sitemap',
  '/_next/image', '/static/chunks/', '/static/media/',
  '/.well-known/',
];

/** Auth-related header names. */
const AUTH_HEADER_NAMES = new Set([
  'authorization', 'x-api-key', 'api-key', 'apikey',
  'x-auth-token', 'access-token', 'x-access-token',
  'token', 'x-token', 'x-csrf-token', 'csrf-token',
  'x-xsrf-token', 'xsrf-token', 'x-session-token',
  'x-ig-app-id', 'x-instagram-ajax', 'x-asbd-id', 'x-csrftoken',
  'x-requested-with',
]);

/** Auth-related header patterns. */
const AUTH_PATTERNS = [
  'auth', 'token', 'key', 'secret', 'bearer', 'jwt',
  'session', 'csrf', 'xsrf', 'cookie',
];

/** Standard browser headers that are NOT site-specific API headers. */
const STANDARD_HEADERS = new Set([
  'accept', 'accept-encoding', 'accept-language',
  'content-type', 'content-length',
  'cookie', 'host', 'origin', 'referer',
  'user-agent', 'connection', 'keep-alive',
  'cache-control', 'pragma', 'priority',
  'sec-ch-ua', 'sec-ch-ua-mobile', 'sec-ch-ua-platform',
  'sec-fetch-dest', 'sec-fetch-mode', 'sec-fetch-site', 'sec-fetch-user',
  'upgrade-insecure-requests', 'dnt', 'te',
  'if-match', 'if-modified-since', 'if-none-match', 'if-range', 'if-unmodified-since',
]);

function isAuthHeader(name: string): boolean {
  const lower = name.toLowerCase();
  if (AUTH_HEADER_NAMES.has(lower)) return true;
  return AUTH_PATTERNS.some(p => lower.includes(p));
}

/** Check if a header is a non-standard, site-specific API header (e.g. x-device-id, x-client-version). */
function isCustomApiHeader(name: string): boolean {
  const lower = name.toLowerCase();
  if (lower.startsWith(':')) return false; // pseudo-headers
  if (lower.startsWith('sec-')) return false; // browser security headers
  if (STANDARD_HEADERS.has(lower)) return false;
  return true;
}

function isStaticAsset(url: string): boolean {
  try {
    const path = new URL(url).pathname.toLowerCase();
    if (STATIC_EXTS.some(ext => path.endsWith(ext))) return true;
    if (SKIP_PATHS.some(prefix => path.startsWith(prefix))) return true;
    return false;
  } catch {
    return true;
  }
}

function isSkippedDomain(domain: string): boolean {
  return SKIP_DOMAINS.some(skip => domain.includes(skip));
}

function isHtmlResponse(entry: HarEntry): boolean {
  const ct = entry.response?.headers?.find(
    h => h.name.toLowerCase() === 'content-type'
  )?.value;
  if (!ct) return false;
  return ct.includes('text/html') || ct.includes('application/xhtml');
}

function isJsonResponse(entry: HarEntry): boolean {
  const ct = entry.response?.headers?.find(
    h => h.name.toLowerCase() === 'content-type'
  )?.value;
  if (!ct) return false;
  return ct.includes('application/json') || ct.includes('text/json');
}

function getRootDomain(domain: string): string {
  const parts = domain.split('.');
  if (parts.length >= 2) return parts.slice(-2).join('.');
  return domain;
}

/**
 * Analyze a HAR file and extract API requests relevant to automation.
 *
 * @param harData - Parsed HAR JSON
 * @param seedUrl - The URL the user visited (used to identify target domain)
 * @returns Object with parsed API requests, auth info, and metadata
 */
export function analyzeHar(
  harData: { log: { entries: HarEntry[] } },
  seedUrl: string,
): {
  apiRequests: ParsedApiRequest[];
  authHeaders: Record<string, string>;
  cookies: Record<string, string>;
  authMethod: string;
  targetDomain: string;
  allDomains: string[];
  summary: string;
} {
  const apiRequests: ParsedApiRequest[] = [];
  const authHeaders: Record<string, string> = {};
  const cookies: Record<string, string> = {};
  const seenDomains = new Set<string>();

  let seedDomain: string;
  try {
    seedDomain = new URL(seedUrl).hostname;
  } catch {
    seedDomain = '';
  }
  const seedRoot = getRootDomain(seedDomain);

  for (const entry of harData.log?.entries ?? []) {
    const { request, response } = entry;
    const url = request.url;
    const method = request.method;

    // Skip static assets
    if (isStaticAsset(url)) continue;

    let parsed: URL;
    try {
      parsed = new URL(url);
    } catch {
      continue;
    }

    const domain = parsed.hostname;

    // Skip third-party tracking/analytics
    if (isSkippedDomain(domain)) continue;

    // Skip HTML page navigations (GET only)
    if (method === 'GET' && isHtmlResponse(entry)) continue;

    seenDomains.add(domain);

    // Only include requests related to the target domain or that return JSON
    const isRelated = getRootDomain(domain) === seedRoot;
    const isJson = isJsonResponse(entry);
    const isApiPath = url.includes('/api/') || url.includes('/v1/') ||
      url.includes('/v2/') || url.includes('/graphql') ||
      domain.startsWith('api.') || domain.includes('api');
    const isMutating = ['POST', 'PUT', 'DELETE', 'PATCH'].includes(method);

    if (!isRelated && !isJson && !isApiPath && !isMutating) continue;

    // Extract auth headers + custom API headers from first-party requests
    const reqHeaders: Record<string, string> = {};
    for (const header of request.headers ?? []) {
      const name = header.name;
      const lowerName = name.toLowerCase();

      // Skip HTTP/2 pseudo-headers
      if (lowerName.startsWith(':')) continue;

      reqHeaders[name] = header.value;

      if (isAuthHeader(lowerName)) {
        authHeaders[lowerName] = header.value;
      } else if (isRelated && isCustomApiHeader(lowerName)) {
        // Capture site-specific API headers from first-party requests (device IDs, client versions, etc.)
        authHeaders[lowerName] = header.value;
      }
    }

    // Extract cookies
    for (const cookie of request.cookies ?? []) {
      cookies[cookie.name] = cookie.value;
    }

    // Get response body — use schema-aware truncation for large responses
    let responseBody: string | undefined;
    if (response?.content?.text) {
      responseBody = response.content.text;
      if (responseBody.length > 10000) {
        const sizeKB = Math.round(responseBody.length / 1024);
        const schema = extractResponseSchema(responseBody);
        if (schema) {
          responseBody = `[SCHEMA - ${sizeKB}KB response]\n${renderSchemaForLLM(schema)}`;
        } else {
          responseBody = responseBody.slice(0, 5000) + '\n... [truncated]';
        }
      }
    }

    // Extract query params
    const queryParams: Record<string, string> = {};
    for (const param of request.queryString ?? []) {
      queryParams[param.name] = param.value;
    }

    const responseContentType = response?.headers?.find(
      h => h.name.toLowerCase() === 'content-type'
    )?.value;

    apiRequests.push({
      method,
      url,
      path: parsed.pathname + parsed.search,
      domain,
      status: response?.status ?? 0,
      requestHeaders: reqHeaders,
      requestBody: request.postData?.text,
      responseContentType,
      responseBody,
      responseSize: response?.content?.size,
      queryParams: Object.keys(queryParams).length > 0 ? queryParams : undefined,
      timestamp: entry.startedDateTime,
    });
  }

  // Determine auth method
  const authMethod = guessAuthMethod(authHeaders, cookies);

  // Generate summary
  const domainCounts: Record<string, number> = {};
  for (const req of apiRequests) {
    domainCounts[req.domain] = (domainCounts[req.domain] ?? 0) + 1;
  }

  const summary = [
    `Total API requests captured: ${apiRequests.length}`,
    `Domains: ${Object.entries(domainCounts).map(([d, c]) => `${d} (${c})`).join(', ')}`,
    `Auth method: ${authMethod}`,
    `Auth headers found: ${Object.keys(authHeaders).length}`,
    `Cookies found: ${Object.keys(cookies).length}`,
  ].join('\n');

  return {
    apiRequests,
    authHeaders,
    cookies,
    authMethod,
    targetDomain: seedDomain,
    allDomains: [...seenDomains],
    summary,
  };
}

function guessAuthMethod(
  authHeaders: Record<string, string>,
  cookies: Record<string, string>,
): string {
  const headerNames = Object.keys(authHeaders);
  const headerValues = Object.values(authHeaders);

  for (const value of headerValues) {
    if (value.toLowerCase().startsWith('bearer ')) return 'Bearer Token';
  }

  if (headerNames.some(h => h.includes('api-key') || h.includes('apikey'))) {
    return 'API Key';
  }

  if (headerNames.includes('authorization')) {
    const val = authHeaders['authorization'];
    if (val?.toLowerCase().startsWith('basic ')) return 'Basic Auth';
    return 'Authorization Header';
  }

  if (headerNames.some(h => h.includes('csrf') || h.includes('xsrf'))) {
    return 'Session + CSRF Token';
  }

  if (headerNames.some(h => h.includes('session') || h.includes('token'))) {
    return 'Custom Token';
  }

  const authCookies = Object.keys(cookies).filter(c => {
    const lower = c.toLowerCase();
    return lower.includes('session') || lower.includes('token') ||
      lower.includes('auth') || lower.includes('jwt');
  });

  if (authCookies.length > 0) {
    return `Cookie-based (${authCookies[0]})`;
  }

  if (Object.keys(cookies).length > 0) {
    return 'Cookie-based';
  }

  return 'Unknown';
}

/**
 * Prepare a concise HAR summary for LLM consumption.
 * Groups requests by endpoint pattern and includes sample responses.
 */
export function prepareHarSummaryForLLM(
  apiRequests: ParsedApiRequest[],
  maxRequests: number = 50,
): string {
  // Group by method + path pattern (remove IDs from paths)
  const grouped: Record<string, ParsedApiRequest[]> = {};
  for (const req of apiRequests) {
    const normalizedPath = req.path
      .replace(/\/\d+/g, '/{id}')
      .replace(/\/[a-f0-9-]{36}/g, '/{uuid}')
      .split('?')[0]; // Remove query string for grouping
    const key = `${req.method} ${req.domain}${normalizedPath}`;
    if (!grouped[key]) grouped[key] = [];
    grouped[key].push(req);
  }

  const lines: string[] = [];
  let count = 0;

  // Sort by request count (most frequent first)
  const sorted = Object.entries(grouped).sort((a, b) => b[1].length - a[1].length);

  for (const [pattern, requests] of sorted) {
    if (count >= maxRequests) break;

    const sample = requests[0];
    lines.push(`\n### ${pattern} (${requests.length} calls)`);
    lines.push(`Status: ${sample.status}`);
    lines.push(`Content-Type: ${sample.responseContentType ?? 'unknown'}`);

    // Show auth-relevant request headers (non-standard headers that may be needed for API calls)
    if (sample.requestHeaders) {
      const authHdrs: string[] = [];
      for (const [k, v] of Object.entries(sample.requestHeaders)) {
        const kl = k.toLowerCase();
        if (kl === 'authorization') authHdrs.push(`${k}: ${v.slice(0, 40)}...`);
        else if (isAuthHeader(kl) || isCustomApiHeader(kl)) authHdrs.push(`${k}: ${v.slice(0, 60)}`);
      }
      if (authHdrs.length > 0) {
        lines.push(`Auth headers: ${authHdrs.join(', ')}`);
      }
    }

    if (sample.queryParams) {
      lines.push(`Query params: ${JSON.stringify(sample.queryParams)}`);
    }

    if (sample.requestBody) {
      const body = sample.requestBody.length > 1000
        ? sample.requestBody.slice(0, 1000) + '...'
        : sample.requestBody;
      lines.push(`Request body: ${body}`);
    }

    if (sample.responseBody) {
      const body = sample.responseBody.length > 2000
        ? sample.responseBody.slice(0, 2000) + '...'
        : sample.responseBody;
      lines.push(`Response sample:\n\`\`\`json\n${body}\n\`\`\``);
    }

    count++;
  }

  return lines.join('\n');
}
