/*global UVServiceWorker, __uv$config*/

importScripts("/uv/uv.bundle.js");
importScripts("/uv/uv.config.js");
importScripts(__uv$config.sw || "/uv/uv.sw.js");

const uv = new UVServiceWorker();

// Rewrite Sec-Fetch-Site via UV's request event so target servers see correct origin relationship
uv.on("request", (event) => {
  const url = event.data.url;
  const referer = event.data.headers.referer;
  if (url && referer) {
    try {
      const target = new URL(url);
      const ref = new URL(referer);
      if (target.origin === ref.origin) {
        event.data.headers["sec-fetch-site"] = "same-origin";
      } else {
        const tParts = target.hostname.split(".");
        const rParts = ref.hostname.split(".");
        const tReg = tParts.length >= 2 ? tParts.slice(-2).join(".") : target.hostname;
        const rReg = rParts.length >= 2 ? rParts.slice(-2).join(".") : ref.hostname;
        event.data.headers["sec-fetch-site"] = tReg === rReg ? "same-site" : "cross-site";
      }
    } catch {}
  } else if (url) {
    event.data.headers["sec-fetch-site"] = "none";
  }
});

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

async function injectScripts(response, destination, decodedUrl) {
  // Only inject into document/iframe HTML responses
  if (!["document", "iframe"].includes(destination)) return response;

  const contentType = response.headers.get("content-type") || "";
  if (!contentType.includes("text/html")) return response;

  try {
    let html = await response.text();

    // Pass the decoded target origin so anti-detect.js can spoof location correctly
    // Use a distinct name to avoid colliding with UV's own __uv$location
    let originHint = '';
    if (decodedUrl) {
      try {
        const targetOrigin = new URL(decodedUrl).origin;
        originHint = '<script>window.__antiDetectTarget={origin:' + JSON.stringify(targetOrigin) + ',href:' + JSON.stringify(decodedUrl) + '};<\/script>';
      } catch {}
    }
    const injectedScripts = originHint + ANTI_DETECT_SCRIPT_TAG + PUPPET_SCRIPT_TAG;
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

// --- Captcha vendor bypass ---
const CAPTCHA_VENDORS = [
  {
    name: "turnstile",
    hosts: [
      "challenges.cloudflare.com",
      "turnstile.cloudflare.com",
    ],
    scriptPatterns: [
      /https:\/\/challenges\.cloudflare\.com\/turnstile\/v0\/api\.js[^"'\s>]*/g,
    ],
  },
  {
    name: "recaptcha",
    hosts: [
      "www.google.com",
      "www.recaptcha.net",
      "www.gstatic.com",
    ],
    scriptPatterns: [
      /https:\/\/www\.google\.com\/recaptcha\/api\.js[^"'\s>]*/g,
      /https:\/\/www\.google\.com\/recaptcha\/enterprise\.js[^"'\s>]*/g,
      /https:\/\/www\.recaptcha\.net\/recaptcha\/api\.js[^"'\s>]*/g,
      /https:\/\/www\.recaptcha\.net\/recaptcha\/enterprise\.js[^"'\s>]*/g,
    ],
  },
  {
    name: "hcaptcha",
    hosts: [
      "js.hcaptcha.com",
      "hcaptcha.com",
      "newassets.hcaptcha.com",
      "assets.hcaptcha.com",
      "imgs.hcaptcha.com",
    ],
    scriptPatterns: [
      /https:\/\/js\.hcaptcha\.com\/1\/api\.js[^"'\s>]*/g,
      /https:\/\/hcaptcha\.com\/1\/api\.js[^"'\s>]*/g,
    ],
  },
];

function hostMatches(hostname, candidates) {
  const host = hostname.toLowerCase();
  return candidates.some((candidate) => host === candidate || host.endsWith(`.${candidate}`));
}

function isCaptchaScript(decodedUrl) {
  if (!decodedUrl) return false;
  try {
    const url = new URL(decodedUrl);
    return CAPTCHA_VENDORS.some(v => hostMatches(url.hostname, v.hosts));
  } catch {
    return false;
  }
}

async function sandboxCaptchaScript(response, decodedUrl) {
  const contentType = response.headers.get("content-type") || "";
  // Wait for JS files to be parsed
  if (!contentType.includes("javascript") && !contentType.includes("application/x-javascript") && !decodedUrl.endsWith(".js")) {
    return response;
  }

  try {
    let js = await response.text();
    
    // Protect against the ancestorOrigins DOM read
    js = js.replace(/\.ancestorOrigins/g, ".fakeAncestorOrigins");
    
    // Protect against strictly checking if it's in an iframe
    js = js.replace(/window\.top\s*!==\s*window(?:\.self)?/g, "false");
    js = js.replace(/window\.top\s*!=\s*window(?:\.self)?/g, "false");
    js = js.replace(/window\.top\s*===\s*window(?:\.self)?/g, "true");
    js = js.replace(/window\.top\s*==\s*window(?:\.self)?/g, "true");

    return new Response(js, {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    });
  } catch (e) {
    return response;
  }
}

// --- Fetch handler with interception ---
async function handleRequest(event) {
  if (uv.route(event)) {
    const decodedUrl = decodeProxiedUrl(event.request.url);

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

      // Captcha script sandboxing
      if (isCaptchaScript(decodedUrl)) {
         response = await sandboxCaptchaScript(response, decodedUrl);
      }

      // Universal ancestorOrigins rewrite for all non-captcha JS responses
      if (!isCaptchaScript(decodedUrl)) {
        const ct = response.headers.get("content-type") || "";
        if (ct.includes("javascript") || ct.includes("application/x-javascript") || decodedUrl.endsWith(".js")) {
          try {
            let js = await response.text();
            js = js.replace(/\.ancestorOrigins/g, ".fakeAncestorOrigins");
            response = new Response(js, {
              status: response.status,
              statusText: response.statusText,
              headers: response.headers,
            });
          } catch {}
        }
      }
    }

    // Inject anti-detect + puppet-agent into HTML responses
    response = await injectScripts(response, event.request.destination, decodedUrl);

    return response;
  }

  return await fetch(event.request);
}

self.addEventListener("fetch", (event) => {
  event.respondWith(handleRequest(event));
});
