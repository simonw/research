"""
Improved version of pytest-unused-port that supports configurable scopes.

This demonstrates what changes would be needed to allow users to create
fixtures with different scopes that build on top of the base fixtures.

The key changes:
1. Add scope-specific variants of unused_port
2. Add scope-specific variants of unused_port_server
3. Allow users to choose which scope they need
"""

import socket
import subprocess
import sys
import time
import pytest


class StaticServer:
    """
    Manages a static file HTTP server running on a port.

    Provides methods to start and stop the server, with automatic cleanup.
    """

    def __init__(self, port):
        self.port = port
        self._process = None
        self._directory = None

    def start(self, directory='.'):
        """
        Start an HTTP server serving the specified directory.

        Args:
            directory: Path to the directory to serve (default: current directory)
        """
        if self._process is not None:
            raise RuntimeError("Server is already running")

        self._directory = directory
        # Start python -m http.server on the specified port and directory
        self._process = subprocess.Popen(
            [sys.executable, '-m', 'http.server', str(self.port), '--directory', directory],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )

        # Wait a bit for the server to start
        time.sleep(0.1)

        # Verify the server is actually running
        if self._process.poll() is not None:
            # Process died immediately
            stdout, stderr = self._process.communicate()
            raise RuntimeError(f"Server failed to start: {stderr}")

        return self

    def stop(self):
        """Stop the HTTP server if it's running."""
        if self._process is not None:
            self._process.terminate()
            try:
                self._process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self._process.kill()
                self._process.wait()
            self._process = None

    def __enter__(self):
        """Context manager support."""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Ensure cleanup on context manager exit."""
        self.stop()
        return False


# ============================================================================
# SOLUTION 1: Provide separate fixtures for each scope
# ============================================================================

# Function scope (default - backward compatible)
@pytest.fixture(scope='function')
def unused_port():
    """Returns an unused port number on localhost (function scope)."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()


@pytest.fixture(scope='class')
def unused_port_class():
    """Returns an unused port number on localhost (class scope)."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()


@pytest.fixture(scope='module')
def unused_port_module():
    """Returns an unused port number on localhost (module scope)."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()


@pytest.fixture(scope='session')
def unused_port_session():
    """Returns an unused port number on localhost (session scope)."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()


# Corresponding server fixtures for each scope
@pytest.fixture(scope='function')
def unused_port_server(unused_port):
    """Returns a StaticServer instance (function scope)."""
    server = StaticServer(unused_port)
    yield server
    server.stop()


@pytest.fixture(scope='class')
def unused_port_server_class(unused_port_class):
    """Returns a StaticServer instance (class scope)."""
    server = StaticServer(unused_port_class)
    yield server
    server.stop()


@pytest.fixture(scope='module')
def unused_port_server_module(unused_port_module):
    """Returns a StaticServer instance (module scope)."""
    server = StaticServer(unused_port_module)
    yield server
    server.stop()


@pytest.fixture(scope='session')
def unused_port_server_session(unused_port_session):
    """Returns a StaticServer instance (session scope)."""
    server = StaticServer(unused_port_session)
    yield server
    server.stop()


# ============================================================================
# SOLUTION 2: Use pytest's params to allow scope configuration
# ============================================================================

# This is more advanced and would require users to configure via pytest.ini
# or conftest.py, but provides the most flexibility

def _get_unused_port():
    """Helper function to get an unused port."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()


# Alternative: Create factory fixtures
@pytest.fixture(scope='session')
def unused_port_factory():
    """
    Factory fixture that returns a function to get unused ports.

    This allows users to get ports in their own fixtures with any scope.
    """
    return _get_unused_port


@pytest.fixture(scope='session')
def static_server_factory():
    """
    Factory fixture that returns a class to create StaticServer instances.

    This allows users to create servers in their own fixtures with any scope.
    """
    return StaticServer
