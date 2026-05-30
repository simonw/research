"""End-to-end Playwright test: the full Datasette ASGI app, captured by the
service worker and running in Pyodide in the browser.

Proves navigation (database -> table pages) and JavaScript fetch() of Datasette's
.json API are both intercepted and answered by Datasette in the Web Worker.
"""

import socket
import subprocess
import time
from pathlib import Path

import pytest
from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parent.parent
BOOT_TIMEOUT_MS = 300_000  # installing Datasette + deps takes a while


def _free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


@pytest.fixture(scope="module")
def server():
    port = _free_port()
    proc = subprocess.Popen(
        ["python3", "-m", "http.server", str(port), "--bind", "127.0.0.1",
         "--directory", str(ROOT)],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    for _ in range(50):
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                break
        except OSError:
            time.sleep(0.1)
    yield f"http://127.0.0.1:{port}"
    proc.terminate()
    proc.wait()


@pytest.fixture(scope="module")
def ds_page(server):
    with sync_playwright() as p:
        browser = p.chromium.launch()
        context = browser.new_context()
        page = context.new_page()
        page.goto(server + "/datasette.html")
        deadline = time.time() + BOOT_TIMEOUT_MS / 1000
        while True:
            state = page.locator("#status").get_attribute("data-state") or ""
            if state == "ready":
                break
            if state.startswith("error"):
                raise AssertionError("Datasette boot failed: " + state)
            if time.time() > deadline:
                raise AssertionError("Datasette boot timed out, last state: " + state)
            time.sleep(0.5)
        yield page
        context.close()
        browser.close()


def goto_app(page, path):
    page.evaluate("(p) => { document.getElementById('appframe').src = p; }", path)
    return page.frame_locator("#appframe")


def app_frame(page):
    for fr in page.frames:
        if "/app" in fr.url:
            return fr
    return page.frames[-1]


def test_datasette_home_served_in_browser(ds_page):
    frame = goto_app(ds_page, "/app/")
    # The database created in lifespan setup is listed.
    expect(frame.locator("a[href='/app/demo']")).to_be_visible()


def test_datasette_table_navigation(ds_page):
    frame = goto_app(ds_page, "/app/")
    frame.locator("a[href='/app/demo']").click()          # -> database page
    expect(frame.locator("a[href='/app/demo/items']")).to_be_visible()
    frame.locator("a[href='/app/demo/items']").click()    # -> table page
    expect(frame.locator("body")).to_contain_text("Widget")
    expect(frame.locator("body")).to_contain_text("Sprocket")


def test_datasette_json_api_fetch_is_intercepted(ds_page):
    # Drive a JavaScript fetch() from inside the page to Datasette's .json API;
    # the service worker must intercept it and Datasette must answer with rows.
    goto_app(ds_page, "/app/")
    ds_page.wait_for_timeout(500)
    frame = app_frame(ds_page)
    result = frame.evaluate(
        """async () => {
            const r = await fetch('/app/demo/items.json?_shape=array');
            const rows = await r.json();
            return { status: r.status, names: rows.map(x => x.name).sort() };
        }"""
    )
    assert result["status"] == 200
    assert result["names"] == ["Gadget", "Sprocket", "Widget"]


def test_jump_endpoint_is_prefixed_and_intercepted(ds_page):
    # The "Jump to" search component's endpoint must be rewritten under /app/ so
    # its client-side fetch is intercepted (Datasette hardcodes it without base_url).
    frame = goto_app(ds_page, "/app/")
    expect(frame.locator("navigation-search")).to_have_attribute("url", "/app/-/jump")
    f = app_frame(ds_page)
    status = f.evaluate("async () => (await fetch('/app/-/jump?q=d')).status")
    assert status == 200


def test_execute_write_page_loads_in_iframe(ds_page):
    # Datasette's write pages send X-Frame-Options: DENY + CSP frame-ancestors,
    # which would block iframe rendering; the service worker strips them so the
    # page (root-only) renders inside our frame.
    frame = goto_app(ds_page, "/app/demo/-/execute-write")
    # The framed document only has this DOM if framing wasn't blocked (a blocked
    # frame shows a browser error page instead). The textarea itself is hidden
    # behind Datasette's CodeMirror editor, so assert it is attached, not visible.
    expect(frame.locator("textarea#sql-editor")).to_be_attached()


def test_table_json_export_link_resolves(ds_page):
    # Datasette double-prefixes export links (/app/app/...); after the fix the
    # "json" link must resolve to data, not a "Database not found" 404, and the
    # parent hash must reflect the clean path.
    frame = goto_app(ds_page, "/app/demo/items")
    frame.locator("a[href='/app/demo/items.json']").first.click()
    expect(frame.locator("body")).to_contain_text("Widget")
    expect(frame.locator("body")).not_to_contain_text("Database not found")
    ds_page.wait_for_function("() => location.hash === '#/demo/items.json'")


def test_hash_routing_syncs_parent_url(ds_page):
    goto_app(ds_page, "/app/")
    frame = ds_page.frame_locator("#appframe")
    # Navigating inside the iframe updates the parent URL #fragment...
    frame.locator("a[href='/app/demo']").click()
    ds_page.wait_for_function("() => location.hash === '#/demo'")
    # ...and editing the parent #fragment drives the iframe.
    ds_page.evaluate("() => { location.hash = '#/demo/items'; }")
    expect(frame.locator("body")).to_contain_text("Widget")
    ds_page.wait_for_function(
        "() => document.getElementById('appframe').contentWindow.location.pathname"
        " === '/app/demo/items'"
    )


def test_logged_in_as_root_and_post_write_works(ds_page):
    # Runs last: it inserts a row. Proves the user is root AND a POST write
    # (with real same-origin Sec-Fetch-Site headers) is intercepted + accepted.
    goto_app(ds_page, "/app/")
    ds_page.wait_for_timeout(500)
    frame = app_frame(ds_page)
    result = frame.evaluate(
        """async () => {
            const actor = (await (await fetch('/app/-/actor.json')).json()).actor;
            const resp = await fetch('/app/demo/items/-/insert', {
                method: 'POST',
                headers: { 'content-type': 'application/json' },
                body: JSON.stringify({ rows: [{ name: 'Cog', qty: 99 }] }),
            });
            const rows = await (await fetch('/app/demo/items.json?_shape=array')).json();
            return { actor, status: resp.status, names: rows.map(r => r.name) };
        }"""
    )
    assert result["actor"] == {"id": "root"}
    assert result["status"] == 201
    assert "Cog" in result["names"]
