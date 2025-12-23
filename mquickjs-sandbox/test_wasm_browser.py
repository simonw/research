"""
Pytest tests for mquickjs Pyodide integration using Playwright.

Run with: pytest test_pyodide_playwright.py -v
"""

import pytest
import http.server
import threading
import os
from pathlib import Path

from playwright.sync_api import sync_playwright, Page, expect


# Path to test files
SCRIPT_DIR = Path(__file__).parent


class TestServer:
    """Simple HTTP server for serving test files."""

    def __init__(self, directory: Path, port: int = 8765):
        self.directory = directory
        self.port = port
        self.server = None
        self.thread = None

    def start(self):
        """Start the server."""
        os.chdir(self.directory)

        handler = http.server.SimpleHTTPRequestHandler

        # Suppress request logging
        class QuietHandler(handler):
            def log_message(self, format, *args):
                pass

        self.server = http.server.HTTPServer(("localhost", self.port), QuietHandler)
        self.thread = threading.Thread(target=self.server.serve_forever)
        self.thread.daemon = True
        self.thread.start()

    def stop(self):
        """Stop the server."""
        if self.server:
            self.server.shutdown()
            self.thread.join(timeout=5)


@pytest.fixture(scope="module")
def server():
    """Start a test HTTP server."""
    srv = TestServer(SCRIPT_DIR, port=8765)
    srv.start()
    yield srv
    srv.stop()


@pytest.fixture(scope="module")
def browser():
    """Create a Playwright browser."""
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        yield browser
        browser.close()


@pytest.fixture
def page(browser, server):
    """Create a new page."""
    page = browser.new_page()
    yield page
    page.close()


class TestBrowserMQuickJS:
    """Test mquickjs running in browser via Playwright."""

    def test_wasm_loads(self, page: Page, server):
        """Test that mquickjs WASM loads successfully."""
        page.goto(f"http://localhost:{server.port}/test_wasm_browser.html")

        # Wait for tests to complete (with timeout)
        page.wait_for_function("window.testComplete === true", timeout=60000)

        # Get results
        results = page.evaluate("window.testResults")

        assert "error" not in results, f"Test error: {results.get('error')}"
        assert results["passed"] >= 1, "Expected at least one test to pass"

    def test_arithmetic(self, page: Page, server):
        """Test arithmetic operations."""
        page.goto(f"http://localhost:{server.port}/test_wasm_browser.html")
        page.wait_for_function("window.testComplete === true", timeout=60000)

        results = page.evaluate("window.testResults")
        assert "error" not in results

        # Find arithmetic test
        for test in results["tests"]:
            if test["name"] == "arithmetic":
                assert test["passed"], f"Arithmetic test failed"
                return

        pytest.fail("Arithmetic test not found")

    def test_string_concat(self, page: Page, server):
        """Test string concatenation."""
        page.goto(f"http://localhost:{server.port}/test_wasm_browser.html")
        page.wait_for_function("window.testComplete === true", timeout=60000)

        results = page.evaluate("window.testResults")
        assert "error" not in results

        for test in results["tests"]:
            if test["name"] == "string concat":
                assert test["passed"], f"String concat test failed"
                return

        pytest.fail("String concat test not found")

    def test_math_functions(self, page: Page, server):
        """Test Math functions."""
        page.goto(f"http://localhost:{server.port}/test_wasm_browser.html")
        page.wait_for_function("window.testComplete === true", timeout=60000)

        results = page.evaluate("window.testResults")
        assert "error" not in results

        math_tests = ["Math.abs", "Math.floor"]
        for test in results["tests"]:
            if test["name"] in math_tests:
                assert test["passed"], f"{test['name']} test failed"

    def test_all_pass(self, page: Page, server):
        """Test that all tests pass."""
        page.goto(f"http://localhost:{server.port}/test_wasm_browser.html")
        page.wait_for_function("window.testComplete === true", timeout=60000)

        results = page.evaluate("window.testResults")
        assert "error" not in results
        assert results["failed"] == 0, f"{results['failed']} tests failed"
        assert results["passed"] == len(results["tests"]), "Not all tests passed"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
