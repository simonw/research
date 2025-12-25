"""Test screenshot functionality."""

import pytest

from vibium_python import browser


class TestScreenshot:
    """Tests for vibe.screenshot() functionality."""

    def test_screenshot_returns_png_bytes(self, clicker_path, test_server):
        """vibe.screenshot() should return PNG image bytes."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")

            png_data = vibe.screenshot()

            # Verify it's PNG data (PNG magic bytes)
            assert png_data[:8] == b'\x89PNG\r\n\x1a\n'
            assert len(png_data) > 1000  # Should be more than 1KB
        finally:
            vibe.quit()

    def test_screenshot_after_navigation(self, clicker_path, test_server):
        """Screenshots should work after navigation."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")
            png1 = vibe.screenshot()

            # Navigate to another page
            link = vibe.find("#link")
            link.click()
            import time
            time.sleep(0.5)

            png2 = vibe.screenshot()

            # Both should be valid PNGs but different content
            assert png1[:8] == b'\x89PNG\r\n\x1a\n'
            assert png2[:8] == b'\x89PNG\r\n\x1a\n'
            # They should be different (different pages)
            assert png1 != png2
        finally:
            vibe.quit()
