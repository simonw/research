/*global UVServiceWorker, __uv$config*/

importScripts("/uv/uv.bundle.js");
importScripts("/uv/uv.config.js");
importScripts(__uv$config.sw || "/uv/uv.sw.js");

const uv = new UVServiceWorker();

// Take control immediately so navigator.serviceWorker.controller is available
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

// --- Dynamic capture registry ---
// Map<key, { urlPattern: RegExp, bodyPattern?: RegExp, responses: object[] }>
const captures = new Map();
let interceptAllJson = false;

/**
 * XOR decode matching UV's codec: odd-indexed chars XOR'd with 2.
 */
function xorDecode(encoded) {
  const [path, ...queryParts] = encoded.split("?");
  const decoded = decodeURIComponent(path);
  let result = "";
  for (let i = 0; i < decoded.length; i++) {
    result += i % 2 ? String.fromCharCode(decoded.charCodeAt(i) ^ 2) : decoded[i];
  }
  return result + (queryParts.length ? "?" + queryParts.join("?") : "");
}

/**
 * Extract the original URL from a /service/... encoded request URL.
 */
function decodeProxiedUrl(requestUrl) {
  const prefix = "/service/";
  const idx = requestUrl.indexOf(prefix);
  if (idx === -1) return null;
  const encoded = requestUrl.slice(idx + prefix.length);
  try {
    return xorDecode(encoded);
  } catch {
    return null;
  }
}

/**
 * Check if a decoded URL matches any registered capture pattern.
 * Returns matching capture keys.
 */
function getMatchingCaptures(decodedUrl) {
  const matches = [];
  for (const [key, capture] of captures) {
    if (capture.urlPattern.test(decodedUrl)) {
      matches.push(key);
    }
  }
  return matches;
}

/**
 * Try to extract and store a response for matched captures.
 */
async function extractAndStore(decodedUrl, response, matchingKeys) {
  try {
    const contentType = response.headers.get("content-type") || "";
    if (!contentType.includes("json") && !contentType.includes("text")) return;

    let payload;
    try {
      payload = await response.json();
    } catch {
      return; // Not valid JSON
    }

    for (const key of matchingKeys) {
      const capture = captures.get(key);
      if (!capture) continue;

      // Optional body pattern filter
      if (capture.bodyPattern) {
        const bodyStr = JSON.stringify(payload);
        if (!capture.bodyPattern.test(bodyStr)) continue;
      }

      capture.responses.push({
        url: decodedUrl,
        timestamp: Date.now(),
        payload,
      });
    }

    // Also broadcast to all clients for the data panel
    const clients = await self.clients.matchAll({ type: "window" });
    for (const client of clients) {
      client.postMessage({
        type: "INTERCEPTED_DATA",
        url: decodedUrl,
        timestamp: Date.now(),
        payload,
      });
    }
  } catch {
    // Ignore extraction errors
  }
}

/**
 * Broadcast all JSON responses when interceptAllJson is enabled.
 */
async function broadcastJsonResponse(decodedUrl, response) {
  try {
    const contentType = response.headers.get("content-type") || "";
    if (!contentType.includes("json")) return;

    const payload = await response.json();
    const clients = await self.clients.matchAll({ type: "window" });
    for (const client of clients) {
      client.postMessage({
        type: "INTERCEPTED_DATA",
        url: decodedUrl,
        timestamp: Date.now(),
        payload,
      });
    }
  } catch {
    // Ignore
  }
}

// --- Message handler for capture registration/retrieval ---
self.addEventListener("message", (event) => {
  const data = event.data;
  if (!data) return;

  if (data.type === "CAPTURE_REGISTER") {
    captures.set(data.key, {
      urlPattern: new RegExp(data.urlPattern, "i"),
      bodyPattern: data.bodyPattern ? new RegExp(data.bodyPattern, "i") : null,
      responses: [],
    });
    // Respond to confirm registration
    event.source?.postMessage({
      type: "CAPTURE_REGISTERED",
      key: data.key,
    });
  }

  if (data.type === "CAPTURE_GET") {
    const capture = captures.get(data.key);
    event.source?.postMessage({
      type: "CAPTURE_RESULT",
      key: data.key,
      responses: capture ? capture.responses : [],
    });
  }

  if (data.type === "CAPTURE_CLEAR") {
    if (data.key) {
      captures.delete(data.key);
    } else {
      captures.clear();
    }
  }

  if (data.type === "SET_INTERCEPT_ALL_JSON") {
    interceptAllJson = !!data.enabled;
  }
});

