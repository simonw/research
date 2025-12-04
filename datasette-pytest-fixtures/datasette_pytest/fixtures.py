"""
Core pytest fixtures for Datasette testing.

These fixtures provide easy-to-use, pre-configured Datasette instances
for common testing scenarios.
"""

import pytest
import pytest_asyncio
import tempfile
import os
from pathlib import Path
from typing import Callable, Dict, Any, Optional, List, Union

try:
    import sqlite_utils
    SQLITE_UTILS_AVAILABLE = True
except ImportError:
    SQLITE_UTILS_AVAILABLE = False


def _create_datasette(
    files: Optional[List[str]] = None,
    memory: bool = False,
    config: Optional[Dict[str, Any]] = None,
    metadata: Optional[Dict[str, Any]] = None,
    settings: Optional[Dict[str, Any]] = None,
    root_enabled: bool = True,
    **kwargs
):
    """
    Internal helper to create a configured Datasette instance.
    """
    from datasette.app import Datasette

    ds = Datasette(
        files=files or [],
        memory=memory,
        config=config,
        metadata=metadata,
        settings=settings,
        **kwargs
    )
    if root_enabled:
        ds.root_enabled = True
    return ds


@pytest.fixture
def ds():
    """
    A simple in-memory Datasette instance with root enabled.

    This is the most basic fixture - gives you a clean Datasette
    with no databases attached (except _memory).

    Usage:
        async def test_something(ds):
            response = await ds.client.get("/-/plugins.json")
            assert response.status_code == 200
    """
    return _create_datasette(memory=True)


@pytest.fixture
def ds_memory():
    """
    Alias for `ds` - a simple in-memory Datasette instance.

    Provided for clarity when you specifically want an in-memory instance.
    """
    return _create_datasette(memory=True)


@pytest.fixture
def fresh_ds():
    """
    Factory fixture that creates a new Datasette instance on each call.

    Useful when you need multiple instances or custom configurations.

    Usage:
        async def test_something(fresh_ds):
            ds1 = fresh_ds()
            ds2 = fresh_ds(config={"plugins": {"my-plugin": {"option": "value"}}})
    """
    def factory(
        files: Optional[List[str]] = None,
        memory: bool = True,
        config: Optional[Dict[str, Any]] = None,
        metadata: Optional[Dict[str, Any]] = None,
        settings: Optional[Dict[str, Any]] = None,
        root_enabled: bool = True,
        **kwargs
    ):
        return _create_datasette(
            files=files,
            memory=memory,
            config=config,
            metadata=metadata,
            settings=settings,
            root_enabled=root_enabled,
            **kwargs
        )
    return factory


@pytest.fixture
def fresh_ds_memory():
    """
    Factory fixture that creates a new in-memory Datasette instance.

    Like fresh_ds but always creates memory-based instances.
    """
    def factory(
        config: Optional[Dict[str, Any]] = None,
        metadata: Optional[Dict[str, Any]] = None,
        settings: Optional[Dict[str, Any]] = None,
        root_enabled: bool = True,
        **kwargs
    ):
        return _create_datasette(
            memory=True,
            config=config,
            metadata=metadata,
            settings=settings,
            root_enabled=root_enabled,
            **kwargs
        )
    return factory


@pytest.fixture
def db_path(tmp_path):
    """
    Provides a path to a temporary SQLite database file.

    The database file is created but empty - use sqlite_utils or
    sqlite3 to populate it.

    Usage:
        def test_with_db(db_path):
            import sqlite_utils
            db = sqlite_utils.Database(db_path)
            db["dogs"].insert({"name": "Cleo"})
    """
    return str(tmp_path / "test.db")


@pytest.fixture
def memory_db():
    """
    Factory fixture that creates named in-memory databases on a Datasette instance.

    Usage:
        async def test_with_memory_db(ds, memory_db):
            db = memory_db(ds, "mydata")
            await db.execute_write("CREATE TABLE dogs (name TEXT)")
            await db.execute_write("INSERT INTO dogs VALUES ('Cleo')")
            response = await ds.client.get("/mydata/dogs.json")
    """
    def factory(datasette, name: str = "data"):
        """
        Add a named in-memory database to a Datasette instance.

        Args:
            datasette: The Datasette instance
            name: The name for the database (default: "data")

        Returns:
            The Database object
        """
        return datasette.add_memory_database(name)
    return factory


@pytest.fixture
def ds_with_db(tmp_path):
    """
    Factory fixture that creates a Datasette instance with a pre-populated database.

    Accepts either:
    - A dict of table_name -> list of records
    - A callable that receives a sqlite_utils.Database and populates it

    Usage:
        async def test_with_data(ds_with_db):
            ds = ds_with_db({
                "dogs": [
                    {"id": 1, "name": "Cleo"},
                    {"id": 2, "name": "Pancakes"},
                ],
                "cats": [
                    {"id": 1, "name": "Whiskers"},
                ]
            })
            response = await ds.client.get("/test/dogs.json")

    With custom setup:
        async def test_custom(ds_with_db):
            def setup(db):
                db["dogs"].insert({"name": "Cleo"}, pk="id")
                db["dogs"].create_index(["name"])
            ds = ds_with_db(setup)
    """
    if not SQLITE_UTILS_AVAILABLE:
        pytest.skip("sqlite-utils is required for ds_with_db fixture")

    def factory(
        tables_or_setup: Union[Dict[str, List[Dict]], Callable],
        db_name: str = "test",
        config: Optional[Dict[str, Any]] = None,
        metadata: Optional[Dict[str, Any]] = None,
        settings: Optional[Dict[str, Any]] = None,
        root_enabled: bool = True,
    ):
        db_path = str(tmp_path / f"{db_name}.db")
        db = sqlite_utils.Database(db_path)

        if callable(tables_or_setup):
            tables_or_setup(db)
        else:
            for table_name, records in tables_or_setup.items():
                if records:
                    db[table_name].insert_all(records)

        return _create_datasette(
            files=[db_path],
            config=config,
            metadata=metadata,
            settings=settings,
            root_enabled=root_enabled,
        )

    return factory


