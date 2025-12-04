# datasette-pytest: Reusable Pytest Fixtures for Datasette Plugin Development

A comprehensive pytest fixtures package designed to make writing tests for Datasette projects and plugins as pleasant and productive as possible.

## Research Summary

This package was designed after analyzing testing patterns across multiple Datasette projects:

- **datasette** (core) - Complex fixture setup in `conftest.py` and `fixtures.py`
- **datasette-extract** - Simple fixtures with environment mocking
- **datasette-edit-schema** - Database fixtures using sqlite-utils
- **datasette-enrichments** - Async fixtures with plugin registration patterns

### Common Pain Points Identified

1. **Database setup boilerplate** - Each plugin creates test databases differently
2. **CSRF token handling** - Manual GET-then-POST pattern repeated everywhere
3. **Authentication setup** - Actor cookie generation is verbose
4. **Event tracking** - Every plugin defines its own TrackEventPlugin
5. **Plugin registration** - try/finally patterns for temporary plugins
6. **No standardized patterns** - Inconsistent approaches across projects

## Installation

```bash
pip install datasette-pytest
```

For database fixture support with sqlite-utils:
```bash
pip install datasette-pytest[sqlite-utils]
```

## Quick Start

```python
import pytest

@pytest.mark.asyncio
async def test_my_plugin(ds):
    """The ds fixture provides a ready-to-use Datasette instance."""
    response = await ds.client.get("/-/plugins.json")
    assert response.status_code == 200
```

## Available Fixtures

### Basic Fixtures

#### `ds`
A simple in-memory Datasette instance with `root_enabled=True`.

```python
@pytest.mark.asyncio
async def test_basic(ds):
    response = await ds.client.get("/")
    assert response.status_code == 200
```

#### `ds_memory`
Alias for `ds` - for clarity when you specifically want memory mode.

#### `fresh_ds`
Factory fixture for creating new Datasette instances with custom configuration.

```python
@pytest.mark.asyncio
async def test_custom_settings(fresh_ds):
    ds = fresh_ds(
        settings={"max_returned_rows": 10},
        config={"plugins": {"my-plugin": {"option": "value"}}}
    )
    response = await ds.client.get("/-/settings.json")
    assert response.json()["max_returned_rows"] == 10
```

### Database Fixtures

#### `db_path`
Provides a path to a temporary SQLite database file.

```python
def test_with_db_path(db_path):
    import sqlite_utils
    db = sqlite_utils.Database(db_path)
    db["dogs"].insert({"name": "Cleo"})
```

#### `memory_db`
Factory for adding named in-memory databases to a Datasette instance.

```python
@pytest.mark.asyncio
async def test_memory_db(ds, memory_db):
    db = memory_db(ds, "mydata")
    await db.execute_write("CREATE TABLE dogs (name TEXT)")
    await db.execute_write("INSERT INTO dogs VALUES ('Cleo')")
    response = await ds.client.get("/mydata/dogs.json")
```

#### `ds_with_db`
Factory for creating Datasette with pre-populated databases.

```python
@pytest.mark.asyncio
async def test_prefilled(ds_with_db):
    # Simple dict format
    ds = ds_with_db({
        "dogs": [{"id": 1, "name": "Cleo"}],
        "cats": [{"id": 1, "name": "Whiskers"}]
    })

    # Or with a setup function for complex schemas
    def setup(db):
        db["dogs"].insert({"name": "Cleo"}, pk="id")
        db["dogs"].enable_fts(["name"])

    ds = ds_with_db(setup)
```

### Authentication Fixtures

#### `authenticated_client`
Factory for creating clients with actor authentication.

```python
@pytest.mark.asyncio
async def test_auth(ds, authenticated_client):
    client = authenticated_client(ds, actor_id="alice")
    response = await client.get("/-/actor.json")
    assert response.json()["actor"]["id"] == "alice"

    # With full actor dict
    client = authenticated_client(ds, actor={
        "id": "alice",
        "roles": ["editor"]
    })
```

#### `root_client`
Shortcut for root-authenticated client.

```python
@pytest.mark.asyncio
async def test_admin(ds, root_client):
    client = root_client(ds)
    # CSRF-protected POST
    response = await client.post_with_csrf(
        "/-/edit-schema/data/dogs",
        data={"drop_table": "1"}
    )
```

### Plugin Testing Fixtures

#### `event_tracker`
Track events fired by Datasette during tests.

```python
@pytest.mark.asyncio
async def test_events(ds, event_tracker):
    tracker = event_tracker(ds)
    # ... perform actions ...
    assert any(e.name == "insert-row" for e in tracker.events)
    assert tracker.last_event.name == "expected-event"
```

#### `non_mocked_hosts`
For pytest-httpx compatibility - prevents mocking localhost.

```python
@pytest.fixture
def non_mocked_hosts():
    return ["localhost"]
```

## Helper Functions

### `post_with_csrf(datasette, path, data, cookies)`
POST with automatic CSRF token handling.

```python
from datasette_pytest import post_with_csrf, actor_cookies

response = await post_with_csrf(
    ds,
    "/edit-schema/data/dogs",
    data={"drop_table": "1"},
    cookies=actor_cookies(ds, "root")
)
```

