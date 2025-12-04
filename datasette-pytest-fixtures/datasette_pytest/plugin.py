"""
Pytest plugin that automatically makes fixtures available.

This module is registered as a pytest plugin via the entry point
in pyproject.toml, making all fixtures automatically available
when the package is installed.
"""

import pytest
import pytest_asyncio

# Import all fixtures so they're available when the plugin is loaded
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

# Re-export fixtures so pytest can find them
__all__ = [
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
]


def pytest_configure(config):
    """
    Configure pytest markers for Datasette tests.
    """
    config.addinivalue_line(
        "markers",
        "datasette: mark test as a Datasette integration test"
    )
    config.addinivalue_line(
        "markers",
        "requires_sqlite_utils: mark test as requiring sqlite-utils"
    )


def pytest_collection_modifyitems(config, items):
    """
    Skip tests that require sqlite-utils if it's not installed.
    """
    try:
        import sqlite_utils  # noqa: F401
        sqlite_utils_available = True
    except ImportError:
        sqlite_utils_available = False

    if not sqlite_utils_available:
        skip_marker = pytest.mark.skip(reason="sqlite-utils not installed")
        for item in items:
            if "requires_sqlite_utils" in item.keywords:
                item.add_marker(skip_marker)
