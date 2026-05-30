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

function setStatus(state) {
  statusEl.textContent = state;
  statusEl.dataset.state = state;
  console.log("[shell]", state);
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
  const worker = new Worker("worker.js");

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
  frame.src = "/app/";
}

main().catch((err) => setStatus("error: " + (err && err.message ? err.message : err)));