@pytest_asyncio.fixture
async def authenticated_client():
    """
    Factory fixture that returns a helper for making authenticated requests.

    Usage:
        async def test_auth(ds, authenticated_client):
            client = authenticated_client(ds, actor_id="user123")
            response = await client.get("/data/private_table")

    Or with full actor dict:
        async def test_auth_full(ds, authenticated_client):
            client = authenticated_client(ds, actor={"id": "admin", "role": "admin"})
            response = await client.get("/admin/settings")
    """
    class AuthenticatedClient:
        def __init__(self, datasette, actor: Dict[str, Any]):
            self.datasette = datasette
            self.actor = actor
            self._cookies = {
                "ds_actor": datasette.sign({"a": actor}, "actor")
            }

        async def get(self, path: str, **kwargs):
            cookies = {**self._cookies, **kwargs.pop("cookies", {})}
            return await self.datasette.client.get(path, cookies=cookies, **kwargs)

        async def post(self, path: str, **kwargs):
            cookies = {**self._cookies, **kwargs.pop("cookies", {})}
            return await self.datasette.client.post(path, cookies=cookies, **kwargs)

        async def post_with_csrf(self, path: str, data: Dict[str, Any] = None, **kwargs):
            """
            POST with automatic CSRF token handling.

            Fetches the page first to get a CSRF token, then submits with the token.
            """
            data = data or {}

            # Get CSRF token
            get_response = await self.get(path)
            if "ds_csrftoken" not in get_response.cookies:
                raise ValueError(f"No CSRF token found at {path}")

            csrftoken = get_response.cookies["ds_csrftoken"]
            self._cookies["ds_csrftoken"] = csrftoken
            data["csrftoken"] = csrftoken

            return await self.post(path, data=data, **kwargs)

        @property
        def cookies(self):
            return self._cookies.copy()

    def factory(
        datasette,
        actor_id: Optional[str] = None,
        actor: Optional[Dict[str, Any]] = None,
    ) -> AuthenticatedClient:
        if actor is None:
            if actor_id is None:
                raise ValueError("Must provide either actor_id or actor")
            actor = {"id": actor_id}
        return AuthenticatedClient(datasette, actor)

    return factory


@pytest_asyncio.fixture
async def root_client(authenticated_client):
    """
    Factory fixture that returns a client authenticated as root.

    Usage:
        async def test_admin(ds, root_client):
            client = root_client(ds)
            response = await client.get("/-/edit-schema/data")
    """
    def factory(datasette):
        return authenticated_client(datasette, actor_id="root")
    return factory


@pytest.fixture
def event_tracker():
    """
    Factory fixture that sets up event tracking on a Datasette instance.

    Returns a tracker object that can be queried for events.

    Usage:
        async def test_events(ds, event_tracker):
            tracker = event_tracker(ds)

            # ... perform actions ...

            events = tracker.events
            assert any(e.name == "insert-row" for e in events)

            last = tracker.last_event
            assert last.name == "insert-row"
    """
    from datasette import hookimpl
    from datasette.plugins import pm

    class EventTracker:
        def __init__(self, datasette):
            self.datasette = datasette
            self._events = []
            self._plugin_name = f"event_tracker_{id(self)}"
            self._registered = False

        def _register(self):
            if self._registered:
                return

            tracker = self

            class TrackerPlugin:
                __name__ = tracker._plugin_name

                @hookimpl
                def track_event(self, datasette, event):
                    tracker._events.append(event)

            pm.register(TrackerPlugin(), name=self._plugin_name)
            self._registered = True

        def _unregister(self):
            if self._registered:
                pm.unregister(name=self._plugin_name)
                self._registered = False

        @property
        def events(self):
            return list(self._events)

        @property
        def last_event(self):
            return self._events[-1] if self._events else None

        def clear(self):
            self._events.clear()

        def filter_by_name(self, name: str):
            return [e for e in self._events if e.name == name]

    trackers = []

    def factory(datasette) -> EventTracker:
        tracker = EventTracker(datasette)
        tracker._register()
        trackers.append(tracker)
        return tracker

    yield factory

    # Cleanup
    for tracker in trackers:
        tracker._unregister()


@pytest.fixture
def non_mocked_hosts():
    """
    pytest-httpx fixture to prevent mocking of localhost requests.

    Required when using pytest-httpx with Datasette tests, since
    Datasette's internal client uses httpx for requests.

    Usage:
        @pytest.fixture
        def non_mocked_hosts():
            return ["localhost"]

        async def test_external_api(ds, httpx_mock, non_mocked_hosts):
            httpx_mock.add_response(url="https://api.example.com/", json={"ok": True})
            # Datasette requests to localhost still work normally
            response = await ds.client.get("/")
    """
    return ["localhost"]
