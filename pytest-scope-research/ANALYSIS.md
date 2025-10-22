# Detailed Analysis of pytest-unused-port Scope Limitation

## Current Implementation

The current `pytest-unused-port` library (v0.1) has two main fixtures:

```python
@pytest.fixture
def unused_port():
    """Returns an unused port number on localhost."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()


@pytest.fixture
def unused_port_server(unused_port):
    """Returns a StaticServer instance."""
    server = StaticServer(unused_port)
    yield server
    server.stop()
```

Both fixtures have **implicit function scope** (the default).

## The Scope Hierarchy

```
┌─────────────────────────────────────────────┐
│  SESSION (broadest)                         │
│  ┌───────────────────────────────────────┐  │
│  │  PACKAGE                              │  │
│  │  ┌─────────────────────────────────┐  │  │
│  │  │  MODULE                         │  │  │
│  │  │  ┌───────────────────────────┐  │  │  │
│  │  │  │  CLASS                    │  │  │  │
│  │  │  │  ┌─────────────────────┐  │  │  │  │
│  │  │  │  │  FUNCTION (default)│  │  │  │  │
│  │  │  │  └─────────────────────┘  │  │  │  │
│  │  │  └───────────────────────────┘  │  │  │
│  │  └─────────────────────────────────┘  │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘

Dependency Rule: Inner scopes can depend on outer scopes
                 Outer scopes CANNOT depend on inner scopes
```

## Why the Current Implementation Fails

When a user tries to create a broader-scoped fixture:

```python
@pytest.fixture(scope='module')  # ← Broader scope
def my_module_server(unused_port_server):  # ← Function scope
    """Trying to reuse the server across all tests in a module."""
    unused_port_server.start('.')
    yield unused_port_server
```

Pytest's fixture resolution fails because:
1. `my_module_server` has module scope (lives for the entire module)
2. `unused_port_server` has function scope (recreated for each test)
3. Pytest cannot instantiate a long-lived (module) fixture that depends on a short-lived (function) fixture

**Error:**
```
ScopeMismatch: You tried to access the function scoped fixture unused_port_server
with a module scoped request object.
```

## Root Cause Analysis

The library's design assumes all users want function-scoped fixtures. This assumption breaks down when users need:

1. **Expensive setup once per module** - Start a server once for all tests in a file
2. **Shared state across test class** - Multiple tests in a class using same port
3. **Session-wide resources** - One server for entire test suite
4. **Custom fixture composition** - Building higher-level fixtures with specific scopes

## Proposed Changes to pytest-unused-port

### Minimal Change (Backward Compatible)

Add explicit scope parameter to existing fixtures:

```python
@pytest.fixture(scope='function')  # ← Make explicit
def unused_port():
    # ... existing code ...

@pytest.fixture(scope='function')  # ← Make explicit
def unused_port_server(unused_port):
    # ... existing code ...
```

**Impact:** None - just documents the current behavior

### Recommended Change: Add Scope Variants

Add fixtures for common scopes:

```python
# Function scope (existing - keep for backward compatibility)
@pytest.fixture(scope='function')
def unused_port():
    """Returns an unused port (function scope)."""
    return _get_unused_port()

@pytest.fixture(scope='function')
def unused_port_server(unused_port):
    """Returns a StaticServer (function scope)."""
    server = StaticServer(unused_port)
    yield server
    server.stop()


# Module scope (new - most commonly needed)
@pytest.fixture(scope='module')
def unused_port_module():
    """Returns an unused port (module scope)."""
    return _get_unused_port()

@pytest.fixture(scope='module')
def unused_port_server_module(unused_port_module):
    """Returns a StaticServer (module scope)."""
    server = StaticServer(unused_port_module)
    yield server
    server.stop()


# Session scope (new - for expensive operations)
@pytest.fixture(scope='session')
def unused_port_session():
    """Returns an unused port (session scope)."""
    return _get_unused_port()

@pytest.fixture(scope='session')
def unused_port_server_session(unused_port_session):
    """Returns a StaticServer (session scope)."""
    server = StaticServer(unused_port_session)
    yield server
    server.stop()


# Helper function (private)
def _get_unused_port():
    """Internal helper to get an unused port."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()
```

