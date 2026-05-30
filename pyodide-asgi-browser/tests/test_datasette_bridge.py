"""Pure-Python unit tests for running Datasette through the ASGI bridge.

Extracts the shared bridge harness (bridge-python.js) and the Datasette setup
embedded in worker-datasette.js, and drives Datasette directly — no Pyodide /
browser. Verifies lifespan startup, base_url prefixing, and that real data is
served as HTML and JSON.
"""

import asyncio
import json
import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent.parent
BRIDGE_JS = HERE / "bridge-python.js"
DATASETTE_JS = HERE / "worker-datasette.js"


def extract_python(const_name, js_file):
    src = js_file.read_text()
    m = re.search(r"const " + re.escape(const_name) + r" = String\.raw`(.*?)`;", src, re.S)
    assert m, f"could not find {const_name} in {js_file.name}"
    return m.group(1)


def load():
    ns = {}
    exec(extract_python("ASGI_BRIDGE_PY", BRIDGE_JS), ns)
    exec(extract_python("DATASETTE_PY", DATASETTE_JS), ns)
    return ns


def serve(paths):
    """Boot Datasette via the bridge once and GET each path."""
    async def go():
        ns = load()
        app = await ns["build_app"]()
        bridge = ns["ASGIBridge"](app, root_path="")
        await bridge.startup()
        out = {}
        for path in paths:
            out[path] = await bridge.handle(
                "GET", path, "", [("host", "127.0.0.1:8000")], b"",
                host="127.0.0.1", port=8000,
            )
        return out

    return asyncio.run(go())


def test_datasette_home_served_with_prefixed_links():
    res = serve(["/app/"])["/app/"]
    assert res["status"] == 200
    body = res["body"]
    assert b"demo" in body
    # base_url must keep generated links under the intercepted /app/ scope.
    assert b'href="/app/demo"' in body
    assert b"/app/-/static/" in body


def test_jump_url_is_prefixed_with_base_url():
    # Datasette hardcodes the navigation-search endpoint as "/-/jump" without the
    # base_url prefix; the worker rewrites it so the fetch stays inside /app/.
    res = serve(["/app/"])["/app/"]
    assert res["status"] == 200
    assert b'"/app/-/jump"' in res["body"]
    assert b'"/-/jump"' not in res["body"]


def test_datasette_table_page_shows_data():
    res = serve(["/app/demo/items"])["/app/demo/items"]
    assert res["status"] == 200
    assert b"Widget" in res["body"]


def test_logged_in_as_root():
    res = serve(["/app/-/actor.json"])["/app/-/actor.json"]
    assert res["status"] == 200
    assert json.loads(res["body"]) == {"actor": {"id": "root"}}


def test_post_insert_is_authorized_for_root():
    # With root_enabled + the root actor, the write API POST must be allowed
    # (and the new same-origin CSRF passes for a header-less request).
    async def go():
        ns = load()
        app = await ns["build_app"]()
        bridge = ns["ASGIBridge"](app, root_path="")
        await bridge.startup()
        payload = json.dumps({"rows": [{"name": "Cog", "qty": 99}]}).encode()
        post = await bridge.handle(
            "POST", "/app/demo/items/-/insert", "",
            [("host", "127.0.0.1:8000"),
             ("content-type", "application/json"),
             ("content-length", str(len(payload)))],
            payload, host="127.0.0.1", port=8000,
        )
        after = await bridge.handle(
            "GET", "/app/demo/items.json", "_shape=array",
            [("host", "127.0.0.1:8000")], b"", host="127.0.0.1", port=8000,
        )
        return post, after

    post, after = asyncio.run(go())
    assert post["status"] == 201, post["body"]
    names = sorted(r["name"] for r in json.loads(after["body"]))
    assert "Cog" in names


def test_datasette_query_string_is_passed_through():
    async def go():
        ns = load()
        app = await ns["build_app"]()
        bridge = ns["ASGIBridge"](app, root_path="")
        await bridge.startup()
        return await bridge.handle(
            "GET", "/app/demo/items.json", "_shape=array",
            [("host", "127.0.0.1:8000")], b"", host="127.0.0.1", port=8000,
        )

    res = asyncio.run(go())
    assert res["status"] == 200
    rows = json.loads(res["body"])
    names = sorted(r["name"] for r in rows)
    assert names == ["Gadget", "Sprocket", "Widget"]
