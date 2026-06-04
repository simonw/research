"""Test navigation functionality."""

import pytest

from vibium_python import browser


class TestNavigation:
    """Tests for navigation functionality."""

    def test_go_navigates_to_url(self, clicker_path, test_server):
        """vibe.go() should navigate to the given URL."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")
            # Find the h1 to verify we're on the right page
            h1 = vibe.find("h1")
            assert h1.text == "Vibium Test Page"
        finally:
            vibe.quit()

    def test_go_navigates_to_file_url(self, clicker_path, fixtures_dir):
        """vibe.go() should navigate to file:// URLs."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(f"file://{fixtures_dir}/index.html")
            h1 = vibe.find("h1")
            assert h1.text == "Vibium Test Page"
        finally:
            vibe.quit()