**Benefits:**
- ✅ Backward compatible (existing fixtures unchanged)
- ✅ Clear naming convention (`_module`, `_session` suffixes)
- ✅ Users can compose their own fixtures with matching scopes
- ✅ Minimal code duplication (shared `_get_unused_port` helper)

### Advanced Change: Factory Pattern

Add factory fixtures for maximum flexibility:

```python
@pytest.fixture(scope='session')
def unused_port_factory():
    """
    Factory that returns a function to get unused ports.

    Use this to create your own fixtures with custom scopes:

        @pytest.fixture(scope='module')
        def my_port(unused_port_factory):
            return unused_port_factory()
    """
    def _factory():
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
            return port
        finally:
            sock.close()
    return _factory


@pytest.fixture(scope='session')
def static_server_factory():
    """
    Factory that returns the StaticServer class.

    Use this to create your own server fixtures with custom scopes:

        @pytest.fixture(scope='module')
        def my_server(unused_port_factory, static_server_factory):
            port = unused_port_factory()
            server = static_server_factory(port)
            yield server
            server.stop()
    """
    return StaticServer
```

**Benefits:**
- ✅ Maximum flexibility - users control scope
- ✅ No fixture duplication in the library
- ✅ Advanced users can create custom compositions
- ⚠️ Requires users to write more boilerplate

## Comparison of Solutions

| Aspect | Current | Scope Variants | Factory Pattern |
|--------|---------|----------------|-----------------|
| Backward Compatible | N/A | ✅ Yes | ✅ Yes |
| Easy to Use | ✅ Simple | ✅ Simple | ⚠️ Requires boilerplate |
| Flexible | ❌ No | ⚠️ Limited to provided scopes | ✅ Full flexibility |
| Code in Library | Minimal | Medium | Minimal |
| Code in User Tests | Minimal | Minimal | Medium |
| Discoverability | ✅ Good | ✅ Good | ⚠️ Requires docs |

## Recommended Hybrid Approach

Implement both solutions:

1. **Add scope variants** for `module` and `session` (most common needs)
2. **Add factory fixtures** for advanced use cases
3. **Keep existing fixtures** unchanged for backward compatibility

This provides:
- Simple path for common cases (scope variants)
- Advanced path for custom needs (factories)
- No breaking changes (existing code works)

## Example User Code After Fix

```python
# Use case 1: Module-scoped server (simple)
@pytest.fixture(scope='module')
def my_api_server(unused_port_server_module):
    """Start API server once for all tests in this module."""
    unused_port_server_module.start('./api')
    yield unused_port_server_module

def test_endpoint_1(my_api_server):
    # Test using the shared server
    pass

def test_endpoint_2(my_api_server):
    # Same server instance
    pass


# Use case 2: Custom scope with factory (advanced)
@pytest.fixture(scope='class')
def class_server(unused_port_factory, static_server_factory):
    """Server with class scope - custom logic."""
    port = unused_port_factory()
    server = static_server_factory(port)
    # Custom setup
    server.start('./custom')
    yield server
    # Custom teardown
    server.stop()


class TestAPI:
    def test_1(self, class_server):
        pass

    def test_2(self, class_server):
        # Same instance as test_1
        pass
```

## Testing the Proposed Changes

See `improved_unused_port.py` for a working implementation and `test_improved_solution.py` for tests demonstrating all use cases.

## Conclusion

The current limitation stems from hardcoded function scope. Adding scope variants and/or factory fixtures would make the library significantly more flexible while maintaining backward compatibility. The recommended hybrid approach provides both simplicity for common cases and flexibility for advanced needs.
