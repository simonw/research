"""
Tests for the Datasette Notes Sync Plugin.

Tests the API endpoints exposed by the plugin.
"""

import pytest
import pytest_asyncio
import json
from datasette.app import Datasette
from datasette.plugins import pm
import datasette_notes_sync


@pytest_asyncio.fixture
async def ds():
    """Create a Datasette instance with the plugin registered."""
    # Register our plugin globally with the plugin manager
    pm.register(datasette_notes_sync, name="notes_sync")
    datasette = Datasette()
    # Add a writable memory database for notes
    datasette.add_memory_database("notes")
    await datasette.invoke_startup()
    try:
        yield datasette
    finally:
        pm.unregister(name="notes_sync")


@pytest.mark.asyncio
async def test_list_notes_empty(ds):
    """Test listing notes when database is empty."""
    response = await ds.client.get("/-/notes-sync/notes")
    assert response.status_code == 200
    data = response.json()
    assert data == []


@pytest.mark.asyncio
async def test_create_note(ds):
    """Test creating a new note."""
    response = await ds.client.post(
        "/-/notes-sync/notes",
        content=json.dumps({
            "title": "Test Note",
            "content": "Hello World"
        })
    )
    assert response.status_code == 201
    data = response.json()
    assert data["title"] == "Test Note"
    assert data["content"] == "Hello World"
    assert "id" in data


@pytest.mark.asyncio
async def test_create_and_get_note(ds):
    """Test creating and retrieving a note."""
    # Create
    create_response = await ds.client.post(
        "/-/notes-sync/notes",
        content=json.dumps({
            "id": "test-note-1",
            "title": "My Note",
            "content": "Content here"
        })
    )
    assert create_response.status_code == 201

    # Get
    get_response = await ds.client.get("/-/notes-sync/notes/test-note-1")
    assert get_response.status_code == 200
    data = get_response.json()
    assert data["id"] == "test-note-1"
    assert data["title"] == "My Note"


@pytest.mark.asyncio
async def test_update_note(ds):
    """Test updating a note."""
    # Create first
    await ds.client.post(
        "/-/notes-sync/notes",
        content=json.dumps({
            "id": "update-test",
            "title": "Original",
            "content": "Original content"
        })
    )

    # Update
    response = await ds.client.request(
        method="PUT",
        path="/-/notes-sync/notes/update-test",
        content=json.dumps({
            "title": "Updated Title",
            "content": "Updated content"
        })
    )
    assert response.status_code == 200
    data = response.json()
    assert data["title"] == "Updated Title"
    assert data["content"] == "Updated content"


@pytest.mark.asyncio
async def test_delete_note(ds):
    """Test soft-deleting a note."""
    # Create first
    await ds.client.post(
        "/-/notes-sync/notes",
        content=json.dumps({
            "id": "delete-test",
            "title": "To Delete"
        })
    )

    # Delete
    response = await ds.client.request(
        method="DELETE",
        path="/-/notes-sync/notes/delete-test"
    )
    assert response.status_code == 200
    assert response.json()["success"] is True

    # Verify not in list
    list_response = await ds.client.get("/-/notes-sync/notes")
    notes = list_response.json()
    assert all(n["id"] != "delete-test" for n in notes)


@pytest.mark.asyncio
async def test_get_nonexistent_note(ds):
    """Test getting a note that doesn't exist."""
    response = await ds.client.get("/-/notes-sync/notes/nonexistent")
    assert response.status_code == 404


@pytest.mark.asyncio
async def test_sync_create_note(ds):
    """Test creating a note via sync endpoint."""
    response = await ds.client.post(
        "/-/notes-sync/sync",
        content=json.dumps({
            "client_id": "client-1",
            "operations": [
                {
                    "id": "op-1",
                    "note_id": "sync-note-1",
                    "op_type": "create",
                    "new_value": {
                        "title": "Synced Note",
                        "content": "Synced Content"
                    }
                }
            ]
        })
    )
    assert response.status_code == 200
    data = response.json()
    assert "sync_timestamp" in data
    assert "notes" in data


@pytest.mark.asyncio
async def test_sync_update_note(ds):
    """Test updating a note via sync endpoint."""
    # Create note first
    await ds.client.post(
        "/-/notes-sync/notes",
        content=json.dumps({
            "id": "sync-update-test",
            "title": "Original",
            "content": "Original"
        })
    )

    # Sync update
    response = await ds.client.post(
        "/-/notes-sync/sync",
        content=json.dumps({
            "client_id": "client-1",
            "operations": [
                {
                    "note_id": "sync-update-test",
                    "op_type": "update",
                    "field": "title",
                    "new_value": "Updated via Sync"
                }
            ]
        })
    )
    assert response.status_code == 200

    # Verify update
    get_response = await ds.client.get("/-/notes-sync/notes/sync-update-test")
    assert get_response.json()["title"] == "Updated via Sync"


@pytest.mark.asyncio
async def test_sync_returns_server_operations(ds):
    """Test that sync returns operations from other clients."""
    # First client creates a note
    await ds.client.post(
        "/-/notes-sync/sync",
        content=json.dumps({
            "client_id": "client-A",
            "operations": [
                {
                    "id": "op-A-1",
                    "note_id": "shared-note",
                    "op_type": "create",
                    "timestamp": "2024-01-01T00:00:00Z",
                    "new_value": {"title": "Shared"}
                }
            ]
        })
    )

    # Second client syncs and should see the operation
    response = await ds.client.post(
        "/-/notes-sync/sync",
        content=json.dumps({
            "client_id": "client-B",
            "last_sync": "2023-01-01T00:00:00Z",
            "operations": []
        })
    )
    assert response.status_code == 200
    data = response.json()
    # Should see client-A's operation
    assert any(op.get("client_id") == "client-A" for op in data["operations"])


@pytest.mark.asyncio
async def test_cors_headers(ds):
    """Test that CORS headers are set."""
    response = await ds.client.get("/-/notes-sync/notes")
    assert response.headers.get("access-control-allow-origin") == "*"


@pytest.mark.asyncio
async def test_options_request(ds):
    """Test OPTIONS request for CORS preflight."""
    response = await ds.client.request(method="OPTIONS", path="/-/notes-sync/notes")
    assert response.status_code == 204
    assert "access-control-allow-methods" in response.headers


@pytest.mark.asyncio
async def test_get_operations(ds):
    """Test the operations debug endpoint."""
    # Create a note via sync to generate an operation
    await ds.client.post(
        "/-/notes-sync/sync",
        content=json.dumps({
            "client_id": "test-client",
            "operations": [
                {
                    "id": "debug-op",
                    "note_id": "debug-note",
                    "op_type": "create",
                    "new_value": {"title": "Debug"}
                }
            ]
        })
    )

    response = await ds.client.get("/-/notes-sync/operations")
    assert response.status_code == 200
    ops = response.json()
    assert isinstance(ops, list)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
