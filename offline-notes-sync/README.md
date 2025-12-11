# Offline Notes Sync System

A prototype offline-first notes application with smart synchronization, similar to Apple Notes.

## Features

- **Offline-first architecture**: Create and edit notes without network connectivity
- **Smart sync**: Automatically merges non-conflicting changes from multiple clients
- **Conflict resolution**: When edits overlap, provides UI to choose local, remote, or merged content
- **Character-level merge**: Detects and auto-merges edits at different positions in the same text
- **Operation-based sync**: Stores operations (not just state) for precise conflict detection

## Architecture

### Client
- Single HTML page with vanilla JavaScript (no framework dependencies)
- IndexedDB for local note storage with pending operation queue
- Service Worker for offline caching of static assets
- Sync manager that batches and sends operations when online

### Server
- Python Starlette + SQLite backend
- CORS-enabled JSON REST API
- Vector clocks for tracking causal relationships between edits
- Three-way merge with character-level diff algorithm

## How the Sync Works

1. **Local edits** are saved immediately to IndexedDB
2. **Operations** (create/update/delete) are queued for sync
3. **When online**, client sends pending operations to server
4. **Server applies** operations and detects conflicts via vector clocks
5. **Merge algorithm** auto-resolves non-overlapping edits
6. **Client receives** updated notes and server operations
7. **Conflicts** are presented to user for manual resolution

## Merge Algorithm

The merge algorithm (`diff_merge.py`) implements Apple Notes-style merging:

```
Base:   "Hello World"
Local:  "Hello Beautiful World"  (inserted "Beautiful " at position 6)
Remote: "Hello World!"           (appended "!" at end)
Result: "Hello Beautiful World!" (both changes merged!)
```

For overlapping edits:
```
Base:   "Hello World"
Local:  "Hello Earth"  (changed "World" to "Earth")
Remote: "Hello Mars"   (changed "World" to "Mars")
Result: Conflict markers requiring user resolution
```

## Running the Application

### Install dependencies
```bash
pip install starlette uvicorn httpx
```

### Start the server
```bash
python server.py
```

### Open the client
Navigate to http://localhost:8000/static/index.html

## Running Tests

### Install test dependencies
```bash
pip install pytest playwright
playwright install chromium
```

### Run all tests
```bash
pytest test_notes_app.py -v
```

### Test coverage
- **API tests**: CRUD operations, sync protocol
- **Merge tests**: Character-level diff, non-overlapping merge, conflict detection
- **UI tests**: Note creation, editing, deletion, sync status
- **Offline tests**: Service worker, IndexedDB persistence

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/notes` | GET | List all non-deleted notes |
| `/api/notes` | POST | Create a new note |
| `/api/notes/{id}` | GET | Get a single note |
| `/api/notes/{id}` | PUT | Update a note |
| `/api/notes/{id}` | DELETE | Soft-delete a note |
| `/api/sync` | POST | Exchange operations with server |
| `/api/operations` | GET | Debug: list all operations |

## Sync Protocol

**Client request:**
```json
{
  "client_id": "unique-client-id",
  "last_sync": "2024-01-01T00:00:00Z",
  "operations": [
    {
      "id": "op-uuid",
      "note_id": "note-uuid",
      "timestamp": "2024-01-01T00:00:00Z",
      "op_type": "update",
      "field": "content",
      "old_value": "original text",
      "new_value": "edited text",
      "base_version_vector": "{\"client_a\": 1}"
    }
  ]
}
```

**Server response:**
```json
{
  "sync_timestamp": "2024-01-01T00:00:01Z",
  "operations": [/* operations from other clients */],
  "notes": [/* current state of affected notes */]
}
```

## Files

| File | Description |
|------|-------------|
| `server.py` | Starlette server with SQLite backend |
| `diff_merge.py` | Character-level diff and three-way merge |
| `static/index.html` | Client HTML/CSS/JavaScript |
| `static/sw.js` | Service worker for offline caching |
| `test_notes_app.py` | Pytest + Playwright test suite |
| `requirements.txt` | Python dependencies |

## Limitations

- Single-user per browser (client_id stored in localStorage)
- No authentication/authorization
- Soft-delete only (deleted notes not truly removed)
- Simple conflict UI (could show diff view)
- No real-time WebSocket updates (polling-based sync)

## Future Improvements

- WebSocket for real-time updates
- CRDT-based text editing (e.g., Yjs) for true real-time collaboration
- Rich text support with formatting merge
- Note sharing and multi-user access control
- End-to-end encryption for notes
