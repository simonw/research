/*global UVServiceWorker, __uv$config*/

importScripts("/uv/uv.bundle.js");
importScripts("/uv/uv.config.js");
importScripts(__uv$config.sw || "/uv/uv.sw.js");

const uv = new UVServiceWorker();

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

// --- Fetch handler with interception ---
async function handleRequest(event) {
  if (uv.route(event)) {
    const response = await uv.fetch(event);

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

    return response;
  }

  return await fetch(event.request);
}

self.addEventListener("fetch", (event) => {
  event.respondWith(handleRequest(event));
});
