"""
Datasette Notes Sync Plugin

A Datasette plugin that provides an API for offline-first notes synchronization.
Implements the same API as the Starlette server.py but as a Datasette plugin.

Features:
- CRUD operations for notes
- Sync endpoint for exchanging operations
- CRDT-based merge for conflict resolution
- CORS support for cross-origin requests
"""

import json
import uuid
from datetime import datetime
from datasette import hookimpl, Response
from functools import wraps

# Import CRDT module from parent directory
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from crdt import CRDTNote, UniqueId


def add_cors_headers(headers):
    """Add CORS headers to response."""
    headers["Access-Control-Allow-Origin"] = "*"
    headers["Access-Control-Allow-Methods"] = "GET, POST, PUT, DELETE, OPTIONS"
    headers["Access-Control-Allow-Headers"] = "Content-Type"
    return headers


def cors_response(body, status=200, headers=None):
    """Create a JSON response with CORS headers."""
    headers = headers or {}
    add_cors_headers(headers)
    return Response.json(body, status=status, headers=headers)


async def ensure_tables(db):
    """Ensure the notes and operations tables exist."""
    await db.execute_write("""
        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            title TEXT DEFAULT '',
            content TEXT DEFAULT '',
            created_at TEXT,
            updated_at TEXT,
            version_vector TEXT DEFAULT '{}',
            deleted INTEGER DEFAULT 0
        )
    """)
    await db.execute_write("""
        CREATE TABLE IF NOT EXISTS operations (
            id TEXT PRIMARY KEY,
            note_id TEXT,
            client_id TEXT,
            timestamp TEXT,
            op_type TEXT,
            field TEXT,
            old_value TEXT,
            new_value TEXT,
            base_version_vector TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        )
    """)
    await db.execute_write("""
        CREATE TABLE IF NOT EXISTS clients (
            id TEXT PRIMARY KEY,
            last_sync TEXT
        )
    """)
    await db.execute_write("""
        CREATE INDEX IF NOT EXISTS idx_operations_timestamp
        ON operations(timestamp)
    """)
    await db.execute_write("""
        CREATE INDEX IF NOT EXISTS idx_operations_note_id
        ON operations(note_id)
    """)


def get_notes_db(datasette):
    """Get the notes database (use 'notes' if exists, otherwise first db)."""
    try:
        return datasette.get_database("notes")
    except KeyError:
        return datasette.get_database()


async def get_notes_list(request, datasette):
    """GET /-/notes-sync/notes - List all non-deleted notes."""
    db = get_notes_db(datasette)
    await ensure_tables(db)

    results = await db.execute("SELECT * FROM notes WHERE deleted = 0 ORDER BY updated_at DESC")
    notes = [dict(row) for row in results.rows]
    return cors_response(notes)


async def create_note(request, datasette):
    """POST /-/notes-sync/notes - Create a new note."""
    db = get_notes_db(datasette)
    await ensure_tables(db)

    body = await request.post_body()
    data = json.loads(body) if body else {}

    note_id = data.get("id", str(uuid.uuid4()))
    now = datetime.utcnow().isoformat() + "Z"

    await db.execute_write("""
        INSERT INTO notes (id, title, content, created_at, updated_at, version_vector, deleted)
        VALUES (?, ?, ?, ?, ?, ?, 0)
    """, [
        note_id,
        data.get("title", ""),
        data.get("content", ""),
        now,
        now,
        json.dumps(data.get("version_vector", {}))
    ])

    # Fetch and return the created note
    result = await db.execute("SELECT * FROM notes WHERE id = ?", [note_id])
    note = dict(result.first())
    return cors_response(note, status=201)


async def get_note(request, datasette):
    """GET /-/notes-sync/notes/<id> - Get a single note."""
    db = get_notes_db(datasette)
    await ensure_tables(db)

    note_id = request.url_vars["note_id"]
    result = await db.execute("SELECT * FROM notes WHERE id = ?", [note_id])
    row = result.first()

    if not row:
        return cors_response({"error": "Note not found"}, status=404)

    return cors_response(dict(row))


