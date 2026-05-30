// worker.js — a dedicated Web Worker that runs Pyodide, installs FastAPI at runtime
// via micropip, and serves an ASGI application over a MessageChannel port that is
// shared with the service worker.
//
// The Python source for the ASGI bridge harness and the demo FastAPI app is embedded
// inline below (String.raw template literals). The pure-Python unit tests extract these
// exact blocks from this file and exercise them directly, so the tested code and the
// shipped code cannot drift apart.

// Pyodide and all wheels are served locally from ./vendor (see vendor.py). This
// keeps the demo self-contained and works in sandboxes where the browser itself
// has no outbound network. Resolved relative to this worker's URL.
const PYODIDE_URL = new URL("vendor/", self.location.href).href;

// ---------------------------------------------------------------------------
// Embedded Python: the ASGI bridge harness (pure Python, no Pyodide imports).
// Constraint: must contain no backticks and no "${" so it survives String.raw.
// ---------------------------------------------------------------------------
// PYTHON-BEGIN asgi_bridge
const ASGI_BRIDGE_PY = String.raw`
import asyncio


class ASGIBridge:
    """Drives an ASGI app: runs the lifespan protocol once and handles HTTP
    requests one message-cycle at a time, returning a plain dict response."""

    def __init__(self, app, root_path=""):
        self.app = app
        self.root_path = root_path
        self._lifespan_task = None
        self._startup_complete = None
        self._shutdown_complete = None
        self._recv_queue = None

    async def startup(self):
        loop = asyncio.get_event_loop()
        self._recv_queue = asyncio.Queue()
        self._startup_complete = loop.create_future()
        self._shutdown_complete = loop.create_future()
        scope = {"type": "lifespan", "asgi": {"version": "3.0", "spec_version": "2.0"}}
        await self._recv_queue.put({"type": "lifespan.startup"})

        async def receive():
            return await self._recv_queue.get()

        async def send(message):
            t = message["type"]
            if t == "lifespan.startup.complete":
                if not self._startup_complete.done():
                    self._startup_complete.set_result(True)
            elif t == "lifespan.startup.failed":
                if not self._startup_complete.done():
                    self._startup_complete.set_exception(
                        RuntimeError(message.get("message", "lifespan startup failed"))
                    )
            elif t == "lifespan.shutdown.complete":
                if not self._shutdown_complete.done():
                    self._shutdown_complete.set_result(True)

        async def run():
            try:
                await self.app(scope, receive, send)
            except Exception as exc:
                if not self._startup_complete.done():
                    self._startup_complete.set_exception(exc)
                if not self._shutdown_complete.done():
                    self._shutdown_complete.set_exception(exc)

        self._lifespan_task = asyncio.ensure_future(run())
        await self._startup_complete

    async def shutdown(self):
        if self._recv_queue is None:
            return
        await self._recv_queue.put({"type": "lifespan.shutdown"})
        await self._shutdown_complete

    async def handle(self, method, path, query_string, headers, body=b"",
                     scheme="http", host="localhost", port=80):
        if isinstance(query_string, str):
            query_string = query_string.encode("latin-1")
        if isinstance(body, str):
            body = body.encode("utf-8")
        if body is None:
            body = b""

        raw_headers = []
        have_host = False
        for name, value in headers:
            name_bytes = name.lower().encode("latin-1")
            if name_bytes == b"host":
                have_host = True
            raw_headers.append((name_bytes, value.encode("latin-1")))
        if not have_host:
            # A service worker cannot forward the browser-managed Host header, so
            # synthesize one. The non-default port MUST be included, otherwise
            # url_for() / redirects emit port-less URLs that fail in the browser.
            host_header = host
            if int(port) not in (80, 443):
                host_header = host + ":" + str(int(port))
            raw_headers.append((b"host", host_header.encode("latin-1")))

        scope = {
            "type": "http",
            "asgi": {"version": "3.0", "spec_version": "2.3"},
            "http_version": "1.1",
            "method": method.upper(),
            "scheme": scheme,
            "path": path,
            "raw_path": path.encode("utf-8"),
            "query_string": query_string,
            "root_path": self.root_path,
            "headers": raw_headers,
            "server": (host, int(port)),
            "client": ("127.0.0.1", 0),
        }

        request_messages = [{"type": "http.request", "body": body, "more_body": False}]

        async def receive():
            if request_messages:
                return request_messages.pop(0)
            return {"type": "http.disconnect"}

        response = {"status": 500, "headers": [], "body": bytearray()}

        async def send(message):
            t = message["type"]
            if t == "http.response.start":
                response["status"] = message["status"]
                response["headers"] = [
                    [k.decode("latin-1"), v.decode("latin-1")]
                    for k, v in message.get("headers", [])
                ]
            elif t == "http.response.body":
                chunk = message.get("body", b"") or b""
                response["body"].extend(chunk)

        await self.app(scope, receive, send)
        return {
            "status": response["status"],
            "headers": response["headers"],
            "body": bytes(response["body"]),
        }
`;
// PYTHON-END asgi_bridge

