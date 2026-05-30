// worker.js — Pyodide Web Worker for the FastAPI demo. It pulls in the shared
// ASGI bridge harness (bridge-python.js) and shared runtime (worker-runtime.js),
// and supplies the inline FastAPI app + glue.
//
// The FastAPI app source is embedded inline below (a String.raw block). The
// pure-Python unit tests extract that exact block (and the bridge) and exercise
// them directly, so the tested code and the shipped code cannot drift apart.

// Pyodide and all wheels are served locally from ./vendor (see vendor.py). This
// keeps the demo self-contained and works in sandboxes where the browser itself
// has no outbound network. Resolved relative to this worker's URL.
const PYODIDE_URL = new URL("vendor/", self.location.href).href;

// The ASGI bridge harness is shared with the Datasette worker. Importing it here
// defines ASGI_BRIDGE_PY (a String.raw block of pure Python).
importScripts("bridge-python.js");

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
// Boot: install FastAPI, run lifespan, then serve requests (shared runtime).
// ---------------------------------------------------------------------------
importScripts("worker-runtime.js");

startAsgiWorker({
  pyodideUrl: PYODIDE_URL,
  installManifest: "install.json",
  installingMessage: "installing-fastapi",
  pythonSources: [ASGI_BRIDGE_PY, APP_PY, GLUE_PY],
  setupExpr: "await bridge.startup()",
});
