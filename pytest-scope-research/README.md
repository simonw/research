# Pytest Scopes and pytest-unused-port Research

This project demonstrates pytest fixture scopes and investigates limitations with the `pytest-unused-port` library.

## Overview

When trying to create a fixture with a different scope (e.g., `module` or `session`) that depends on `pytest-unused-port`'s `unused_port_server` fixture, pytest raises a `ScopeMismatch` error. This is because `unused_port_server` has the default `function` scope, and pytest fixtures with broader scopes cannot depend on fixtures with narrower scopes.

## Pytest Fixture Scopes

Pytest supports 5 different fixture scopes, from narrowest to broadest:

1. **function** (default) - Fixture runs once per test function
2. **class** - Fixture runs once per test class
3. **module** - Fixture runs once per test module (file)
4. **package** - Fixture runs once per test package (directory)
5. **session** - Fixture runs once per entire test session

### Key Rule: Scope Dependencies

**A fixture can only depend on fixtures with the same or broader scope.**

Examples:
- ✅ Function-scoped fixture can depend on function, class, module, or session-scoped fixtures
- ✅ Module-scoped fixture can depend on module or session-scoped fixtures
- ❌ Module-scoped fixture CANNOT depend on function or class-scoped fixtures
- ❌ Session-scoped fixture CANNOT depend on module, class, or function-scoped fixtures

## The Problem with pytest-unused-port

Looking at the source code of `pytest-unused-port`:

```python
@pytest.fixture
def unused_port():
    """Returns an unused port number (function scope - default)."""
    # ... implementation ...

@pytest.fixture
def unused_port_server(unused_port):
    """Returns a StaticServer instance (function scope - default)."""
    # ... implementation ...
```

Both fixtures use the default scope (`function`). This means:

```python
# This FAILS with ScopeMismatch error
@pytest.fixture(scope='module')
def my_module_server(unused_port_server):
    # Cannot use function-scoped unused_port_server in module-scoped fixture!
    pass
```

### Error Message

```
ScopeMismatch: You tried to access the function scoped fixture unused_port_server
with a module scoped request object.
```

See `test_unused_port_issue.py` for a demonstration.

## Files in this Project

### 1. `test_scopes_basic.py`

Comprehensive demonstration of all pytest scopes with detailed examples showing:
- How each scope behaves
- When fixtures are created and destroyed
- How scopes interact with each other

Run with: `pytest test_scopes_basic.py -v -s`

### 2. `test_unused_port_issue.py`

Demonstrates the actual problem - attempting to create a module-scoped fixture that depends on the function-scoped `unused_port_server`.

Run with: `pytest test_unused_port_issue.py -v` (will show errors)

### 3. `test_workarounds.py`

Shows workaround approaches:
- Creating your own module-scoped port and server fixtures from scratch
- Using function scope and accepting the limitation
- Why depending directly on `unused_port` also doesn't work

Run with: `pytest test_workarounds.py -v`

### 4. `improved_unused_port.py`

Proposed improvements to the `pytest-unused-port` library showing two solutions:

#### Solution 1: Scope-Specific Fixtures

Provide separate fixtures for each scope:
- `unused_port` / `unused_port_server` (function scope - backward compatible)
- `unused_port_class` / `unused_port_server_class` (class scope)
- `unused_port_module` / `unused_port_server_module` (module scope)
- `unused_port_session` / `unused_port_server_session` (session scope)

This allows users to choose the appropriate scope for their needs:

```python
@pytest.fixture(scope='module')
def my_custom_server(unused_port_server_module):
    # Both have module scope - no ScopeMismatch!
    yield unused_port_server_module
```

#### Solution 2: Factory Fixtures

Provide session-scoped factory fixtures that return callables:
- `unused_port_factory()` - Returns a function to get unused ports
- `static_server_factory()` - Returns the StaticServer class

This gives users maximum flexibility to create their own fixtures with any scope:

```python
@pytest.fixture(scope='module')
def my_server(unused_port_factory, static_server_factory):
    port = unused_port_factory()
    server = static_server_factory(port)
    yield server
    server.stop()
```

### 5. `test_improved_solution.py`

Demonstrates how the improved library would work with:
- Module-scoped server fixtures
- Wrapping existing fixtures with same scope
- Factory pattern for maximum flexibility
- Class-scoped fixtures

Run with: `pytest test_improved_solution.py -v`

## Running the Tests

```bash
# Set up the environment
python -m venv venv
source venv/bin/activate
pip install -r requirements.txt

# Run all tests
pytest -v

# Run with output to see fixture lifecycle
pytest test_scopes_basic.py -v -s

# See the scope mismatch error
pytest test_unused_port_issue.py -v

# Test the improved solutions
pytest test_improved_solution.py -v
```

## Recommendations for pytest-unused-port

To make the library more flexible, consider implementing:

1. **Scope-specific fixtures** (Solution 1) - Simple and explicit
   - Pros: Clear naming, easy to understand, backward compatible
   - Cons: More fixtures to maintain, some code duplication

2. **Factory fixtures** (Solution 2) - Most flexible
   - Pros: Maximum flexibility, minimal fixtures, users control scope
   - Cons: Requires users to write more boilerplate code

3. **Hybrid approach** - Provide both:
   - Keep existing function-scoped fixtures for backward compatibility
   - Add `_module` and `_session` variants for common use cases
   - Add factory fixtures for advanced users who need full control

## Key Takeaways

1. **Fixture scope matters** - Understanding scopes is crucial for effective pytest usage
2. **Dependencies must respect scope hierarchy** - Broader-scoped fixtures cannot depend on narrower-scoped fixtures
3. **Library design considerations** - Libraries providing fixtures should consider whether users might need different scopes
4. **Workarounds exist** - You can always create your own fixtures from scratch, but it's better if libraries provide the flexibility you need

## Further Reading

- [Pytest Fixture Scopes Documentation](https://docs.pytest.org/en/latest/how-to/fixtures.html#scope-sharing-fixtures-across-classes-modules-packages-or-session)
- [Pytest Fixture Finalization](https://docs.pytest.org/en/latest/how-to/fixtures.html#teardown-cleanup-aka-fixture-finalization)
