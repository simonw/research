"""
Helper functions for Datasette testing.

These are standalone utilities that can be imported and used directly,
without needing to be fixtures.
"""

import time
from typing import Dict, Any, Optional, List, Tuple

import httpx


async def post_with_csrf(
    datasette,
    path: str,
    data: Dict[str, Any] = None,
    cookies: Dict[str, str] = None,
) -> "httpx.Response":
    """
    POST to a path with automatic CSRF token handling.

    This helper fetches the page first to obtain a CSRF token,
    then submits the POST request with the token included.

    Args:
        datasette: The Datasette instance
        path: The URL path to POST to
        data: Form data to submit
        cookies: Additional cookies (e.g., actor cookies)

    Returns:
        The POST response

    Usage:
        response = await post_with_csrf(
            ds,
            "/-/edit-schema/data/dogs",
            data={"drop_table": "1"},
            cookies=actor_cookies(ds, "root")
        )
    """
    data = data or {}
    cookies = cookies or {}

    # First, GET the page to obtain CSRF token
    get_response = await datasette.client.get(path, cookies=cookies)
    if "ds_csrftoken" not in get_response.cookies:
        raise ValueError(f"No CSRF token found at {path}")

    csrftoken = get_response.cookies["ds_csrftoken"]
    cookies = {**cookies, "ds_csrftoken": csrftoken}
    data = {**data, "csrftoken": csrftoken}

    return await datasette.client.post(path, data=data, cookies=cookies)


def actor_cookies(datasette, actor_id: str = None, actor: Dict[str, Any] = None) -> Dict[str, str]:
    """
    Generate authentication cookies for an actor.

    Args:
        datasette: The Datasette instance
        actor_id: Simple actor ID string (will be wrapped as {"id": actor_id})
        actor: Full actor dict

    Returns:
        Dict with the ds_actor cookie

    Usage:
        cookies = actor_cookies(ds, "root")
        response = await ds.client.get("/private", cookies=cookies)

        # Or with full actor:
        cookies = actor_cookies(ds, actor={"id": "user", "roles": ["admin"]})
    """
    if actor is None:
        if actor_id is None:
            raise ValueError("Must provide either actor_id or actor")
        actor = {"id": actor_id}

    return {
        "ds_actor": datasette.sign({"a": actor}, "actor")
    }


def get_messages(datasette, response) -> List[Tuple[str, int]]:
    """
    Extract flash messages from a response.

    Datasette stores flash messages in a signed cookie called ds_messages.
    This helper extracts and returns them.

    Args:
        datasette: The Datasette instance (needed for unsigning)
        response: The HTTP response object

    Returns:
        List of (message, level) tuples where level is one of:
        - Datasette.INFO (1)
        - Datasette.WARNING (2)
        - Datasette.ERROR (3)

    Usage:
        response = await post_with_csrf(ds, "/some/action", ...)
        messages = get_messages(ds, response)
        assert messages[0][0] == "Action completed successfully"
    """
    if "ds_messages" not in response.cookies:
        return []

    return datasette.unsign(response.cookies["ds_messages"], "messages")


def assert_message(datasette, response, expected_message: str, level: int = None):
    """
    Assert that a specific message appears in the response.

    Args:
        datasette: The Datasette instance
        response: The HTTP response object
        expected_message: The message text to look for
        level: Optional message level to match (Datasette.INFO, WARNING, or ERROR)

    Raises:
        AssertionError if the message is not found

    Usage:
        await post_with_csrf(ds, "/delete", data={"confirm": "1"})
        assert_message(ds, response, "Item deleted successfully")
    """
    messages = get_messages(datasette, response)
    message_texts = [m[0] for m in messages]

    if expected_message not in message_texts:
        raise AssertionError(
            f"Expected message '{expected_message}' not found.\n"
            f"Messages found: {messages}"
        )

    if level is not None:
        for msg_text, msg_level in messages:
            if msg_text == expected_message:
                if msg_level != level:
                    raise AssertionError(
                        f"Message '{expected_message}' found but with level {msg_level}, "
                        f"expected {level}"
                    )
                break


def assert_permission_denied(response, message: str = None):
    """
    Assert that a request was denied due to permissions.

    Args:
        response: The HTTP response object
        message: Optional specific message to look for in the response

    Raises:
        AssertionError if the response isn't a 403

    Usage:
        response = await ds.client.get("/admin/secret")
        assert_permission_denied(response)
    """
    if response.status_code != 403:
        raise AssertionError(
            f"Expected 403 Forbidden, got {response.status_code}"
        )

    if message is not None and message not in response.text:
        raise AssertionError(
            f"Expected message '{message}' not found in 403 response"
        )


