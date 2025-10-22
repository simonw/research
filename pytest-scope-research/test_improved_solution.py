"""
Tests demonstrating how the improved pytest-unused-port would work.

These tests use the improved fixtures from improved_unused_port.py
"""

import pytest
from improved_unused_port import (
    unused_port_module,
    unused_port_server_module,
    unused_port_factory,
    static_server_factory,
)


# ============================================================================
# Using scope-specific fixtures
# ============================================================================

def test_module_scoped_1(unused_port_server_module):
    """First test with module-scoped server."""
    unused_port_server_module.start('.')
    print(f"Test 1 using module port {unused_port_server_module.port}")
    assert unused_port_server_module.port > 0


def test_module_scoped_2(unused_port_server_module):
    """Second test shares the same module-scoped server."""
    print(f"Test 2 using module port {unused_port_server_module.port}")
    assert unused_port_server_module.port > 0


# Example showing how you could wrap the server fixture with same scope
@pytest.fixture(scope='module')
def wrapped_server(unused_port_server_module):
    """
    Custom fixture that wraps unused_port_server_module.

    This works because both have module scope!
    You can add pre/post processing around the server.
    """
    # Custom setup logic before yielding the server
    print(f"\nWrapped setup - server on port {unused_port_server_module.port}")
    # The server instance is already created, we just wrap it
    yield unused_port_server_module
    # Custom teardown logic after the test
    print(f"Wrapped teardown - server on port {unused_port_server_module.port}")
    # The actual server stop() is handled by unused_port_server_module


def test_wrapped_1(wrapped_server):
    """Test using wrapped module-scoped fixture."""
    # Note: Don't call start() - the server may already be started
    # from other tests using the same module-scoped fixture
    print(f"Wrapped test 1 using port {wrapped_server.port}")
    assert wrapped_server.port > 0


def test_wrapped_2(wrapped_server):
    """Test using same wrapped module-scoped fixture."""
    print(f"Wrapped test 2 using port {wrapped_server.port}")
    assert wrapped_server.port > 0


# ============================================================================
# Using factory fixtures for maximum flexibility
# ============================================================================

@pytest.fixture(scope='module')
def my_factory_server(static_server_factory, unused_port_factory):
    """
    Custom fixture using factory pattern.

    This gives us complete control over scope while reusing the library's logic.
    """
    port = unused_port_factory()
    server = static_server_factory(port)
    yield server
    server.stop()


def test_factory_1(my_factory_server):
    """Test using factory-based fixture."""
    my_factory_server.start('.')
    print(f"Factory test 1 using port {my_factory_server.port}")
    assert my_factory_server.port > 0


def test_factory_2(my_factory_server):
    """Test using same factory-based fixture."""
    print(f"Factory test 2 using port {my_factory_server.port}")
    assert my_factory_server.port > 0


# ============================================================================
# Demonstrating different scopes can coexist
# ============================================================================

class TestClassScoped:
    """Tests using class-scoped fixtures."""

    @pytest.fixture(scope='class')
    def class_server(self, static_server_factory, unused_port_factory):
        """Class-scoped server for this test class."""
        port = unused_port_factory()
        server = static_server_factory(port)
        server.start('.')
        yield server
        server.stop()

    def test_class_1(self, class_server):
        """First test in class."""
        print(f"Class test 1 using port {class_server.port}")
        assert class_server.port > 0

    def test_class_2(self, class_server):
        """Second test in class shares same server."""
        print(f"Class test 2 using port {class_server.port}")
        assert class_server.port > 0
