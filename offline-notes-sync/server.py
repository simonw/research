"""
Offline Notes Sync Server

A Starlette + SQLite JSON API for syncing notes with offline-first clients.
Implements vector clocks and operation-based sync for smart merging.
"""

import json
import sqlite3
import uuid
from datetime import datetime
from pathlib import Path
from typing import Any

from starlette.applications import Starlette

# Import character-level merge algorithm
from diff_merge import merge_texts
from starlette.middleware import Middleware
from starlette.middleware.cors import CORSMiddleware
from starlette.requests import Request
from starlette.responses import JSONResponse
from starlette.routing import Route
from starlette.staticfiles import StaticFiles


# Database setup
DB_PATH = Path(__file__).parent / "notes.db"


def get_db() -> sqlite3.Connection:
    """Get a database connection with row factory."""
    conn = sqlite3.connect(str(DB_PATH))
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    return conn


def init_db():
    """Initialize the database schema."""
    conn = get_db()
    conn.executescript("""
        -- Notes table stores the current state of each note
        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            version_vector TEXT NOT NULL DEFAULT '{}',
            deleted INTEGER NOT NULL DEFAULT 0
        );

        -- Operations table stores all operations for sync
        CREATE TABLE IF NOT EXISTS operations (
            id TEXT PRIMARY KEY,
            note_id TEXT NOT NULL,
            client_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            op_type TEXT NOT NULL,
            field TEXT,
            old_value TEXT,
            new_value TEXT,
            base_version_vector TEXT NOT NULL,
            FOREIGN KEY (note_id) REFERENCES notes(id)
        );

        -- Index for efficient sync queries
        CREATE INDEX IF NOT EXISTS idx_operations_timestamp ON operations(timestamp);
        CREATE INDEX IF NOT EXISTS idx_operations_note_id ON operations(note_id);

        -- Clients table tracks known clients
        CREATE TABLE IF NOT EXISTS clients (
            id TEXT PRIMARY KEY,
            last_seen TEXT NOT NULL,
            last_sync TEXT
        );
    """)
    conn.commit()
    conn.close()


def parse_version_vector(vv_str: str) -> dict:
    """Parse a version vector from JSON string."""
    if not vv_str:
        return {}
    return json.loads(vv_str)


def serialize_version_vector(vv: dict) -> str:
    """Serialize a version vector to JSON string."""
    return json.dumps(vv, sort_keys=True)


def compare_version_vectors(vv1: dict, vv2: dict) -> str:
    """
    Compare two version vectors.
    Returns:
        'equal': vectors are identical
        'before': vv1 happened before vv2
        'after': vv1 happened after vv2
        'concurrent': vectors are concurrent (conflict)
    """
    all_keys = set(vv1.keys()) | set(vv2.keys())

    vv1_dominates = False
    vv2_dominates = False

    for key in all_keys:
        v1 = vv1.get(key, 0)
        v2 = vv2.get(key, 0)

        if v1 > v2:
            vv1_dominates = True
        elif v2 > v1:
            vv2_dominates = True

    if not vv1_dominates and not vv2_dominates:
        return 'equal'
    elif vv1_dominates and not vv2_dominates:
        return 'after'
    elif vv2_dominates and not vv1_dominates:
        return 'before'
    else:
        return 'concurrent'


def merge_version_vectors(vv1: dict, vv2: dict) -> dict:
    """Merge two version vectors by taking max of each component."""
    result = dict(vv1)
    for key, value in vv2.items():
        result[key] = max(result.get(key, 0), value)
    return result


def increment_version_vector(vv: dict, client_id: str) -> dict:
    """Increment the version vector for a client."""
    result = dict(vv)
    result[client_id] = result.get(client_id, 0) + 1
    return result


def three_way_merge(base: str, ours: str, theirs: str) -> tuple[str, bool]:
    """
    Perform a three-way merge of text content using character-level diff.
    Returns (merged_text, had_conflict).

    Uses the merge_texts function from diff_merge module which implements:
    - Character-level LCS (Longest Common Subsequence) diffing
    - Three-way merge with automatic conflict resolution when possible
    - Conflict markers only when edits truly overlap
    """
    return merge_texts(base or '', ours or '', theirs or '')


