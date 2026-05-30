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
    STATE["app"] = ds.app()
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
