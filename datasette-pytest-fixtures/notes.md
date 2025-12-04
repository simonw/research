# Datasette Pytest Fixtures Research Notes

## Goal
Design a reusable pytest fixtures package for Datasette projects and plugins.

## Repos Cloned
- simonw/datasette
- datasette/datasette-extract
- simonw/datasette-edit-schema
- datasette/datasette-enrichments

## Investigation Steps

### Step 1: Review Datasette Plugin Testing Documentation

Key findings from `/tmp/datasette/docs/testing_plugins.rst`:

1. **Recommended testing approach**: Use `pytest` with `pytest-asyncio`
2. **Datasette client**: Use `datasette.client` for HTTP requests (wraps HTTPX)
3. **Basic test setup**:
   ```python
   from datasette.app import Datasette
   import pytest

   @pytest.mark.asyncio
   async def test_plugin():
       datasette = Datasette(memory=True)
       response = await datasette.client.get("/-/plugins.json")
   ```
4. **invoke_startup()**: Important to call if not using `datasette.client.get()` which calls it automatically
5. **Actor cookies**: `datasette.client.actor_cookie()` helper for authentication
6. **pytest-httpx**: For mocking external HTTP calls, need to return `["localhost"]` from `non_mocked_hosts()` fixture
7. **Temporary plugin registration**: Use `datasette.pm.register()/unregister()` with try/finally pattern

### Step 2: Analyze Datasette Core Tests

**Key fixture patterns from conftest.py:**

1. `ds_client` - Async fixture creating a pre-configured Datasette instance
2. `restore_working_directory` - For tests that change cwd
3. `ds_localhost_http_server` - Session-scoped HTTP server subprocess
4. `ds_unix_domain_socket_server` - UDS server for testing
5. `check_actions_are_documented` - Session-scoped autouse fixture for validation
6. `TrackEventPlugin` - Plugin for tracking events during tests
7. `wait_until_responds()` - Utility for polling server startup

**Key fixture patterns from fixtures.py:**

1. `make_app_client()` - Context manager for creating test Datasette with various configs
2. Session-scoped variants: `app_client`, `app_client_with_cors`, `app_client_base_url_prefix`, etc.
3. Comprehensive test database schema (`TABLES`, `TABLE_PARAMETERIZED_SQL`)
4. `TestClient` class wrapping Datasette's async client

### Step 3: Analyze datasette-extract Tests

**Simple fixtures in conftest.py:**
- `mock_api_key` - Autouse fixture mocking environment variables
- `vcr_config` - For VCR cassette filtering
- `mock_pricing_cache` - Pre-populating cache to avoid HTTP requests

**Test patterns:**
- Uses `Datasette()` constructor directly in tests
- Sets `ds.root_enabled = True` for root access
- Uses `ds.add_memory_database()` for in-memory databases
- Uses `ds.client.actor_cookie()` for authentication

### Step 4: Analyze datasette-edit-schema Tests

**Fixtures in conftest.py:**
- `db_and_path` - Creates a temporary SQLite database with test data using sqlite_utils
- `db_path` and `db` - Derived fixtures
- `ds` - Creates Datasette instance from db_path
- `rule` - Dataclass for testing permission rules
- `permission_plugin` - Registers a test plugin for permission testing
- `TrackEventPlugin` - Event tracking for assertions

**Test patterns:**
- `root_datasette()` helper function (not a fixture)
- CSRF token handling: fetch page -> get csrftoken -> POST with token
- Cookie management for actors: `ds.sign({"a": {"id": "root"}}, "actor")`
- Message assertions: `ds.unsign(response.cookies["ds_messages"], "messages")`

### Step 5: Analyze datasette-enrichments Tests

**Fixtures in conftest.py:**
- `datasette` - Async fixture creating test database with test tables and Datasette instance
- `load_uppercase_plugin` - Autouse fixture registering demo enrichments

**Test patterns:**
- Creates database using `sqlite3` directly
- Uses `await datasette.invoke_startup()` explicitly
- Poll-based waiting with `wait_for_job()` utility
- State tracking on datasette object: `datasette._test_db = db`

