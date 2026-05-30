// bootstrap.js — runs in the shell page (top frame). It:
//   1. registers the service worker,
//   2. starts the long-lived Pyodide Web Worker,
//   3. waits until Pyodide + FastAPI + ASGI lifespan startup are ready,
//   4. brokers every captured request from the service worker to the worker,
//   5. points the <iframe> at /app/ so the captured app takes over.
//
// The iframe keeps the Pyodide worker alive across in-app navigations, and the
// shell (a long-lived window client) is what the service worker reaches for on
// each request — so the bridge keeps working even when the SW is restarted.

const statusEl = document.getElementById("status");
const frame = document.getElementById("appframe");

const APP_PREFIX = new URL("app", location.href).pathname.replace(/\/$/, "");
// Opt-in (set by datasette.html): mirror the iframe's location into the parent
// URL bar as a #fragment, and restore it on load. Lets the captured app behave
// like a normal full-page app with shareable/bookmarkable URLs and a back button.
const HASH_ROUTING = !!self.ASGI_HASH_ROUTING;

function setStatus(state) {
  statusEl.textContent = state;
  statusEl.dataset.state = state;
  console.log("[shell]", state);
}

// The path to show in the iframe on boot: from the URL #fragment if present.
function initialAppPath() {
  if (HASH_ROUTING && location.hash.length > 1) {
    return APP_PREFIX + location.hash.slice(1);
  }
  return APP_PREFIX + "/";
}

function setupHashSync() {
  let syncing = false;

  // iframe navigated (link, form redirect, hashchange-driven reload) -> update
  // the parent URL bar to reflect the page inside Datasette.
  frame.addEventListener("load", () => {
    let path;
    try {
      const loc = frame.contentWindow.location;
      path = loc.pathname + loc.search;
    } catch (e) {
      return; // shouldn't happen (same origin)
    }
    if (!path.startsWith(APP_PREFIX)) return;
    const newHash = "#" + (path.slice(APP_PREFIX.length) || "/");
    if (location.hash !== newHash) {
      syncing = true; // suppress the hashchange we are about to cause
      location.hash = newHash;
    }
  });

  // Parent URL #fragment changed by the user (back/forward, edited URL) -> point
  // the iframe at the corresponding app path.
  window.addEventListener("hashchange", () => {
    if (syncing) {
      syncing = false;
      return;
    }
    const target = APP_PREFIX + (location.hash.slice(1) || "/");
    try {
      const loc = frame.contentWindow.location;
      if (loc.pathname + loc.search === target) return;
    } catch (e) { /* fall through */ }
    frame.src = target;
  });
}

// --- talk to the Pyodide worker over a private MessageChannel ----------------
let workerPort = null;
const workerPending = new Map();

function callWorker(req) {
  const id =
    (self.crypto && self.crypto.randomUUID && self.crypto.randomUUID()) ||
    String(Date.now()) + "-" + Math.random();
  return new Promise((resolve) => {
    workerPending.set(id, resolve);
    workerPort.postMessage(
      { type: "request", id, method: req.method, url: req.url, headers: req.headers, body: req.body },
      req.body ? [req.body] : [],
    );
  });
}

// --- broker requests coming from the service worker -------------------------
navigator.serviceWorker.addEventListener("message", (event) => {
  const data = event.data;
  if (!data || data.type !== "asgi-request") return;
  const replyPort = event.ports[0];
  callWorker(data.request).then((response) => {
    replyPort.postMessage(
      { status: response.status, headers: response.headers, body: response.body },
      response.body ? [response.body] : [],
    );
  });
});

async function main() {
  if (!("serviceWorker" in navigator)) {
    setStatus("error: no service worker support");
    return;
  }

  setStatus("registering-sw");
  await navigator.serviceWorker.register("sw.js");
  await navigator.serviceWorker.ready;

  setStatus("starting-worker");
  // Which Pyodide worker to run (FastAPI demo by default, Datasette if set).
  const worker = new Worker(self.ASGI_WORKER || "worker.js");

  const ready = new Promise((resolve, reject) => {
    worker.onmessage = (event) => {
      const data = event.data || {};
      if (data.type === "ready") resolve();
      else if (data.type === "status") setStatus(data.message);
      else if (data.type === "log") console.log("[worker]", data.message);
      else if (data.type === "error") {
        setStatus("error: " + data.message);
        reject(new Error(data.message));
      }
    };
  });

  // Private channel between the shell and the Pyodide worker.
  const channel = new MessageChannel();
  workerPort = channel.port1;
  workerPort.onmessage = (event) => {
    const msg = event.data;
    if (msg && msg.type === "response") {
      const resolve = workerPending.get(msg.id);
      if (resolve) {
        workerPending.delete(msg.id);
        resolve(msg);
      }
    }
  };
  worker.postMessage({ type: "init", port: channel.port2 }, [channel.port2]);

  await ready;
  setStatus("ready");
  if (HASH_ROUTING) setupHashSync();
  frame.src = initialAppPath();
}

main().catch((err) => setStatus("error: " + (err && err.message ? err.message : err)));
