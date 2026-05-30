// worker-runtime.js — shared Pyodide ASGI worker runtime.
//
// Both worker.js (FastAPI) and worker-datasette.js (Datasette) call
// startAsgiWorker(config) with their own install manifest and Python sources.
// This runtime loads Pyodide, installs the vendored wheels, execs the Python,
// runs the app's setup/lifespan, and then serves requests posted by the shell
// over the MessageChannel port (each handled via the Python `handle_request`).
//
// config = {
//   pyodideUrl,                 // base URL of the vendored Pyodide + wheels
//   installManifest,            // JSON file (relative to pyodideUrl) listing wheels
//   pythonSources: [str, ...],  // Python blocks to exec (bridge, app, glue)
//   setupExpr,                  // Python expression to await (e.g. "await setup()")
//   installingMessage,          // status string while installing
// }
function startAsgiWorker(config) {
  let bridgePort = null;
  let handleRequest = null;

  const initPromise = (async () => {
    importScripts(config.pyodideUrl + "pyodide.js");
    self.postMessage({ type: "status", message: "loading-pyodide" });
    const pyodide = await loadPyodide({ indexURL: config.pyodideUrl });

    self.postMessage({ type: "status", message: config.installingMessage || "installing-packages" });
    // Pyodide packages to load first (e.g. sqlite3, which Datasette imports and
    // which is unvendored from the stdlib in Pyodide).
    await pyodide.loadPackage(["micropip", ...(config.loadPackages || [])]);
    const micropip = pyodide.pyimport("micropip");
    // Install the non-bundled pure-Python wheels from the local vendor dir; their
    // bundled dependencies resolve from the local Pyodide lock.
    const install = await (await fetch(config.pyodideUrl + config.installManifest)).json();
    await micropip.install(install.wheels.map((name) => config.pyodideUrl + name));

    self.postMessage({ type: "status", message: "starting-app" });
    for (const src of config.pythonSources) {
      await pyodide.runPythonAsync(src);
    }
    handleRequest = pyodide.globals.get("handle_request");

    // Run the app's setup / ASGI lifespan startup.
    await pyodide.runPythonAsync(config.setupExpr);
    self.postMessage({ type: "ready" });
  })().catch((err) => {
    self.postMessage({ type: "error", message: String((err && err.message) || err) });
    throw err;
  });

  async function onBridgeMessage(event) {
    const msg = event.data;
    if (!msg || msg.type !== "request") return;
    try {
      await initPromise;
      const url = new URL(msg.url);
      const headersJson = JSON.stringify(msg.headers || []);
      const bodyArr = msg.body ? new Uint8Array(msg.body) : new Uint8Array();
      const port = url.port
        ? parseInt(url.port, 10)
        : (url.protocol === "https:" ? 443 : 80);

      const result = await handleRequest(
        msg.method,
        url.pathname,
        url.search.replace(/^\?/, ""),
        headersJson,
        bodyArr,
        url.protocol.replace(":", ""),
        url.hostname,
        port,
      );

      // Copy the body into a standalone buffer so we can transfer it without
      // touching Pyodide's WASM heap.
      const bodyCopy = result.body ? result.body.slice() : new Uint8Array();
      bridgePort.postMessage(
        { type: "response", id: msg.id, status: result.status, headers: result.headers, body: bodyCopy.buffer },
        [bodyCopy.buffer],
      );
    } catch (err) {
      const payload = new TextEncoder().encode(
        "ASGI bridge error: " + ((err && err.message) || err),
      );
      bridgePort.postMessage(
        { type: "response", id: msg.id, status: 500, headers: [["content-type", "text/plain; charset=utf-8"]], body: payload.buffer },
        [payload.buffer],
      );
    }
  }

  self.onmessage = (event) => {
    const data = event.data;
    if (data && data.type === "init") {
      bridgePort = data.port;
      bridgePort.onmessage = onBridgeMessage; // setting onmessage starts the port
    }
  };
}
