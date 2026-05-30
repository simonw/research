// worker-datasette.js — Pyodide Web Worker that runs *Datasette* (a real ASGI
// app) through the exact same bridge + runtime as the FastAPI demo.
//
// Datasette is configured with base_url="/app/" so every link, static asset and
// API URL it generates is rooted under the service-worker-intercepted scope, and
// num_sql_threads=0 so SQLite runs inline on Pyodide's event loop (no threads).
//
// The Datasette setup is embedded inline (a String.raw block) and extracted by
// the pure-Python unit tests. Constraint: no backticks, no "${".

const PYODIDE_URL = new URL("vendor/", self.location.href).href;

// Shared ASGI bridge harness -> defines ASGI_BRIDGE_PY.
importScripts("bridge-python.js");

// PYTHON-BEGIN datasette
const DATASETTE_PY = String.raw`
from datasette.app import Datasette
from datasette import hookimpl
from datasette.plugins import pm

STATE = {"ds": None, "app": None}


class RootAuthPlugin:
    """Logs every request in as the root actor. The browser tab is single-user,
    so this is the in-Pyodide equivalent of running 'datasette --root' and then
    visiting the one-time auth-token URL."""

    __name__ = "RootAuthPlugin"

    @hookimpl
    def actor_from_request(self, datasette, request):
        return {"id": "root"}


def _ensure_root_plugin():
    # pm is process-global; register the plugin once.
    if pm.get_plugin("pyodide_root_auth") is None:
        pm.register(RootAuthPlugin(), name="pyodide_root_auth")


def datasette_base_url_fixes(app, base_url):
    # Cheap workarounds for two Datasette base_url bugs (to be fixed upstream),
    # applied to HTML response bodies:
    #   1. base.html hardcodes the navigation-search ("Jump to") endpoint as
    #      url="/-/jump" without the base_url prefix, so its client-side fetch()
    #      escapes the service worker's /app/ scope.
    #   2. The table/row/query "export" links double-apply base_url, because
    #      urls.path(path_with_format(request=request, ...)) is given a path that
    #      already includes base_url -> /app/app/... -> 404 ("Database not found").
    # (Both assume no database is literally named "app".)
    prefix = base_url.rstrip("/").encode("latin-1")  # b"/app"
    replacements = [
        (b'"/-/jump"', b'"' + prefix + b'/-/jump"'),   # fix 1
        (prefix + prefix + b"/", prefix + b"/"),        # fix 2: /app/app/ -> /app/
    ]

    async def wrapped(scope, receive, send):
        if scope["type"] != "http":
            await app(scope, receive, send)
            return
        state = {"html": False}

        async def send_wrapper(message):
            if message["type"] == "http.response.start":
                headers = message.get("headers", [])
                for key, value in headers:
                    if key.lower() == b"content-type" and b"text/html" in value.lower():
                        state["html"] = True
                if state["html"]:
                    # Body length changes, so drop content-length and let the
                    # bridge/Response report the real length.
                    headers = [
                        (k, v) for k, v in headers if k.lower() != b"content-length"
                    ]
                    message = {**message, "headers": headers}
                await send(message)
            elif message["type"] == "http.response.body" and state["html"]:
                body = message.get("body", b"") or b""
                for find, repl in replacements:
                    if find in body:
                        body = body.replace(find, repl)
                await send({**message, "body": body})
            else:
                await send(message)

        await app(scope, receive, send_wrapper)

    return wrapped


async def build_app():
    _ensure_root_plugin()
    # base_url keeps every generated URL under /app/; num_sql_threads=0 runs
    # SQLite inline (Pyodide has no threads).
    ds = Datasette(
        memory=True,
        settings={"base_url": "/app/", "num_sql_threads": 0},
    )
    # Equivalent of the --root CLI flag: lets the root actor hold full
    # permissions (without it, root_enabled defaults to False and root is
    # denied), which unlocks the write/POST features in the UI.
    ds.root_enabled = True
    db = ds.add_memory_database("demo")
    await db.execute_write(
        "create table if not exists items "
        "(id integer primary key, name text, qty integer)"
    )
    existing = (await db.execute("select count(*) from items")).first()[0]
    if not existing:
        await db.execute_write(
            "insert into items (name, qty) values "
            "('Widget', 5), ('Gadget', 12), ('Sprocket', 7)"
        )
    STATE["ds"] = ds
    STATE["app"] = datasette_base_url_fixes(ds.app(), "/app/")
    return STATE["app"]
`;
// PYTHON-END datasette

// Pyodide-only glue (imports the js module, so not unit-tested in pure Python).
const GLUE_PY = String.raw`
import json
from js import Object
from pyodide.ffi import to_js

bridge = None


async def setup():
    global bridge
    app = await build_app()
    # Datasette's base_url handles the /app prefix, so root_path stays empty.
    bridge = ASGIBridge(app, root_path="")
    await bridge.startup()


async def handle_request(method, path, query, headers_json, body_buf, scheme, host, port):
    headers = json.loads(headers_json)
    body = b"" if body_buf is None else body_buf.to_py().tobytes()
    resp = await bridge.handle(
        method, path, query, headers, body,
        scheme=scheme, host=host, port=int(port),
    )
    return to_js(
        {"status": resp["status"], "headers": resp["headers"], "body": resp["body"]},
        dict_converter=Object.fromEntries,
    )
`;

// Shared runtime: loads Pyodide, installs wheels, runs setup, serves requests.
importScripts("worker-runtime.js");

startAsgiWorker({
  pyodideUrl: PYODIDE_URL,
  installManifest: "datasette.json",
  installingMessage: "installing-datasette",
  loadPackages: ["sqlite3"], // unvendored stdlib module Datasette needs
  pythonSources: [ASGI_BRIDGE_PY, DATASETTE_PY, GLUE_PY],
  setupExpr: "await setup()",
});