// ---------------------------------------------------------------------------
// Embedded Python: the demo FastAPI application.
// Constraint: no backticks, no "${" (the inline page JS therefore uses string
// concatenation and double quotes rather than template literals).
// ---------------------------------------------------------------------------
// PYTHON-BEGIN app
const APP_PY = String.raw`
from contextlib import asynccontextmanager

from fastapi import FastAPI, Form, Request
from fastapi.responses import HTMLResponse, RedirectResponse
from starlette.status import HTTP_303_SEE_OTHER

ITEMS = []


@asynccontextmanager
async def lifespan(app):
    # Runs when the bridge drives the ASGI lifespan startup inside Pyodide.
    ITEMS.clear()
    ITEMS.extend([
        {"id": 1, "name": "Widget"},
        {"id": 2, "name": "Gadget"},
        {"id": 3, "name": "Sprocket"},
    ])
    yield
    # (shutdown would run here)


app = FastAPI(lifespan=lifespan)


def render(title, body):
    html = (
        "<!doctype html><html><head><meta charset=utf-8>"
        "<title>" + title + "</title>"
        "<style>body{font-family:system-ui,sans-serif;margin:2rem;max-width:40rem}"
        ".item{font-weight:bold}</style></head><body>"
        "<h1>" + title + "</h1>" + body +
        "</body></html>"
    )
    return HTMLResponse(html)


@app.get("/", response_class=HTMLResponse)
async def home(request: Request):
    body = (
        "<p id=home-marker>This page is served by FastAPI running in Pyodide, "
        "via a service worker speaking ASGI.</p>"
        "<ul>"
        "<li><a id=nav-about href='" + str(request.url_for("about")) + "'>About (link navigation)</a></li>"
        "<li><a id=nav-widget href='" + str(request.url_for("widget")) + "'>Live widget (page runs its own JS)</a></li>"
        "</ul>"
        "<form id=greet-form method=post action='" + str(request.url_for("submit")) + "'>"
        "<input name=name id=name-input value=Simon>"
        "<button type=submit id=greet-btn>Greet</button>"
        "</form>"
    )
    return render("Home", body)


@app.get("/about", response_class=HTMLResponse)
async def about(request: Request):
    body = (
        "<p id=about-marker>This is the about page, fetched by following a link.</p>"
        "<a id=back-home href='" + str(request.url_for("home")) + "'>Back home</a>"
    )
    return render("About", body)


@app.post("/submit")
async def submit(request: Request, name: str = Form(...)):
    target = request.url_for("thanks").include_query_params(name=name)
    return RedirectResponse(url=target, status_code=HTTP_303_SEE_OTHER)


@app.get("/thanks", response_class=HTMLResponse)
async def thanks(request: Request, name: str = ""):
    body = "<p id=thanks-marker>Thanks, " + name + "!</p>"
    return render("Thanks", body)


@app.get("/widget", response_class=HTMLResponse)
async def widget(request: Request):
    api_url = str(request.url_for("api_items"))
    script = (
        "<p>Items loaded by this page's own inline JavaScript via fetch():</p>"
        "<ul id=items></ul>"
        "<script>"
        "fetch(\"" + api_url + "\").then(function(r){return r.json();}).then(function(d){"
        "var ul=document.getElementById(\"items\");"
        "d.items.forEach(function(it){"
        "var li=document.createElement(\"li\");"
        "li.textContent=it.name;li.className=\"item\";ul.appendChild(li);});"
        "document.body.setAttribute(\"data-items-loaded\",String(d.items.length));"
        "});"
        "</script>"
    )
    return render("Widget", script)


@app.get("/api/items")
async def api_items():
    return {"items": ITEMS}
`;
// PYTHON-END app

// ---------------------------------------------------------------------------
// Pyodide-only glue: wires the bridge to JS, converting at the boundary.
// (Not unit-tested in pure Python because it imports the Pyodide `js` module.)
// ---------------------------------------------------------------------------
const GLUE_PY = String.raw`
import json
from js import Object
from pyodide.ffi import to_js

bridge = ASGIBridge(app, root_path="/app")


async def handle_request(method, path, query, headers_json, body_buf, scheme, host, port):
    headers = json.loads(headers_json)
    if body_buf is None:
        body = b""
    else:
        body = body_buf.to_py().tobytes()
    resp = await bridge.handle(
        method, path, query, headers, body,
        scheme=scheme, host=host, port=int(port),
    )
    return to_js(
        {"status": resp["status"], "headers": resp["headers"], "body": resp["body"]},
        dict_converter=Object.fromEntries,
    )
`;

// ---------------------------------------------------------------------------
// JS side: load Pyodide, install FastAPI, run lifespan, then serve requests.
// ---------------------------------------------------------------------------
let bridgePort = null;
let handle_request = null;

function log(message) {
  self.postMessage({ type: "log", message: String(message) });
}

const initPromise = (async () => {
  importScripts(PYODIDE_URL + "pyodide.js");
  self.postMessage({ type: "status", message: "loading-pyodide" });
  const pyodide = await loadPyodide({ indexURL: PYODIDE_URL });

  self.postMessage({ type: "status", message: "installing-fastapi" });
  await pyodide.loadPackage("micropip");
  const micropip = pyodide.pyimport("micropip");
  // Install the non-bundled pure-Python wheels from the local vendor dir; their
  // dependencies (pydantic, anyio, ...) resolve from the local Pyodide lock.
  const install = await (await fetch(PYODIDE_URL + "install.json")).json();
  const wheelUrls = install.wheels.map((name) => PYODIDE_URL + name);
  await micropip.install(wheelUrls);

  self.postMessage({ type: "status", message: "starting-app" });
  await pyodide.runPythonAsync(ASGI_BRIDGE_PY);
  await pyodide.runPythonAsync(APP_PY);
  await pyodide.runPythonAsync(GLUE_PY);
  handle_request = pyodide.globals.get("handle_request");

  // Run the ASGI lifespan startup so FastAPI startup events fire.
  await pyodide.runPythonAsync("await bridge.startup()");
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

    const result = await handle_request(
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