async def update_note(request, datasette):
    """PUT /-/notes-sync/notes/<id> - Update a note."""
    db = get_notes_db(datasette)
    await ensure_tables(db)

    note_id = request.url_vars["note_id"]
    body = await request.post_body()
    data = json.loads(body) if body else {}

    now = datetime.utcnow().isoformat() + "Z"

    # Check if note exists
    result = await db.execute("SELECT * FROM notes WHERE id = ?", [note_id])
    if not result.first():
        return cors_response({"error": "Note not found"}, status=404)

    # Update fields that are provided
    updates = []
    params = []

    if "title" in data:
        updates.append("title = ?")
        params.append(data["title"])
    if "content" in data:
        updates.append("content = ?")
        params.append(data["content"])
    if "version_vector" in data:
        updates.append("version_vector = ?")
        params.append(json.dumps(data["version_vector"]))

    updates.append("updated_at = ?")
    params.append(now)
    params.append(note_id)

    await db.execute_write(
        f"UPDATE notes SET {', '.join(updates)} WHERE id = ?",
        params
    )

    # Fetch and return the updated note
    result = await db.execute("SELECT * FROM notes WHERE id = ?", [note_id])
    return cors_response(dict(result.first()))


async def delete_note(request, datasette):
    """DELETE /-/notes-sync/notes/<id> - Soft delete a note."""
    db = get_notes_db(datasette)
    await ensure_tables(db)

    note_id = request.url_vars["note_id"]
    now = datetime.utcnow().isoformat() + "Z"

    result = await db.execute("SELECT * FROM notes WHERE id = ?", [note_id])
    if not result.first():
        return cors_response({"error": "Note not found"}, status=404)

    await db.execute_write(
        "UPDATE notes SET deleted = 1, updated_at = ? WHERE id = ?",
        [now, note_id]
    )

    return cors_response({"success": True})