def wait_until_responds(
    url: str,
    timeout: float = 5.0,
    interval: float = 0.1,
    client: httpx.Client = None,
    **kwargs
):
    """
    Wait until a URL responds successfully.

    Useful for testing against live server processes.

    Args:
        url: The URL to poll
        timeout: Maximum time to wait in seconds
        interval: Time between poll attempts
        client: Optional httpx.Client (e.g., for Unix domain sockets)
        **kwargs: Additional arguments to pass to client.get()

    Raises:
        AssertionError if the URL doesn't respond within timeout

    Usage:
        # Start a Datasette server process...
        wait_until_responds("http://localhost:8001/")
        # Now safe to make requests
    """
    if client is None:
        client = httpx

    start = time.time()
    while time.time() - start < timeout:
        try:
            client.get(url, **kwargs)
            return
        except httpx.ConnectError:
            time.sleep(interval)

    raise AssertionError(f"Timed out waiting for {url} to respond after {timeout}s")


async def wait_for_condition(
    condition_fn,
    timeout: float = 5.0,
    interval: float = 0.1,
    message: str = "Condition was not met"
):
    """
    Wait until an async condition function returns True.

    Args:
        condition_fn: Async function that returns True when condition is met
        timeout: Maximum time to wait in seconds
        interval: Time between checks
        message: Error message if timeout occurs

    Raises:
        AssertionError if condition isn't met within timeout

    Usage:
        async def job_complete():
            result = await db.execute("SELECT status FROM jobs WHERE id = ?", [job_id])
            return result.fetchone()[0] == "complete"

        await wait_for_condition(job_complete, timeout=10)
    """
    import asyncio

    start = time.time()
    while time.time() - start < timeout:
        if await condition_fn():
            return
        await asyncio.sleep(interval)

    raise AssertionError(f"{message} (timed out after {timeout}s)")


class DatabaseFixtureHelper:
    """
    Helper class for creating test databases with common patterns.

    This provides a fluent interface for building test databases.

    Usage:
        helper = DatabaseFixtureHelper(tmp_path)
        db_path = helper.create_db("test") \\
            .add_table("dogs", [{"name": "Cleo"}, {"name": "Pancakes"}]) \\
            .add_table("cats", [{"name": "Whiskers"}]) \\
            .with_fts("dogs", ["name"]) \\
            .build()

        ds = Datasette([db_path])
    """

    def __init__(self, base_path):
        """Initialize with a base path for database files."""
        self._base_path = base_path
        self._current_db = None
        self._current_db_path = None

    def create_db(self, name: str = "test"):
        """Start creating a new database."""
        try:
            import sqlite_utils
        except ImportError:
            raise ImportError("sqlite-utils is required for DatabaseFixtureHelper")

        self._current_db_path = str(self._base_path / f"{name}.db")
        self._current_db = sqlite_utils.Database(self._current_db_path)
        return self

    def add_table(
        self,
        name: str,
        records: List[Dict[str, Any]],
        pk: str = None,
        foreign_keys: List[Tuple[str, str, str]] = None,
    ):
        """
        Add a table with records.

        Args:
            name: Table name
            records: List of record dicts
            pk: Primary key column name
            foreign_keys: List of (column, other_table, other_column) tuples
        """
        if self._current_db is None:
            raise ValueError("Call create_db() first")

        self._current_db[name].insert_all(
            records,
            pk=pk,
            foreign_keys=foreign_keys,
        )
        return self

    def add_empty_table(self, name: str, columns: Dict[str, type], pk: str = None):
        """
        Create an empty table with a specific schema.

        Args:
            name: Table name
            columns: Dict mapping column names to Python types
            pk: Primary key column name
        """
        if self._current_db is None:
            raise ValueError("Call create_db() first")

        self._current_db[name].create(columns, pk=pk)
        return self

    def with_fts(self, table: str, columns: List[str]):
        """
        Add full-text search to a table.

        Args:
            table: The table to add FTS to
            columns: List of column names to index
        """
        if self._current_db is None:
            raise ValueError("Call create_db() first")

        self._current_db[table].enable_fts(columns, create_triggers=True)
        return self

    def with_index(self, table: str, columns: List[str], unique: bool = False):
        """
        Add an index to a table.

        Args:
            table: The table to add index to
            columns: List of column names
            unique: Whether the index should be unique
        """
        if self._current_db is None:
            raise ValueError("Call create_db() first")

        self._current_db[table].create_index(columns, unique=unique)
        return self

    def execute(self, sql: str, params: List = None):
        """
        Execute arbitrary SQL.

        Args:
            sql: The SQL to execute
            params: Optional parameters
        """
        if self._current_db is None:
            raise ValueError("Call create_db() first")

        if params:
            self._current_db.execute(sql, params)
        else:
            self._current_db.executescript(sql)
        return self

    def build(self) -> str:
        """
        Finish building and return the database path.

        Returns:
            The path to the created database file
        """
        if self._current_db is None:
            raise ValueError("Call create_db() first")

        path = self._current_db_path
        self._current_db = None
        self._current_db_path = None
        return path

    def build_datasette(self, **kwargs):
        """
        Finish building and return a Datasette instance.

        Args:
            **kwargs: Additional arguments to pass to Datasette constructor

        Returns:
            A configured Datasette instance
        """
        from datasette.app import Datasette

        db_path = self.build()
        ds = Datasette([db_path], **kwargs)
        ds.root_enabled = True
        return ds
