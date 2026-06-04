// sw.js — service worker that captures the server side of the app.
//
// Every same-origin request whose path starts with /app (link navigations, form
// submissions and fetch() calls) is intercepted and answered by the Python ASGI
// app running in the Pyodide Web Worker. Everything else (the shell, sw.js,
// worker.js, bootstrap.js, the local vendor/ assets) falls through to the network.
//
// Service workers are terminated when idle, so we must NOT cache a long-lived
// MessageChannel port in worker memory — it would be lost on restart and later
// requests would hang. Instead, for every request we locate the shell *window*
// client fresh via clients.matchAll() and hand it a one-shot reply port. The shell
// (which owns the long-lived Pyodide worker) brokers the request and replies. This
// survives any number of service-worker restarts.

const APP_PREFIX = new URL("app", self.location.href).pathname.replace(/\/$/, "");

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

function isAppRequest(rawUrl) {
  const url = new URL(rawUrl);
  if (url.origin !== self.location.origin) return false;
  return url.pathname === APP_PREFIX || url.pathname.startsWith(APP_PREFIX + "/");
}

// The shell is the window client that is NOT inside the captured /app scope.
async function findShellClient() {
  const windows = await self.clients.matchAll({
    type: "window",
    includeUncontrolled: true,
  });
  for (const client of windows) {
    const path = new URL(client.url).pathname;
    if (!(path === APP_PREFIX || path.startsWith(APP_PREFIX + "/"))) {
      return client;
    }
  }
  return windows[0] || null;
}

self.addEventListener("fetch", (event) => {
  if (!isAppRequest(event.request.url)) return; // network passthrough
  event.respondWith(handleAppRequest(event.request));
});

async function handleAppRequest(request) {
  const shell = await findShellClient();
  if (!shell) {
    return new Response("ASGI bridge host not available", {
      status: 503,
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
  }

  const hasBody = !["GET", "HEAD"].includes(request.method);
  const body = hasBody ? await request.arrayBuffer() : null;
  const headers = [...request.headers.entries()];

  const channel = new MessageChannel();
  const reply = new Promise((resolve) => {
    channel.port1.onmessage = (event) => resolve(event.data);
  });

  shell.postMessage(
    {
      type: "asgi-request",
      request: { method: request.method, url: request.url, headers, body },
    },
    [channel.port2],
  );

  const msg = await reply;

  const responseHeaders = new Headers();
  for (const [name, value] of msg.headers) {
    const lower = name.toLowerCase();
    // We host every app inside an iframe, so neutralise frame-busting headers
    // the app may send (e.g. Datasette's write pages set these). This is a
    // property of the bridge, not of any one app, so it lives here.
    if (lower === "x-frame-options") continue;
    if (lower === "content-security-policy") {
      const kept = value
        .split(";")
        .map((directive) => directive.trim())
        .filter((directive) => directive && !/^frame-ancestors\b/i.test(directive));
      if (kept.length) responseHeaders.append(name, kept.join("; "));
      continue;
    }
    responseHeaders.append(name, value);
  }
  const status = msg.status;
  // 204/205/304 must not carry a body.
  const responseBody =
    status === 204 || status === 205 || status === 304 ? null : msg.body;
  return new Response(responseBody, { status, headers: responseHeaders });
}
