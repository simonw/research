# Quick Summary: Pytest Scopes and pytest-unused-port

## The Problem

When you try to create a fixture with broader scope (e.g., `module`) that depends on `unused_port_server` from pytest-unused-port, you get:

```
ScopeMismatch: You tried to access the function scoped fixture unused_port_server
with a module scoped request object.
```

## Why It Happens

- `unused_port_server` has **function scope** (default)
- Your fixture has **module scope** (broader)
- **Rule:** Broader-scoped fixtures cannot depend on narrower-scoped fixtures

## Pytest Scope Hierarchy (Narrowest to Broadest)

```
function → class → module → package → session
```

A fixture can only depend on fixtures with **equal or broader** scope.

## What You Can Do Right Now

### Option 1: Reimplement (Recommended)

```python
import socket
import pytest
from pytest_unused_port import StaticServer

@pytest.fixture(scope='module')
def my_module_port():
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        return port
    finally:
        sock.close()

@pytest.fixture(scope='module')
def my_module_server(my_module_port):
    server = StaticServer(my_module_port)
    yield server
    server.stop()
```

### Option 2: Accept Function Scope

```python
@pytest.fixture  # Defaults to function scope
def my_server(unused_port_server):
    # Custom logic here
    yield unused_port_server
```

**Downside:** New server instance for every test

## What pytest-unused-port Should Add

The library should provide scope-specific fixtures:

```python
# Module scope
unused_port_module
unused_port_server_module

# Session scope
unused_port_session
unused_port_server_session

# Factory pattern (advanced)
unused_port_factory()
static_server_factory()
```

## Files in This Research Project

| File | Purpose |
|------|---------|
| `README.md` | Full documentation |
| `SUMMARY.md` | This quick reference |
| `ANALYSIS.md` | Deep technical analysis |
| `REAL_WORLD_EXAMPLE.md` | Your specific use case |
| `test_scopes_basic.py` | Learn pytest scopes |
| `test_unused_port_issue.py` | See the error in action |
| `test_workarounds.py` | Solutions you can use now |
| `improved_unused_port.py` | Proposed library improvements |
| `test_improved_solution.py` | How improvements would work |

## Running the Examples

```bash
# See pytest scopes in action
pytest test_scopes_basic.py -v -s

# See the ScopeMismatch error
pytest test_unused_port_issue.py -v

# Test current workarounds
pytest test_workarounds.py -v

# Test proposed improvements
pytest test_improved_solution.py -v
```

## Key Takeaway

The issue is **fixture scope compatibility**. The current library only provides function-scoped fixtures. To build on them with broader scopes, the library needs to provide scope-specific variants or factory fixtures.

## Recommended Action

1. **For your immediate need:** Use Option 1 (reimplement) from above
2. **For the community:** Consider opening an issue/PR on pytest-unused-port suggesting scope variants
3. **For your learning:** Study `test_scopes_basic.py` to master pytest scopes

---

For complete details, see `README.md` and `ANALYSIS.md`.