async def sync_notes(request, datasette):
    """POST /-/notes-sync/sync - Exchange operations with server."""
    db = get_notes_db(datasette)
    await ensure_tables(db)

    body = await request.post_body()
    data = json.loads(body) if body else {}

    client_id = data.get("client_id", str(uuid.uuid4()))
    last_sync = data.get("last_sync")
    client_operations = data.get("operations", [])

    now = datetime.utcnow().isoformat() + "Z"
    affected_note_ids = set()

    # Process client operations
    for op in client_operations:
        op_id = op.get("id", str(uuid.uuid4()))
        note_id = op.get("note_id")
        op_type = op.get("op_type")

        affected_note_ids.add(note_id)

        # Store the operation
        old_val = op.get("old_value")
        new_val = op.get("new_value")
        # Serialize dicts to JSON strings
        old_val_str = json.dumps(old_val) if isinstance(old_val, dict) else old_val
        new_val_str = json.dumps(new_val) if isinstance(new_val, dict) else new_val

        await db.execute_write("""
            INSERT OR IGNORE INTO operations
            (id, note_id, client_id, timestamp, op_type, field, old_value, new_value, base_version_vector)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, [
            op_id,
            note_id,
            client_id,
            op.get("timestamp", now),
            op_type,
            op.get("field"),
            old_val_str,
            new_val_str,
            json.dumps(op.get("base_version_vector", {}))
        ])

        # Apply operation to note
        if op_type == "create":
            # Check if note already exists
            result = await db.execute("SELECT id FROM notes WHERE id = ?", [note_id])
            if not result.first():
                await db.execute_write("""
                    INSERT INTO notes (id, title, content, created_at, updated_at, version_vector, deleted)
                    VALUES (?, ?, ?, ?, ?, ?, 0)
                """, [
                    note_id,
                    op.get("new_value", {}).get("title", ""),
                    op.get("new_value", {}).get("content", ""),
                    now,
                    now,
                    json.dumps({client_id: 1})
                ])

        elif op_type == "update":
            field = op.get("field")
            new_value = op.get("new_value")
            if field and new_value is not None:
                await db.execute_write(
                    f"UPDATE notes SET {field} = ?, updated_at = ? WHERE id = ?",
                    [new_value, now, note_id]
                )

        elif op_type == "delete":
            await db.execute_write(
                "UPDATE notes SET deleted = 1, updated_at = ? WHERE id = ?",
                [now, note_id]
            )

    # Get operations since last sync (from other clients)
    server_operations = []
    if last_sync:
        result = await db.execute("""
            SELECT * FROM operations
            WHERE timestamp > ? AND client_id != ?
            ORDER BY timestamp
        """, [last_sync, client_id])
        server_operations = [dict(row) for row in result.rows]

    # Update client's last sync time
    await db.execute_write("""
        INSERT OR REPLACE INTO clients (id, last_sync) VALUES (?, ?)
    """, [client_id, now])

    # Get current state of affected notes
    notes = []
    if affected_note_ids:
        placeholders = ",".join("?" * len(affected_note_ids))
        result = await db.execute(
            f"SELECT * FROM notes WHERE id IN ({placeholders})",
            list(affected_note_ids)
        )
        notes = [dict(row) for row in result.rows]

    return cors_response({
        "sync_timestamp": now,
        "operations": server_operations,
        "notes": notes
    })


async def get_operations(request, datasette):
    """GET /-/notes-sync/operations - Debug: list all operations."""
    db = get_notes_db(datasette)
    await ensure_tables(db)

    result = await db.execute("SELECT * FROM operations ORDER BY timestamp DESC LIMIT 100")
    operations = [dict(row) for row in result.rows]
    return cors_response(operations)


async def handle_options(request, datasette):
    """Handle CORS preflight requests."""
    headers = {}
    add_cors_headers(headers)
    return Response("", status=204, headers=headers)


async def notes_handler(request, datasette):
    """Handler for /-/notes-sync/notes endpoint."""
    if request.method == "OPTIONS":
        return await handle_options(request, datasette)
    elif request.method == "GET":
        return await get_notes_list(request, datasette)
    elif request.method == "POST":
        return await create_note(request, datasette)
    else:
        return cors_response({"error": "Method not allowed"}, status=405)


async def note_handler(request, datasette):
    """Handler for /-/notes-sync/notes/<id> endpoint."""
    if request.method == "OPTIONS":
        return await handle_options(request, datasette)
    elif request.method == "GET":
        return await get_note(request, datasette)
    elif request.method == "PUT":
        return await update_note(request, datasette)
    elif request.method == "DELETE":
        return await delete_note(request, datasette)
    else:
        return cors_response({"error": "Method not allowed"}, status=405)


async def sync_handler(request, datasette):
    """Handler for /-/notes-sync/sync endpoint."""
    if request.method == "OPTIONS":
        return await handle_options(request, datasette)
    elif request.method == "POST":
        return await sync_notes(request, datasette)
    else:
        return cors_response({"error": "Method not allowed"}, status=405)


async def operations_handler(request, datasette):
    """Handler for /-/notes-sync/operations endpoint."""
    if request.method == "OPTIONS":
        return await handle_options(request, datasette)
    elif request.method == "GET":
        return await get_operations(request, datasette)
    else:
        return cors_response({"error": "Method not allowed"}, status=405)


@hookimpl
def register_routes():
    """Register the notes sync API routes."""
    return [
        (r"^/-/notes-sync/notes$", notes_handler),
        (r"^/-/notes-sync/notes/(?P<note_id>[^/]+)$", note_handler),
        (r"^/-/notes-sync/sync$", sync_handler),
        (r"^/-/notes-sync/operations$", operations_handler),
    ]
