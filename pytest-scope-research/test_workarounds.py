"""
Workarounds for the scope issue with pytest-unused-port.

Since unused_port_server is function-scoped, here are ways to work with it:
"""

import pytest
import socket
from pytest_unused_port import StaticServer


# WORKAROUND 1: Create your own module-scoped fixture from scratch
@pytest.fixture(scope='module')
def my_module_port():
    """Get an unused port for module scope."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()


@pytest.fixture(scope='module')
def my_module_server_v1(my_module_port):
    """Module-scoped server using our own port fixture."""
    server = StaticServer(my_module_port)
    yield server
    server.stop()


def test_workaround_1a(my_module_server_v1):
    """Test using workaround 1."""
    my_module_server_v1.start('.')
    print(f"Test 1a using port {my_module_server_v1.port}")
    assert my_module_server_v1.port > 0


def test_workaround_1b(my_module_server_v1):
    """Second test should use same port (module scope)."""
    print(f"Test 1b using port {my_module_server_v1.port}")
    assert my_module_server_v1.port > 0


# WORKAROUND 2: Use function scope but accept it
@pytest.fixture
def my_function_server(unused_port_server):
    """
    Function-scoped fixture that decorates unused_port_server.

    This works because both have the same scope.
    """
    # Can add extra logic here
    unused_port_server.start('.')
    yield unused_port_server
    # Additional cleanup if needed


def test_workaround_2a(my_function_server):
    """Test using function-scoped decorated fixture."""
    print(f"Test 2a using port {my_function_server.port}")
    assert my_function_server.port > 0


def test_workaround_2b(my_function_server):
    """Each test gets a new server (function scope)."""
    print(f"Test 2b using port {my_function_server.port}")
    assert my_function_server.port > 0


# WORKAROUND 3: Directly use unused_port fixture with broader scope
@pytest.fixture(scope='module')
def my_module_server_v2(unused_port):
    """
    Module-scoped server that depends only on unused_port.

    Note: unused_port is also function-scoped, so this will ALSO fail!
    """
    server = StaticServer(unused_port)
    yield server
    server.stop()


# This will also fail - commented out to avoid test errors
# def test_workaround_3(my_module_server_v2):
#     """This will fail with ScopeMismatch."""
#     pass
