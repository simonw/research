"""Test click functionality."""

import pytest

from vibium_python import browser


class TestClick:
    """Tests for element.click() functionality."""

    def test_click_button_changes_text(self, clicker_path, test_server):
        """Clicking a button should update the page."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")

            # Find and click the counter button
            button = vibe.find("#counter-btn")
            assert "Click count: 0" in button.text

            button.click()

            # Find the button again to get updated text
            button = vibe.find("#counter-btn")
            assert "Click count: 1" in button.text
        finally:
            vibe.quit()

    def test_click_link_navigates(self, clicker_path, test_server):
        """Clicking a link should navigate to a new page."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")

            # Click the link to page 2
            link = vibe.find("#link")
            link.click()

            # Wait for navigation and verify we're on page 2
            import time
            time.sleep(0.5)

            h1 = vibe.find("h1")
            assert h1.text == "Page 2"
        finally:
            vibe.quit()