def apply_operation(conn: sqlite3.Connection, op: dict, server_vv: dict = None) -> dict:
    """
    Apply an operation to the database.
    Returns the updated note state.
    """
    note_id = op['note_id']
    client_id = op['client_id']
    op_type = op['op_type']

    # Get current note state
    cursor = conn.execute("SELECT * FROM notes WHERE id = ?", (note_id,))
    row = cursor.fetchone()

    now = datetime.utcnow().isoformat() + 'Z'

    if op_type == 'create':
        if row is not None:
            # Note already exists - this is a duplicate create, skip
            return dict(row)

        vv = {client_id: 1}
        conn.execute("""
            INSERT INTO notes (id, title, content, created_at, updated_at, version_vector, deleted)
            VALUES (?, ?, ?, ?, ?, ?, 0)
        """, (
            note_id,
            op.get('new_value', {}).get('title', ''),
            op.get('new_value', {}).get('content', ''),
            op.get('timestamp', now),
            now,
            serialize_version_vector(vv)
        ))

    elif op_type == 'update':
        if row is None:
            # Note doesn't exist - create it first
            vv = {client_id: 1}
            conn.execute("""
                INSERT INTO notes (id, title, content, created_at, updated_at, version_vector, deleted)
                VALUES (?, '', '', ?, ?, ?, 0)
            """, (note_id, now, now, serialize_version_vector(vv)))
            cursor = conn.execute("SELECT * FROM notes WHERE id = ?", (note_id,))
            row = cursor.fetchone()

        current_vv = parse_version_vector(row['version_vector'])
        base_vv = parse_version_vector(op.get('base_version_vector', '{}'))

        field = op.get('field')
        new_value = op.get('new_value')

        # Check for conflicts
        comparison = compare_version_vectors(base_vv, current_vv)

        if comparison == 'concurrent' and field in ('title', 'content'):
            # Concurrent edit - need to merge
            if field == 'content':
                # Use three-way merge for content
                base_content = op.get('old_value', '')
                merged, had_conflict = three_way_merge(base_content, row['content'], new_value)
                new_value = merged
            # For title, use LWW with client_id as tiebreaker (later client wins)

        # Update the field
        new_vv = increment_version_vector(
            merge_version_vectors(current_vv, base_vv),
            client_id
        )

        if field == 'title':
            conn.execute("""
                UPDATE notes SET title = ?, updated_at = ?, version_vector = ?
                WHERE id = ?
            """, (new_value, now, serialize_version_vector(new_vv), note_id))
        elif field == 'content':
            conn.execute("""
                UPDATE notes SET content = ?, updated_at = ?, version_vector = ?
                WHERE id = ?
            """, (new_value, now, serialize_version_vector(new_vv), note_id))
        elif field == 'both':
            # Update both title and content
            conn.execute("""
                UPDATE notes SET title = ?, content = ?, updated_at = ?, version_vector = ?
                WHERE id = ?
            """, (
                new_value.get('title', row['title']),
                new_value.get('content', row['content']),
                now,
                serialize_version_vector(new_vv),
                note_id
            ))

    elif op_type == 'delete':
        if row is not None:
            current_vv = parse_version_vector(row['version_vector'])
            new_vv = increment_version_vector(current_vv, client_id)
            conn.execute("""
                UPDATE notes SET deleted = 1, updated_at = ?, version_vector = ?
                WHERE id = ?
            """, (now, serialize_version_vector(new_vv), note_id))

    # Record the operation
    op_id = op.get('id') or str(uuid.uuid4())
    try:
        conn.execute("""
            INSERT INTO operations (id, note_id, client_id, timestamp, op_type, field, old_value, new_value, base_version_vector)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            op_id,
            note_id,
            client_id,
            op.get('timestamp', now),
            op_type,
            op.get('field'),
            json.dumps(op.get('old_value')) if op.get('old_value') is not None else None,
            json.dumps(op.get('new_value')) if op.get('new_value') is not None else None,
            op.get('base_version_vector', '{}')
        ))
    except sqlite3.IntegrityError:
        # Operation already exists, skip
        pass

    # Return updated note
    cursor = conn.execute("SELECT * FROM notes WHERE id = ?", (note_id,))
    row = cursor.fetchone()
    if row:
        return dict(row)
    return None


# API Routes

async def get_notes(request: Request) -> JSONResponse:
    """Get all non-deleted notes."""
    conn = get_db()
    cursor = conn.execute("""
        SELECT id, title, content, created_at, updated_at, version_vector, deleted
        FROM notes WHERE deleted = 0
        ORDER BY updated_at DESC
    """)
    notes = []
    for row in cursor:
        notes.append({
            'id': row['id'],
            'title': row['title'],
            'content': row['content'],
            'created_at': row['created_at'],
            'updated_at': row['updated_at'],
            'version_vector': parse_version_vector(row['version_vector']),
            'deleted': bool(row['deleted'])
        })
    conn.close()
    return JSONResponse(notes)


async def get_note(request: Request) -> JSONResponse:
    """Get a single note by ID."""
    note_id = request.path_params['note_id']
    conn = get_db()
    cursor = conn.execute("""
        SELECT id, title, content, created_at, updated_at, version_vector, deleted
        FROM notes WHERE id = ?
    """, (note_id,))
    row = cursor.fetchone()
    conn.close()

    if row is None:
        return JSONResponse({'error': 'Note not found'}, status_code=404)

    return JSONResponse({
        'id': row['id'],
        'title': row['title'],
        'content': row['content'],
        'created_at': row['created_at'],
        'updated_at': row['updated_at'],
        'version_vector': parse_version_vector(row['version_vector']),
        'deleted': bool(row['deleted'])
    })


async def create_note(request: Request) -> JSONResponse:
    """Create a new note."""
    data = await request.json()

    note_id = data.get('id') or str(uuid.uuid4())
    client_id = data.get('client_id') or 'server'

    conn = get_db()

    op = {
        'id': str(uuid.uuid4()),
        'note_id': note_id,
        'client_id': client_id,
        'timestamp': datetime.utcnow().isoformat() + 'Z',
        'op_type': 'create',
        'new_value': {
            'title': data.get('title', ''),
            'content': data.get('content', '')
        },
        'base_version_vector': '{}'
    }

    note = apply_operation(conn, op)
    conn.commit()
    conn.close()

    return JSONResponse({
        'id': note['id'],
        'title': note['title'],
        'content': note['content'],
        'created_at': note['created_at'],
        'updated_at': note['updated_at'],
        'version_vector': parse_version_vector(note['version_vector']),
        'deleted': bool(note['deleted'])
    }, status_code=201)


async def update_note(request: Request) -> JSONResponse:
    """Update an existing note."""
    note_id = request.path_params['note_id']
    data = await request.json()

    client_id = data.get('client_id') or 'server'
    base_vv = data.get('version_vector', {})

    conn = get_db()

    # Get current note for old values
    cursor = conn.execute("SELECT * FROM notes WHERE id = ?", (note_id,))
    row = cursor.fetchone()

    if row is None:
        conn.close()
        return JSONResponse({'error': 'Note not found'}, status_code=404)

    # Determine what fields changed
    changes = {}
    if 'title' in data and data['title'] != row['title']:
        changes['title'] = data['title']
    if 'content' in data and data['content'] != row['content']:
        changes['content'] = data['content']

    if not changes:
        conn.close()
        return JSONResponse({
            'id': row['id'],
            'title': row['title'],
            'content': row['content'],
            'created_at': row['created_at'],
            'updated_at': row['updated_at'],
            'version_vector': parse_version_vector(row['version_vector']),
            'deleted': bool(row['deleted'])
        })

    # Create operations for each changed field
    for field, new_value in changes.items():
        op = {
            'id': str(uuid.uuid4()),
            'note_id': note_id,
            'client_id': client_id,
            'timestamp': datetime.utcnow().isoformat() + 'Z',
            'op_type': 'update',
            'field': field,
            'old_value': row[field],
            'new_value': new_value,
            'base_version_vector': serialize_version_vector(base_vv)
        }
        note = apply_operation(conn, op)

    conn.commit()

    # Get final state
    cursor = conn.execute("SELECT * FROM notes WHERE id = ?", (note_id,))
    row = cursor.fetchone()
    conn.close()

    return JSONResponse({
        'id': row['id'],
        'title': row['title'],
        'content': row['content'],
        'created_at': row['created_at'],
        'updated_at': row['updated_at'],
        'version_vector': parse_version_vector(row['version_vector']),
        'deleted': bool(row['deleted'])
    })


async def delete_note(request: Request) -> JSONResponse:
    """Soft delete a note."""
    note_id = request.path_params['note_id']
    data = await request.json() if request.headers.get('content-type') == 'application/json' else {}

    client_id = data.get('client_id') or 'server'

    conn = get_db()

    op = {
        'id': str(uuid.uuid4()),
        'note_id': note_id,
        'client_id': client_id,
        'timestamp': datetime.utcnow().isoformat() + 'Z',
        'op_type': 'delete',
        'base_version_vector': '{}'
    }

    apply_operation(conn, op)
    conn.commit()
    conn.close()

    return JSONResponse({'status': 'deleted'})


async def sync(request: Request) -> JSONResponse:
    """
    Sync endpoint - the heart of the offline sync system.

    Client sends:
    {
        "client_id": "unique-client-id",
        "last_sync": "ISO timestamp or null",
        "operations": [list of operations since last sync]
    }

    Server responds:
    {
        "sync_timestamp": "ISO timestamp",
        "operations": [list of operations since client's last_sync],
        "notes": [current state of all notes touched by sync]
    }
    """
    data = await request.json()

    client_id = data.get('client_id')
    if not client_id:
        return JSONResponse({'error': 'client_id required'}, status_code=400)

    last_sync = data.get('last_sync')
    client_operations = data.get('operations', [])

    conn = get_db()
    now = datetime.utcnow().isoformat() + 'Z'

    # Register/update client
    conn.execute("""
        INSERT INTO clients (id, last_seen, last_sync)
        VALUES (?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET last_seen = ?, last_sync = ?
    """, (client_id, now, last_sync, now, last_sync))

    # Get server operations since client's last sync
    if last_sync:
        cursor = conn.execute("""
            SELECT * FROM operations
            WHERE timestamp > ? AND client_id != ?
            ORDER BY timestamp ASC
        """, (last_sync, client_id))
    else:
        cursor = conn.execute("""
            SELECT * FROM operations
            WHERE client_id != ?
            ORDER BY timestamp ASC
        """, (client_id,))

    server_operations = []
    for row in cursor:
        server_operations.append({
            'id': row['id'],
            'note_id': row['note_id'],
            'client_id': row['client_id'],
            'timestamp': row['timestamp'],
            'op_type': row['op_type'],
            'field': row['field'],
            'old_value': json.loads(row['old_value']) if row['old_value'] else None,
            'new_value': json.loads(row['new_value']) if row['new_value'] else None,
            'base_version_vector': row['base_version_vector']
        })

    # Apply client operations
    affected_note_ids = set()
    for op in client_operations:
        op['client_id'] = client_id  # Ensure client_id is set
        apply_operation(conn, op)
        affected_note_ids.add(op['note_id'])

    # Also include notes affected by server operations
    for op in server_operations:
        affected_note_ids.add(op['note_id'])

    # Get current state of affected notes
    notes = []
    for note_id in affected_note_ids:
        cursor = conn.execute("""
            SELECT id, title, content, created_at, updated_at, version_vector, deleted
            FROM notes WHERE id = ?
        """, (note_id,))
        row = cursor.fetchone()
        if row:
            notes.append({
                'id': row['id'],
                'title': row['title'],
                'content': row['content'],
                'created_at': row['created_at'],
                'updated_at': row['updated_at'],
                'version_vector': parse_version_vector(row['version_vector']),
                'deleted': bool(row['deleted'])
            })

    conn.commit()
    conn.close()

    return JSONResponse({
        'sync_timestamp': now,
        'operations': server_operations,
        'notes': notes
    })


async def get_operations(request: Request) -> JSONResponse:
    """Get operations for debugging/admin."""
    since = request.query_params.get('since')
    note_id = request.query_params.get('note_id')

    conn = get_db()

    query = "SELECT * FROM operations WHERE 1=1"
    params = []

    if since:
        query += " AND timestamp > ?"
        params.append(since)

    if note_id:
        query += " AND note_id = ?"
        params.append(note_id)

    query += " ORDER BY timestamp ASC"

    cursor = conn.execute(query, params)
    operations = []
    for row in cursor:
        operations.append({
            'id': row['id'],
            'note_id': row['note_id'],
            'client_id': row['client_id'],
            'timestamp': row['timestamp'],
            'op_type': row['op_type'],
            'field': row['field'],
            'old_value': json.loads(row['old_value']) if row['old_value'] else None,
            'new_value': json.loads(row['new_value']) if row['new_value'] else None,
            'base_version_vector': row['base_version_vector']
        })

    conn.close()
    return JSONResponse(operations)


async def health(request: Request) -> JSONResponse:
    """Health check endpoint."""
    return JSONResponse({'status': 'ok'})


# CORS middleware for cross-origin requests
middleware = [
    Middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )
]

# Routes
routes = [
    Route("/", health),
    Route("/health", health),
    Route("/api/notes", get_notes, methods=["GET"]),
    Route("/api/notes", create_note, methods=["POST"]),
    Route("/api/notes/{note_id}", get_note, methods=["GET"]),
    Route("/api/notes/{note_id}", update_note, methods=["PUT", "PATCH"]),
    Route("/api/notes/{note_id}", delete_note, methods=["DELETE"]),
    Route("/api/sync", sync, methods=["POST"]),
    Route("/api/operations", get_operations, methods=["GET"]),
]

# Create app
app = Starlette(
    debug=True,
    routes=routes,
    middleware=middleware,
    on_startup=[init_db]
)

# Mount static files for the client
static_dir = Path(__file__).parent / "static"
if static_dir.exists():
    app.mount("/static", StaticFiles(directory=str(static_dir)), name="static")


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
