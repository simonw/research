# Real World Example: The Actual Problem You Encountered

## What You Tried to Do

You wanted to create a custom fixture that builds on top of `unused_port_server` from pytest-unused-port, likely with a broader scope or additional functionality.

### Your Code (Probable)

```python
import pytest

# Attempting to create a module-scoped fixture
@pytest.fixture(scope='module')
def my_server(unused_port_server):
    """
    Custom server fixture that should be shared across all tests in a module.
    """
    # Maybe you wanted to configure it differently
    unused_port_server.start('./my_directory')

    # Maybe you wanted to add custom setup
    # ... do some setup ...

    yield unused_port_server

    # Maybe you wanted custom cleanup
    # ... do some cleanup ...


# Tests that use the custom fixture
def test_1(my_server):
    """First test."""
    assert my_server.port > 0


def test_2(my_server):
    """
    Second test - expects to use the SAME server instance as test_1
    because of module scope.
    """
    assert my_server.port > 0
```

## What Happened

You got a `ScopeMismatch` error:

```
ERROR at setup of test_1

ScopeMismatch: You tried to access the function scoped fixture unused_port_server
with a module scoped request object.

Requesting fixture stack:
your_test.py:4:  def my_server(unused_port_server)

Requested fixture:
.../pytest_unused_port/__init__.py:89:  def unused_port_server(unused_port)
```

## Why It Failed

```
Your fixture:           my_server (scope='module')
                              ↓ depends on
pytest-unused-port:     unused_port_server (scope='function')
                              ↓ depends on
pytest-unused-port:     unused_port (scope='function')

Problem: Module scope cannot depend on function scope!
```

## Visual Explanation

### What You Wanted

```
Test Module Lifecycle
├── Module Setup
│   └── my_server created (module scope)
│       └── Server starts on port 12345
├── test_1 runs
│   └── Uses my_server (port 12345)
├── test_2 runs
│   └── Uses SAME my_server (port 12345)  ← Same instance!
└── Module Teardown
    └── my_server destroyed
        └── Server stops
```

### What pytest-unused-port Provides

```
Test Module Lifecycle
├── test_1 runs
│   ├── unused_port_server created (function scope)
│   │   └── Server starts on port 12345
│   └── unused_port_server destroyed
│       └── Server stops
├── test_2 runs
│   ├── unused_port_server created AGAIN (function scope)
│   │   └── Server starts on port 54321  ← Different instance!
│   └── unused_port_server destroyed
│       └── Server stops
```

## Why You Can't Make It Work

### Attempt 1: Module Scope on Your Fixture

```python
@pytest.fixture(scope='module')  # ← Module scope
def my_server(unused_port_server):  # ← Function scope
    yield unused_port_server
```

**Result:** `ScopeMismatch` error
**Why:** Module scope fixture cannot depend on function scope fixture

### Attempt 2: Try Using Just unused_port

```python
@pytest.fixture(scope='module')  # ← Module scope
def my_server(unused_port):  # ← Also function scope!
    from pytest_unused_port import StaticServer
    server = StaticServer(unused_port)
    yield server
    server.stop()
```

**Result:** `ScopeMismatch` error
**Why:** `unused_port` is also function scoped!

### Attempt 3: No Scope (Implicit Function)

```python
@pytest.fixture  # ← Defaults to function scope
def my_server(unused_port_server):  # ← Function scope
    # Do custom stuff
    yield unused_port_server
```

**Result:** ✅ Works, but...
**Problem:** Your fixture is now function-scoped too, so you get a new server for every test (defeats the purpose if you wanted to share state)

## Workarounds You Could Use Now

### Workaround 1: Reimplement from Scratch

```python
import socket
import pytest
from pytest_unused_port import StaticServer

@pytest.fixture(scope='module')
def my_module_port():
    """Get unused port - module scoped."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()


@pytest.fixture(scope='module')
def my_server(my_module_port):
    """Module-scoped server."""
    server = StaticServer(my_module_port)
    server.start('./my_directory')
    yield server
    server.stop()
```

**Pros:** Works!
**Cons:** You're reimplementing `unused_port` and can't easily reuse pytest-unused-port's logic

### Workaround 2: Accept Function Scope

```python
@pytest.fixture  # Default function scope
def my_server(unused_port_server):
    """Function-scoped server with custom logic."""
    unused_port_server.start('./my_directory')
    # Custom setup
    yield unused_port_server
    # Custom cleanup happens via unused_port_server
```

**Pros:** Simple, works with pytest-unused-port
**Cons:** New server for every test (may be slow, can't share state)

## What pytest-unused-port Needs to Change

The library should provide fixtures with explicit scopes:

```python
# They should add these:
@pytest.fixture(scope='module')
def unused_port_module():
    """Unused port with module scope."""
    # ... implementation ...

@pytest.fixture(scope='module')
def unused_port_server_module(unused_port_module):
    """StaticServer with module scope."""
    # ... implementation ...
```

Then your code would work:

```python
@pytest.fixture(scope='module')
def my_server(unused_port_server_module):  # ← Both module scope!
    """Now both fixtures have the same scope - it works!"""
    unused_port_server_module.start('./my_directory')
    yield unused_port_server_module
```

## Testing Your Use Case

In this research project:

1. **`test_unused_port_issue.py`** - Reproduces your exact error
2. **`test_workarounds.py`** - Shows the workarounds you can use today
3. **`improved_unused_port.py`** - Shows what the library could provide
4. **`test_improved_solution.py`** - Demonstrates how it would work with the fix

## Quick Reference: Scope Rules

| Your Fixture Scope | Can Depend On | Cannot Depend On |
|-------------------|---------------|------------------|
| `function` | any scope | none |
| `class` | `class`, `module`, `session` | `function` |
| `module` | `module`, `session` | `function`, `class` |
| `session` | `session` only | `function`, `class`, `module` |

**Golden Rule:** A fixture can only depend on fixtures with the same or broader scope.

## Next Steps

1. **Short term:** Use Workaround 1 (reimplement from scratch)
2. **Medium term:** Consider opening an issue or PR on pytest-unused-port to add scope variants
3. **Long term:** Understand fixture scopes deeply to avoid this issue in future projects

See `ANALYSIS.md` for detailed technical analysis and proposed changes to the library.
