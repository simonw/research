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

// --- Puppet agent injection for HTML responses ---
// Inline the puppet-agent code to avoid URL rewriting by UV's client-side hooks.
// This code is loaded once at SW init by fetching /puppet-agent.js from origin.
let PUPPET_SCRIPT_TAG = '';
(async () => {
  try {
    const resp = await fetch('/puppet-agent.js');
    const code = await resp.text();
    PUPPET_SCRIPT_TAG = '<script>' + code + '<\/script>';
  } catch (e) {
    console.error('[sw] Failed to load puppet-agent.js:', e);
  }
})();

async function injectPuppetAgent(response, destination) {
  // Only inject into document/iframe HTML responses
  if (!["document", "iframe"].includes(destination)) return response;

  const contentType = response.headers.get("content-type") || "";
  if (!contentType.includes("text/html")) return response;

  try {
    const html = await response.text();
    // Insert puppet-agent script just before </head> so it runs in the proxied page context
    // The /puppet-agent.js URL is on our origin (same as SW), not proxied
    let modified;
    const headClose = html.indexOf("</head>");
    if (headClose !== -1) {
      modified = html.slice(0, headClose) + PUPPET_SCRIPT_TAG + html.slice(headClose);
    } else {
      // Fallback: prepend to body
      const bodyOpen = html.indexOf("<body");
      if (bodyOpen !== -1) {
        modified = html.slice(0, bodyOpen) + PUPPET_SCRIPT_TAG + html.slice(bodyOpen);
      } else {
        modified = PUPPET_SCRIPT_TAG + html;
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

// --- Fetch handler with interception ---
async function handleRequest(event) {
  if (uv.route(event)) {
    let response = await uv.fetch(event);

    // Decode the original URL from the proxied request
    const decodedUrl = decodeProxiedUrl(event.request.url);

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

    // Inject puppet-agent into HTML responses
    response = await injectPuppetAgent(response, event.request.destination);

    return response;
  }

  return await fetch(event.request);
}

self.addEventListener("fetch", (event) => {
  event.respondWith(handleRequest(event));
});