// --- Anti-detect + puppet agent injection for HTML responses ---
// Inline both scripts to avoid URL rewriting by UV's client-side hooks.
// These are loaded once at SW init by fetching from origin.
let ANTI_DETECT_SCRIPT_TAG = '';
let PUPPET_SCRIPT_TAG = '';
(async () => {
  try {
    const [antiDetectResp, puppetResp] = await Promise.all([
      fetch('/anti-detect.js'),
      fetch('/puppet-agent.js'),
    ]);
    const antiDetectCode = await antiDetectResp.text();
    const puppetCode = await puppetResp.text();
    ANTI_DETECT_SCRIPT_TAG = '<script>' + antiDetectCode + '<\/script>';
    PUPPET_SCRIPT_TAG = '<script>' + puppetCode + '<\/script>';
  } catch (e) {
    console.error('[sw] Failed to load injected scripts:', e);
  }
})();

async function injectScripts(response, destination) {
  // Only inject into document/iframe HTML responses
  if (!["document", "iframe"].includes(destination)) return response;

  const contentType = response.headers.get("content-type") || "";
  if (!contentType.includes("text/html")) return response;

  try {
    let html = await response.text();

    // Fix Turnstile: inject direct (un-proxied) script tags for Turnstile API
    html = fixTurnstileInHtml(html);

    // Inject anti-detect first (runs before puppet-agent), then puppet-agent
    const injectedScripts = ANTI_DETECT_SCRIPT_TAG + PUPPET_SCRIPT_TAG;
    let modified;
    const headClose = html.indexOf("</head>");
    if (headClose !== -1) {
      modified = html.slice(0, headClose) + injectedScripts + html.slice(headClose);
    } else {
      const bodyOpen = html.indexOf("<body");
      if (bodyOpen !== -1) {
        modified = html.slice(0, bodyOpen) + injectedScripts + html.slice(bodyOpen);
      } else {
        modified = injectedScripts + html;
      }
    }

    return new Response(modified, {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    });
  } catch {
    return response;
  }
}

// --- Turnstile / Cloudflare challenge bypass ---
// Turnstile requires its scripts and iframes to load from the real origin.
// We bypass UV for challenges.cloudflare.com requests (script/iframe loads)
// and rewrite HTML to load the Turnstile API script directly (un-proxied).
const TURNSTILE_BYPASS_HOSTS = new Set([
  "challenges.cloudflare.com",
  "turnstile.cloudflare.com",
]);

// Turnstile API script pattern — needs to be loaded un-proxied in HTML
const TURNSTILE_SCRIPT_RE = /https:\/\/challenges\.cloudflare\.com\/turnstile\/v0\/api\.js[^"']*/g;

/**
 * Rewrite HTML to load Turnstile scripts directly (not through UV proxy).
 * UV would rewrite the script src to go through the proxy, breaking origin checks.
 * We inject the real script URL as a separate <script> tag and remove the proxied version.
 */
function fixTurnstileInHtml(html) {
  // Find all turnstile script URLs referenced in the HTML
  const turnstileUrls = html.match(TURNSTILE_SCRIPT_RE);
  if (!turnstileUrls) return html;

  // Add direct (un-proxied) Turnstile script tags before </head>
  const directScripts = [...new Set(turnstileUrls)]
    .map(url => `<script src="${url}" async defer></script>`)
    .join('');

  const headClose = html.indexOf('</head>');
  if (headClose !== -1) {
    html = html.slice(0, headClose) + directScripts + html.slice(headClose);
  }
  return html;
}

// --- Fetch handler with interception ---
async function handleRequest(event) {
  if (uv.route(event)) {
    const decodedUrl = decodeProxiedUrl(event.request.url);

    // Bypass UV for Turnstile/challenge iframe and script requests
    if (decodedUrl) {
      try {
        const targetHost = new URL(decodedUrl).hostname;
        if (TURNSTILE_BYPASS_HOSTS.has(targetHost)) {
          return await fetch(decodedUrl, { mode: "cors", credentials: "omit" });
        }
      } catch {
        // Invalid URL, continue with UV
      }
    }

    let response = await uv.fetch(event);

    if (decodedUrl) {
      // Check registered captures
      const matchingKeys = getMatchingCaptures(decodedUrl);
      if (matchingKeys.length > 0) {
        extractAndStore(decodedUrl, response.clone(), matchingKeys);
      }

      // Broadcast all JSON if enabled
      if (interceptAllJson) {
        broadcastJsonResponse(decodedUrl, response.clone());
      }
    }

    // Inject anti-detect + puppet-agent into HTML responses
    response = await injectScripts(response, event.request.destination);

    return response;
  }

  return await fetch(event.request);
}

self.addEventListener("fetch", (event) => {
  event.respondWith(handleRequest(event));
});
