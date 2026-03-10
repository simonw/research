/*global UVServiceWorker, __uv$config*/

importScripts("/uv/uv.bundle.js");
importScripts("/uv/uv.config.js");
importScripts(__uv$config.sw || "/uv/uv.sw.js");

const uv = new UVServiceWorker();

// Patterns to intercept for data extraction
const INTERCEPT_PATTERNS = [
  /\/api\/graphql/,
  /graphql\.instagram\.com/,
  /i\.instagram\.com\/api\/v1/,
  /\/api\/v1\/users/,
  /\/api\/v1\/feed/,
];

function shouldIntercept(url) {
  return INTERCEPT_PATTERNS.some((pattern) => pattern.test(url));
}

// Hook into UV's response pipeline
if (typeof uv.on === "function") {
  uv.on("response", async (event) => {
    try {
      const decodedUrl = event.url || "";
      if (!shouldIntercept(decodedUrl)) return;

      const cloned = event.response.clone();
      const contentType = cloned.headers.get("content-type") || "";
      if (!contentType.includes("json")) return;

      const payload = await cloned.json();
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
      // Silently ignore parse errors for non-JSON responses
    }
  });
}

async function handleRequest(event) {
  if (uv.route(event)) {
    const response = await uv.fetch(event);

    // Fallback interception if uv.on is not available
    if (typeof uv.on !== "function") {
      try {
        // Try to extract the original URL from the request
        const reqUrl = event.request.url;
        if (shouldIntercept(reqUrl)) {
          const cloned = response.clone();
          const contentType = cloned.headers.get("content-type") || "";
          if (contentType.includes("json")) {
            const payload = await cloned.json();
            const clients = await self.clients.matchAll({ type: "window" });
            for (const client of clients) {
              client.postMessage({
                type: "INTERCEPTED_DATA",
                url: reqUrl,
                timestamp: Date.now(),
                payload,
              });
            }
          }
        }
      } catch {
        // Ignore
      }
    }

    return response;
  }

  return await fetch(event.request);
}

self.addEventListener("fetch", (event) => {
  event.respondWith(handleRequest(event));
});
