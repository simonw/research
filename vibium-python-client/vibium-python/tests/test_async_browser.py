"""Test async browser functionality."""

import pytest

from vibium_python import async_browser


@pytest.mark.asyncio
class TestAsyncBrowser:
    """Tests for async browser API."""

    async def test_async_launch(self, clicker_path):
        """async_browser.launch() should return an AsyncVibe instance."""
        vibe = await async_browser.launch(headless=True, executable_path=clicker_path)
        try:
            assert vibe is not None
            assert hasattr(vibe, "go")
            assert hasattr(vibe, "find")
            assert hasattr(vibe, "screenshot")
            assert hasattr(vibe, "quit")
        finally:
            await vibe.quit()

    async def test_async_navigation(self, clicker_path, test_server):
        """Async navigation should work."""
        vibe = await async_browser.launch(headless=True, executable_path=clicker_path)
        try:
            await vibe.go(test_server.base_url + "/index.html")
            h1 = await vibe.find("h1")
            assert h1.text == "Vibium Test Page"
        finally:
            await vibe.quit()

    async def test_async_find(self, clicker_path, test_server):
        """Async find should return AsyncElement."""
        vibe = await async_browser.launch(headless=True, executable_path=clicker_path)
        try:
            await vibe.go(test_server.base_url + "/index.html")
            element = await vibe.find("#welcome")
            assert element.text == "Welcome to the test page!"
        finally:
            await vibe.quit()

    async def test_async_click(self, clicker_path, test_server):
        """Async click should work."""
        vibe = await async_browser.launch(headless=True, executable_path=clicker_path)
        try:
            await vibe.go(test_server.base_url + "/index.html")

            button = await vibe.find("#counter-btn")
            assert "Click count: 0" in button.text

            await button.click()

            button = await vibe.find("#counter-btn")
            assert "Click count: 1" in button.text
        finally:
            await vibe.quit()

    async def test_async_type(self, clicker_path, test_server):
        """Async type should work."""
        import asyncio

        vibe = await async_browser.launch(headless=True, executable_path=clicker_path)
        try:
            await vibe.go(test_server.base_url + "/index.html")

            input_elem = await vibe.find("#name-input")
            await input_elem.type("AsyncTest")

            button = await vibe.find("#greet-btn")
            await button.click()

            await asyncio.sleep(0.2)
            result = await vibe.find("#result")
            assert "Hello, AsyncTest!" in result.text
        finally:
            await vibe.quit()

    async def test_async_screenshot(self, clicker_path, test_server):
        """Async screenshot should return PNG bytes."""
        vibe = await async_browser.launch(headless=True, executable_path=clicker_path)
        try:
            await vibe.go(test_server.base_url + "/index.html")
            png_data = await vibe.screenshot()

            # Verify PNG magic bytes
            assert png_data[:8] == b'\x89PNG\r\n\x1a\n'
            assert len(png_data) > 1000
        finally:
            await vibe.quit()