## Common Patterns Identified

### 1. Database Creation
Three approaches seen:
- `sqlite_utils.Database` (datasette-edit-schema) - Most ergonomic
- `sqlite3.connect` directly (datasette core, enrichments)
- `ds.add_memory_database()` (datasette-extract) - For in-memory databases

### 2. Datasette Instance Creation
```python
ds = Datasette([db_path], config={...}, metadata={...})
ds.root_enabled = True  # Common pattern for enabling root
await ds.invoke_startup()  # If not using client.get() first
```

### 3. Authentication
```python
cookies = {"ds_actor": ds.sign({"a": {"id": "root"}}, "actor")}
# Or using helper:
cookies = {"ds_actor": ds.client.actor_cookie({"id": "root"})}
```

### 4. CSRF Token Handling
```python
response = await ds.client.get("/some/path", cookies=cookies)
csrftoken = response.cookies["ds_csrftoken"]
cookies["ds_csrftoken"] = csrftoken
post_response = await ds.client.post(
    "/some/path",
    data={"csrftoken": csrftoken, ...},
    cookies=cookies,
)
```

### 5. Event/Action Tracking
Custom plugins registered to track events:
```python
class TrackEventPlugin:
    __name__ = "TrackEventPlugin"

    @hookimpl
    def track_event(self, datasette, event):
        datasette._tracked_events = getattr(datasette, "_tracked_events", [])
        datasette._tracked_events.append(event)
```

### 6. Plugin Registration for Tests
```python
datasette.pm.register(TestPlugin(), name="undo")
try:
    # test code
finally:
    datasette.pm.unregister(name="undo")
```

### 7. Fixture Scoping
- `scope="session"` for expensive fixtures (database setup, server processes)
- No scope (function) for tests that modify data
- `autouse=True` for fixtures needed by all tests

## Missing/Improvable Areas

1. **No standard way to create test databases** - Each plugin rolls its own
2. **No reusable CSRF helper** - Boilerplate in every test
3. **No standard authenticated client** - Actor cookie setup is manual
4. **No standard event tracking** - Each plugin defines its own TrackEventPlugin
5. **No reusable assertion helpers** - e.g., checking messages, permissions
6. **No memory database fixture** - Common use case not standardized
7. **No table factory** - Easy table creation for tests

## Package Design

Based on the analysis, I designed `datasette-pytest` with the following components:

### Core Fixtures (fixtures.py)
- `ds` / `ds_memory` - Simple in-memory Datasette instances
- `fresh_ds` / `fresh_ds_memory` - Factory fixtures for custom configs
- `db_path` - Temporary database path
- `memory_db` - Factory for named in-memory databases
- `ds_with_db` - Factory for pre-populated databases
- `authenticated_client` / `root_client` - Auth clients with CSRF support
- `event_tracker` - Event tracking fixture
- `non_mocked_hosts` - pytest-httpx compatibility

### Helper Functions (helpers.py)
- `post_with_csrf()` - CSRF-aware POST
- `actor_cookies()` - Generate auth cookies
- `get_messages()` / `assert_message()` - Flash message helpers
- `assert_permission_denied()` - Permission assertion
- `wait_until_responds()` - Server polling
- `wait_for_condition()` - Async condition waiting
- `DatabaseFixtureHelper` - Fluent database builder

### Plugin Helpers (plugin_helpers.py)
- `TrackEventPlugin` - Reusable event tracker
- `register_plugin()` - Context manager for temp plugins
- `create_test_plugin()` - Plugin factory
- `PermissionMocker` - Permission mocking
- `create_route_handler()` - Simple route handler factory

### Key Design Decisions
1. **Fixture scoping**: Function-scoped by default for isolation
2. **Optional sqlite-utils**: Works without it, enhanced with it
3. **Async-first**: All HTTP operations are async
4. **Root enabled**: Simplifies authenticated testing
5. **CSRF built-in**: Eliminates repetitive patterns
6. **Pytest plugin**: Auto-registers via entry point

