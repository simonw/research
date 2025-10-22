"""
Demonstrates the issue with creating fixtures that depend on
pytest-unused-port's unused_port_server fixture.

The problem: unused_port_server has function scope (default),
so you cannot create a fixture with a broader scope (like module or session)
that depends on it.
"""

import pytest


# This will FAIL - you cannot have a module-scoped fixture depend on a function-scoped fixture
@pytest.fixture(scope='module')
def my_module_server(unused_port_server):
    """
    Attempting to create a module-scoped fixture that reuses unused_port_server.

    This will fail with:
    ScopeMismatch: You tried to access the function scoped fixture unused_port_server
    with a module scoped request object, involved factories:
    """
    unused_port_server.start('.')
    yield unused_port_server
    # Cleanup happens automatically via unused_port_server


def test_1(my_module_server):
    """First test using module server."""
    print(f"Test 1 using port {my_module_server.port}")
    assert my_module_server.port > 0


def test_2(my_module_server):
    """Second test should reuse the same module server."""
    print(f"Test 2 using port {my_module_server.port}")
    assert my_module_server.port > 0
