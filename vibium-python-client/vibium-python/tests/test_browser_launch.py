"""Test browser launch functionality."""

import pytest

from vibium_python import browser


class TestBrowserLaunch:
    """Tests for browser.launch()"""

    def test_launch_returns_vibe_instance(self, clicker_path):
        """browser.launch() should return a Vibe instance."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            assert vibe is not None
            assert hasattr(vibe, "go")
            assert hasattr(vibe, "find")
            assert hasattr(vibe, "screenshot")
            assert hasattr(vibe, "quit")
        finally:
            vibe.quit()

    def test_launch_with_headless_option(self, clicker_path):
        """browser.launch(headless=True) should work without visible browser."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            assert vibe is not None
        finally:
            vibe.quit()
