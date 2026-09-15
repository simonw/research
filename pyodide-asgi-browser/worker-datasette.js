// worker-datasette.js — Pyodide Web Worker that runs *Datasette* (a real ASGI
// app) through the exact same bridge + runtime as the FastAPI demo.
//
// Datasette is configured with a base_url matching the published app prefix, so
// every link, static asset and API URL it generates is rooted under the
// service-worker-intercepted scope, and
// num_sql_threads=0 so SQLite runs inline on Pyodide's event loop (no threads).
//
// The Datasette setup is embedded inline (a String.raw block) and extracted by
// the pure-Python unit tests. Constraint: no backticks, no "${".

const PYODIDE_URL = new URL("vendor/", self.location.href).href;
const APP_BASE_URL = new URL("app/", self.location.href).pathname;

// Shared ASGI bridge harness -> defines ASGI_BRIDGE_PY.
importScripts("bridge-python.js", "datasette-startup.js");

const STARTUP_OPTIONS = parseDatasetteOptions(self.location.href);

// PYTHON-BEGIN datasette
const DATASETTE_PY = String.raw`
from datasette.app import Datasette
from datasette.database import Database
from datasette import hookimpl
from datasette.plugins import pm

STATE = {"ds": None, "app": None}
DATASETTE_BASE_URL = "/app/"


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
    # Workarounds for Datasette base_url bugs, applied to responses:
    #   1. base.html hardcodes the navigation-search ("Jump to") endpoint as
    #      url="/-/jump" without the base_url prefix, so its client-side fetch()
    #      escapes the service worker's app scope.
    #   2. The table/row/query "export" links double-apply base_url, because
    #      urls.path(path_with_format(request=request, ...)) is given a path that
    #      already includes base_url -> double-prefix -> 404 ("Database not found").
    #   3. The legacy /db?sql= redirect to /db/-/query omits base_url.
    prefix = base_url.rstrip("/").encode("latin-1")
    replacements = [
        (b'"/-/jump"', b'"' + prefix + b'/-/jump"'),   # fix 1
    ]

    async def wrapped(scope, receive, send):
        if scope["type"] != "http":
            await app(scope, receive, send)
            return
        state = {"html": False}
        # Only collapse the duplicated prefix of this page's export/sort links.
        # A blanket /app/app/ replacement breaks databases literally named app.
        path = scope["path"].encode("utf-8")
        page_replacements = replacements + [
            (prefix + path + suffix, path + suffix) for suffix in (b".", b"?")
        ]

        async def send_wrapper(message):
            if message["type"] == "http.response.start":
                headers = message.get("headers", [])
                headers = [
                    (key, prefix + value if key.lower() == b"location"
                     and value.startswith(b"/") and not value.startswith(b"//")
                     and value != prefix and not value.startswith(prefix + b"/") else value)
                    for key, value in headers
                ]
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
                for find, repl in page_replacements:
                    if find in body:
                        body = body.replace(find, repl)
                await send({**message, "body": body})
            else:
                await send(message)

        await app(scope, receive, send_wrapper)

    return wrapped


async def build_app(options=None, fetch_bytes=None, directory="."):
    options = options or {}
    files, metadata = await prepare_datasette(options, fetch_bytes, directory)
    config_kwargs = {}
    if options.get("config"):
        config = await load_datasette_document(options["config"], fetch_bytes, "config")
        if isinstance(config.get("settings"), dict):
            # Datasette versions can reject duplicate constructor/config settings.
            # These two are required by the bridge and are supplied below.
            config["settings"] = {key: value for key, value in config["settings"].items()
                                  if key not in ("base_url", "num_sql_threads")}
        config_kwargs["config"] = config
    _ensure_root_plugin()
    # base_url keeps every generated URL under the service-worker-intercepted
    # app prefix; num_sql_threads=0 runs SQLite inline (Pyodide has no threads).
    ds = Datasette(
        files=files,
        memory=True,
        metadata=metadata,
        settings={"base_url": DATASETTE_BASE_URL, "num_sql_threads": 0},
        **config_kwargs,
    )
    # Equivalent of the --root CLI flag: lets the root actor hold full
    # permissions (without it, root_enabled defaults to False and root is
    # denied), which unlocks the write/POST features in the UI.
    ds.root_enabled = True
    if not files:
        # add_database/Database also work on stable Datasette versions selected
        # with ?ref=; add_memory_database is only available in newer releases.
        db = ds.add_database(Database(ds, memory_name="demo"), name="demo")
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
    STATE["app"] = datasette_base_url_fixes(ds.app(), DATASETTE_BASE_URL)
    return STATE["app"]
`;
// PYTHON-END datasette

// Pyodide-only glue (imports the js module, so not unit-tested in pure Python).
const GLUE_PY = String.raw`
import json
from js import Object
from pyodide.ffi import to_js
from pyodide.http import pyfetch

bridge = None


async def fetch_bytes(url):
    response = await pyfetch(url)
    if not response.ok:
        raise ValueError("Could not load {}: HTTP {}".format(url, response.status))
    return await response.bytes()


async def setup():
    global bridge
    app = await build_app(DATASETTE_OPTIONS, fetch_bytes)
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
  installPackages: async (pyodide, wheelUrls) => {
    // Pass values as data, never interpolate URL values into Python source.
    pyodide.globals.set("_datasette_options_json", JSON.stringify(STARTUP_OPTIONS));
    pyodide.globals.set("_datasette_wheels_json", JSON.stringify(wheelUrls));
    await pyodide.runPythonAsync(DATASETTE_INSTALL_PY);
  },
  pythonSources: [
    ASGI_BRIDGE_PY,
    DATASETTE_STARTUP_PY,
    DATASETTE_PY,
    "DATASETTE_BASE_URL = " + JSON.stringify(APP_BASE_URL),
    GLUE_PY,
  ],
  setupExpr: "await setup()",
});
