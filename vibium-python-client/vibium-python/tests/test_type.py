"""Test type functionality."""

import pytest

from vibium_python import browser


class TestType:
    """Tests for element.type() functionality."""

    def test_type_into_input(self, clicker_path, test_server):
        """Typing into an input should update its value."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")

            # Find the input and type into it
            input_elem = vibe.find("#name-input")
            input_elem.type("Claude")

            # Click the greet button
            button = vibe.find("#greet-btn")
            button.click()

            # Check the result
            import time
            time.sleep(0.2)
            result = vibe.find("#result")
            assert "Hello, Claude!" in result.text
        finally:
            vibe.quit()

    def test_type_special_characters(self, clicker_path, test_server):
        """Typing should handle special characters."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")

            # Type text with special characters
            input_elem = vibe.find("#name-input")
            input_elem.type("Test123")

            # Verify by clicking greet
            button = vibe.find("#greet-btn")
            button.click()

            import time
            time.sleep(0.2)
            result = vibe.find("#result")
            assert "Hello, Test123!" in result.text
        finally:
            vibe.quit()
