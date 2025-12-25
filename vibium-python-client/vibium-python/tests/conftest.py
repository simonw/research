"""Pytest fixtures for vibium-python tests."""

import http.server
import socket
import subprocess
import threading
import time
from pathlib import Path

import pytest


def find_free_port():
    """Find an available port."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("", 0))
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        return s.getsockname()[1]


class TestServer:
    """A simple HTTP server for serving test fixtures."""

    def __init__(self, fixtures_dir: Path, port: int):
        self.fixtures_dir = fixtures_dir
        self.port = port
        self.server = None
        self.thread = None

    def start(self):
        """Start the HTTP server in a background thread."""
        handler = http.server.SimpleHTTPRequestHandler
        handler.directory = str(self.fixtures_dir)

        class QuietHandler(handler):
            def log_message(self, format, *args):
                pass  # Suppress logging

        self.server = http.server.HTTPServer(("127.0.0.1", self.port), QuietHandler)
        self.thread = threading.Thread(target=self.server.serve_forever)
        self.thread.daemon = True
        self.thread.start()
        # Wait for server to be ready
        time.sleep(0.1)

    def stop(self):
        """Stop the HTTP server."""
        if self.server:
            self.server.shutdown()
            self.thread.join(timeout=1)

    @property
    def base_url(self):
        """Get the base URL for the server."""
        return f"http://127.0.0.1:{self.port}"


@pytest.fixture(scope="session")
def fixtures_dir():
    """Path to the test fixtures directory."""
    return Path(__file__).parent / "fixtures"


@pytest.fixture(scope="session")
def test_server(fixtures_dir):
    """Start an HTTP server serving the fixtures directory."""
    port = find_free_port()
    server = TestServer(fixtures_dir, port)
    server.start()
    yield server
    server.stop()


@pytest.fixture(scope="session")
def clicker_path():
    """Path to the clicker binary."""
    # Try to find clicker in common locations
    paths = [
        Path("/tmp/vibium/clicker/bin/clicker"),
        Path.home() / ".local" / "bin" / "clicker",
        Path("/usr/local/bin/clicker"),
    ]

    for path in paths:
        if path.exists():
            return str(path)

    # Try to find it on PATH
    try:
        result = subprocess.run(
            ["which", "clicker"], capture_output=True, text=True
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except FileNotFoundError:
        pass

    pytest.skip("clicker binary not found")
