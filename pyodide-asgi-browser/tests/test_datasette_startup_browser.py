"""Real Pyodide coverage of URL inputs, plugin wheels and version selection."""
import io
import sqlite3
import threading
import zipfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlencode, urlsplit

import pytest
from playwright.sync_api import expect, sync_playwright

from test_datasette_browser import BOOT_TIMEOUT_MS, app_frame, server


@pytest.fixture(scope="module")
def browser():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        yield browser
        browser.close()


@pytest.fixture
def inputs(tmp_path):
    database = tmp_path / "source.db"
    with sqlite3.connect(database) as db:
        db.execute("create table original (id integer primary key, name text)")
        db.execute("insert into original values (1, 'downloaded')")
    wheel = io.BytesIO()
    info = "asgi_demo_test_plugin-1.0.dist-info/"
    with zipfile.ZipFile(wheel, "w") as archive:
        archive.writestr("asgi_demo_test_plugin.py", "from datasette import hookimpl\n"
                         "@hookimpl\ndef prepare_connection(conn, datasette):\n"
                         "    message = datasette.plugin_config('asgi-demo-test-plugin')['message']\n"
                         "    conn.create_function('startup_test', 0, lambda: message)\n")
        archive.writestr(info + "WHEEL", "Wheel-Version: 1.0\nGenerator: tests\n"
                         "Root-Is-Purelib: true\nTag: py3-none-any\n")
        archive.writestr(info + "METADATA", "Metadata-Version: 2.1\n"
                         "Name: asgi-demo-test-plugin\nVersion: 1.0\n")
        archive.writestr(info + "entry_points.txt", "[datasette]\n"
                         "asgi_demo_test_plugin = asgi_demo_test_plugin\n")
        archive.writestr(info + "RECORD", "")
    resources = {
        "/a/source.db": ("application/octet-stream", database.read_bytes()),
        "/b/source.db": ("application/octet-stream", database.read_bytes()),
        "/a/items.csv": ("text/csv", b"id,name,qty\n2,CSV,5\n"),
        "/b/items.csv": ("text/csv", b"name;qty\nSemicolon;12\n"),
        "/a/items.json": ("application/json", b'[{"name":"JSON","qty":7}]'),
        "/first.sql": ("text/plain", b"create table items (id integer primary key, name text, qty integer);"),
        "/second.sql": ("text/plain", b"insert into items values (1, 'SQL seed', 3);"),
        "/metadata.yml": ("text/plain", b"title: Imported demo\n"),
        "/config.yml": ("text/plain", b"plugins:\n  asgi-demo-test-plugin:\n    message: plugin ready\n"),
        "/asgi_demo_test_plugin-1.0-py3-none-any.whl": ("application/octet-stream", wheel.getvalue()),
    }

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            item = resources.get(urlsplit(self.path).path)
            self.send_response(200 if item else 404)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Content-Type", item[0] if item else "text/plain")
            self.end_headers()
            self.wfile.write(item[1] if item else b"missing")

        def log_message(self, *args):
            pass

    httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    yield "http://127.0.0.1:{}".format(httpd.server_port)
    httpd.shutdown()
    thread.join()
    httpd.server_close()


def wait_for_boot(page):
    page.wait_for_function("""() => {
        const state = document.querySelector('#status').dataset.state || '';
        return state === 'ready' || state.startsWith('error');
    }""", timeout=BOOT_TIMEOUT_MS)
    return page.locator("#status").get_attribute("data-state")


def test_combined_inputs_metadata_plugin_and_fragment(browser, server, inputs):
    query = [(kind, inputs + path) for kind, path in [
        ("url", "/a/source.db"), ("url", "/b/source.db"),
        ("csv", "/a/items.csv"), ("csv", "/b/items.csv"),
        ("json", "/a/items.json"), ("sql", "/first.sql"), ("sql", "/second.sql"),
        ("metadata", "/metadata.yml"),
        ("config", "/config.yml"),
        ("install", "/asgi_demo_test_plugin-1.0-py3-none-any.whl"),
    ]]
    with browser.new_context() as context:
        page = context.new_page()
        page.goto(server + "/datasette.html?" + urlencode(query)
                  + "#/data?sql=select+startup_test()")
        assert wait_for_boot(page) == "ready"
        expect(page.frame_locator("#appframe").locator("body")).to_contain_text("plugin ready")
        frame = app_frame(page)
        result = frame.evaluate("""async () => {
            const rows = async path => (await fetch('/app/' + path + '?_shape=array')).json();
            return {
                first: await rows('source/original.json'),
                second: await rows('source_2/original.json'),
                csv: await rows('data/items.json'),
                semicolon: await rows('data/items_1.json'),
                json: await rows('data/items_2.json'),
                home: await (await fetch('/app/')).text(),
            };
        }""")
        assert result["first"] == result["second"] == [{"id": 1, "name": "downloaded"}]
        assert result["csv"] == [{"id": 1, "name": "SQL seed", "qty": 3},
                                 {"id": 2, "name": "CSV", "qty": 5}]
        assert result["semicolon"][0]["qty"] == 12
        assert result["json"][0]["name"] == "JSON"
        assert "Imported demo" in result["home"]
        assert 'href="/app/demo"' not in result["home"]
        assert urlsplit(page.url).query == urlencode(query)


def test_missing_data_url_shows_boot_error(browser, server, inputs):
    with browser.new_context() as context:
        page = context.new_page()
        page.goto(server + "/datasette.html?" + urlencode({"csv": inputs + "/missing.csv"}))
        assert "HTTP 404" in wait_for_boot(page)


@pytest.mark.parametrize("ref", ["1.0a31", "0.65.2"])
def test_ref_installs_requested_version(browser, server, ref):
    # Explicit refs intentionally use PyPI, unlike the fully vendored default.
    with browser.new_context() as context:
        page = context.new_page()
        page.goto(server + "/datasette.html?ref=" + ref)
        assert wait_for_boot(page) == "ready"
        expect(page.frame_locator("#appframe").locator("a[href='/app/demo']")).to_be_visible()
        version = app_frame(page).evaluate("""async () =>
            (await (await fetch('/app/-/versions.json')).json()).datasette.version
        """)
        assert version == ref


def test_linked_table_treatments_config(browser, server):
    config = ("https://gist.githubusercontent.com/simonw/6d6e346717d9aff43fac6c8644845975/"
              "raw/de71c6bca5ffbeff0b13c96a9620fbf72b037e6b/config.yml")
    assets = "https://static.simonwillison.net/static/2026/table-treatments/static/"
    with browser.new_context() as context:
        page = context.new_page()
        loaded = []
        page.on("response", lambda response: loaded.append((response.url, response.status)))
        page.goto(server + "/datasette.html?" + urlencode({"config": config}) + "#/demo/items")
        assert wait_for_boot(page) == "ready"
        frame = page.frame_locator("#appframe")
        expect(frame.locator("body")).to_contain_text("Widget")
        expect(frame.locator('link[href="' + assets + 'table-treatments.css"]')).to_be_attached()
        expect(frame.locator('script[src="' + assets + 'table-treatments.js"]')).to_be_attached()
        app_frame(page).wait_for_load_state("load")
        assert (assets + "table-treatments.css", 200) in loaded
        assert (assets + "table-treatments.js", 200) in loaded
