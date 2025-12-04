"""
datasette-pytest: Reusable pytest fixtures for Datasette plugin development.

This package provides a comprehensive set of fixtures to make testing
Datasette plugins as pleasant and productive as possible.
"""

__version__ = "0.1.0"

from .fixtures import (
    ds,
    ds_memory,
    ds_with_db,
    fresh_ds,
    fresh_ds_memory,
    authenticated_client,
    root_client,
    db_path,
    memory_db,
    event_tracker,
    non_mocked_hosts,
)

from .helpers import (
    post_with_csrf,
    get_messages,
    assert_message,
    assert_permission_denied,
    actor_cookies,
    wait_until_responds,
)

from .plugin_helpers import (
    register_plugin,
    create_test_plugin,
    TrackEventPlugin,
)

__all__ = [
    # Fixtures
    "ds",
    "ds_memory",
    "ds_with_db",
    "fresh_ds",
    "fresh_ds_memory",
    "authenticated_client",
    "root_client",
    "db_path",
    "memory_db",
    "event_tracker",
    "non_mocked_hosts",
    # Helpers
    "post_with_csrf",
    "get_messages",
    "assert_message",
    "assert_permission_denied",
    "actor_cookies",
    "wait_until_responds",
    # Plugin helpers
    "register_plugin",
    "create_test_plugin",
    "TrackEventPlugin",
]
