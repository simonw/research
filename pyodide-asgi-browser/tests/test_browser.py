"""End-to-end Playwright tests driving real Chromium.

These prove the full stack: the shell boots Pyodide in a Web Worker, installs FastAPI,
registers the service worker, and the SW then answers link navigations, a form POST
(303 redirect), a JSON fetch(), and a page that runs its OWN inline JavaScript which
itself issues an intercepted fetch().

Booting Pyodide + installing FastAPI from the network is slow, so the booted page is
shared across tests via a module-scoped fixture.
"""

import socket
import subprocess
import time
from pathlib import Path

import pytest
from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parent.parent
BOOT_TIMEOUT_MS = 240_000  # Pyodide download + micropip install fastapi can be slow


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
    # Wait for the server to accept connections.
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
def app_page(server):
    with sync_playwright() as p:
        browser = p.chromium.launch()
        context = browser.new_context()
        page = context.new_page()
        page.goto(server + "/")
        # Wait until the shell reports the Pyodide worker + FastAPI are ready,
        # failing fast (with the message) if the worker reports an error instead.
        deadline = time.time() + BOOT_TIMEOUT_MS / 1000
        while True:
            state = page.locator("#status").get_attribute("data-state") or ""
            if state == "ready":
                break
            if state.startswith("error"):
                raise AssertionError("boot failed: " + state)
            if time.time() > deadline:
                raise AssertionError("boot timed out, last state: " + state)
            time.sleep(0.5)
        yield page, server
        context.close()
        browser.close()


def goto_app(page, path):
    """Point the captured-app iframe at a path and return its frame locator."""
    page.evaluate("(p) => { document.getElementById('appframe').src = p; }", path)
    return page.frame_locator("#appframe")


def test_home_is_served_by_pyodide(app_page):
    page, _ = app_page
    frame = goto_app(page, "/app/")
    expect(frame.locator("#home-marker")).to_contain_text("FastAPI running in Pyodide")


def test_link_navigation_is_intercepted(app_page):
    page, _ = app_page
    frame = goto_app(page, "/app/")
    frame.locator("#nav-about").click()
    expect(frame.locator("#about-marker")).to_contain_text("about page")
    # And we can navigate back via a link too.
    frame.locator("#back-home").click()
    expect(frame.locator("#home-marker")).to_be_visible()


def test_form_post_redirects_through_asgi(app_page):
    page, _ = app_page
    frame = goto_app(page, "/app/")
    frame.locator("#name-input").fill("Simon")
    frame.locator("#greet-btn").click()
    # POST -> 303 -> GET /app/thanks?name=Simon, all handled by the SW/ASGI bridge.
    expect(frame.locator("#thanks-marker")).to_have_text("Thanks, Simon!")


def test_page_runs_its_own_js_fetch(app_page):
    page, _ = app_page
    frame = goto_app(page, "/app/widget")
    # The widget page's own inline JS does fetch('/app/api/items'); that fetch is
    # itself intercepted and answered by FastAPI. Lifespan startup seeded 3 items.
    expect(frame.locator("body")).to_have_attribute(
        "data-items-loaded", "3", timeout=30_000
    )
    items = frame.locator(".item")
    expect(items).to_have_count(3)
    expect(items.nth(0)).to_have_text("Widget")
    expect(items.nth(2)).to_have_text("Sprocket")
