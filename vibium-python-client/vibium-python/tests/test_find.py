"""Test element finding functionality."""

import pytest

from vibium_python import browser


class TestFind:
    """Tests for vibe.find() functionality."""

    def test_find_returns_element(self, clicker_path, test_server):
        """vibe.find() should return an Element with info."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")
            element = vibe.find("h1")

            assert element is not None
            assert element.tag.lower() == "h1"
            assert element.text == "Vibium Test Page"
            assert element.bounding_box.width > 0
            assert element.bounding_box.height > 0
        finally:
            vibe.quit()

    def test_find_with_id_selector(self, clicker_path, test_server):
        """vibe.find() should work with ID selectors."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")
            element = vibe.find("#welcome")

            assert element.text == "Welcome to the test page!"
        finally:
            vibe.quit()

    def test_find_with_class_selector(self, clicker_path, test_server):
        """vibe.find() should work with class selectors."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")
            element = vibe.find(".container")

            assert element is not None
            assert "Vibium Test Page" in element.text
        finally:
            vibe.quit()

    def test_find_input_element(self, clicker_path, test_server):
        """vibe.find() should find input elements."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")
            element = vibe.find("#name-input")

            assert element.tag.lower() == "input"
        finally:
            vibe.quit()

    def test_find_link_element(self, clicker_path, test_server):
        """vibe.find() should find link elements."""
        vibe = browser.launch(headless=True, executable_path=clicker_path)
        try:
            vibe.go(test_server.base_url + "/index.html")
            element = vibe.find("#link")

            assert element.tag.lower() == "a"
            assert "Page 2" in element.text
        finally:
            vibe.quit()