### `actor_cookies(datasette, actor_id=None, actor=None)`
Generate authentication cookies.

```python
cookies = actor_cookies(ds, "root")
cookies = actor_cookies(ds, actor={"id": "user", "roles": ["admin"]})
```

### `get_messages(datasette, response)`
Extract flash messages from a response.

```python
messages = get_messages(ds, response)
assert messages[0][0] == "Table deleted"
```

### `assert_message(datasette, response, message, level=None)`
Assert a specific message appears in the response.

```python
assert_message(ds, response, "Changes saved")
assert_message(ds, response, "Error!", level=Datasette.ERROR)
```

### `assert_permission_denied(response)`
Assert a request was denied (403).

```python
response = await ds.client.get("/admin/secret")
assert_permission_denied(response)
```

## Plugin Testing Helpers

### `register_plugin(datasette, plugin, name=None)`
Context manager for temporary plugin registration.

```python
from datasette_pytest import register_plugin

class MyTestPlugin:
    __name__ = "MyTestPlugin"

    @hookimpl
    def register_routes(self):
        return [(r"^/test$", my_handler)]

with register_plugin(ds, MyTestPlugin()):
    response = await ds.client.get("/test")
```

### `create_test_plugin(...)`
Factory for creating test plugins without boilerplate.

```python
from datasette_pytest import create_test_plugin, create_route_handler

plugin = create_test_plugin(
    routes=[
        (r"^/-/api$", create_route_handler(response_json={"ok": True}))
    ],
    menu_links=[
        {"href": "/-/api", "label": "API"}
    ]
)
```

### `PermissionMocker`
Fluent API for mocking permissions.

```python
from datasette_pytest import PermissionMocker

mocker = PermissionMocker()
mocker.allow("edit-schema", database="mydb", actor_id="alice")
mocker.deny("drop-table")

with register_plugin(ds, mocker.as_plugin()):
    # Permissions are now mocked
    pass
```

### `DatabaseFixtureHelper`
Fluent API for building test databases.

```python
from datasette_pytest import DatabaseFixtureHelper

helper = DatabaseFixtureHelper(tmp_path)
ds = helper.create_db("test") \
    .add_table("dogs", [{"name": "Cleo"}]) \
    .with_fts("dogs", ["name"]) \
    .with_index("dogs", ["name"]) \
    .build_datasette()
```

## Example Test Files

See the `examples/` directory for complete working examples:

- `test_basic_usage.py` - Basic fixture usage
- `test_authentication.py` - Auth and permission testing
- `test_plugins.py` - Plugin testing patterns

## Design Decisions

### Fixture Scope
- Most fixtures are function-scoped for test isolation
- `session` scope available via factory fixtures when needed

### sqlite-utils Optional
- Database fixtures work with or without sqlite-utils
- Enhanced features available when sqlite-utils is installed

### Async-First
- All HTTP fixtures support async/await
- Uses pytest-asyncio for test execution

### Root Enabled by Default
- All Datasette fixtures have `root_enabled=True`
- Simplifies testing authenticated features

### CSRF Handling Built-In
- Authenticated clients have `post_with_csrf()` method
- Eliminates boilerplate GET-then-POST patterns

## Package Structure

```
datasette_pytest/
├── __init__.py      # Public API exports
├── fixtures.py      # Core pytest fixtures
├── helpers.py       # Standalone helper functions
├── plugin_helpers.py # Plugin testing utilities
└── plugin.py        # pytest plugin registration
```

## Comparison with Current Patterns

### Before (typical plugin test)
```python
@pytest.fixture
def db_path(tmpdir):
    path = str(tmpdir / "test.db")
    db = sqlite_utils.Database(path)
    db["dogs"].insert_all([{"name": "Cleo"}])
    return path

@pytest.fixture
def ds(db_path):
    ds = Datasette([db_path])
    ds.root_enabled = True
    return ds

@pytest.mark.asyncio
async def test_drop_table(ds):
    cookies = {"ds_actor": ds.sign({"a": {"id": "root"}}, "actor")}
    get_response = await ds.client.get(
        "/-/edit-schema/data/dogs",
        cookies=cookies
    )
    csrftoken = get_response.cookies["ds_csrftoken"]
    cookies["ds_csrftoken"] = csrftoken
    response = await ds.client.post(
        "/-/edit-schema/data/dogs",
        data={"drop_table": "1", "csrftoken": csrftoken},
        cookies=cookies
    )
    assert response.status_code == 302
```

### After (with datasette-pytest)
```python
@pytest.mark.asyncio
async def test_drop_table(ds_with_db, root_client):
    ds = ds_with_db({"dogs": [{"name": "Cleo"}]})
    client = root_client(ds)
    response = await client.post_with_csrf(
        "/-/edit-schema/test/dogs",
        data={"drop_table": "1"}
    )
    assert response.status_code == 302
```

## Future Enhancements

Potential additions based on observed patterns:

1. **VCR integration** - Fixtures for recording/replaying HTTP interactions
2. **Snapshot testing** - HTML/JSON snapshot comparisons
3. **Table assertions** - Direct database state assertions
4. **Load testing utilities** - Concurrent request helpers
5. **CLI testing** - Helpers for testing CLI commands

## License

Apache 2.0
