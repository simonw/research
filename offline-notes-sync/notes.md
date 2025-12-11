# Offline Notes Sync - Development Notes

## Project Goal
Build a prototype offline-first notes application with:
- Single HTML page client (no React)
- Service workers for offline capability
- IndexedDB for local storage
- Python Starlette + SQLite server
- Smart merge/sync similar to Apple Notes

## Architecture Design

### Sync Strategy Research

Apple Notes and similar apps use sophisticated sync mechanisms. Key approaches:

1. **Operational Transformation (OT)** - Used by Google Docs
   - Transforms operations based on concurrent edits
   - Complex to implement correctly

2. **Conflict-free Replicated Data Types (CRDTs)** - Used by Figma, Apple Notes
   - Data structures that automatically merge
   - Eventually consistent without coordination

3. **Three-way merge** - Used by Git
   - Compare current versions against common ancestor
   - Good for text content

### Chosen Approach: Hybrid CRDT + Three-Way Merge

For this prototype, I'll implement:

1. **Vector clocks** for causality tracking
   - Each client has a unique ID
   - Each edit increments that client's clock
   - Detect concurrent vs sequential edits

2. **Per-field conflict resolution**:
   - Title: Last-writer-wins with vector clock comparison
   - Content: Character-level CRDT (simplified) or three-way merge
   - Metadata: LWW with timestamps

3. **Operation log** for sync
   - Store operations, not just state
   - Replay operations to reconstruct state
   - Enable offline operation accumulation

### Data Model

```
Note {
  id: UUID (client-generated)
  title: string
  content: string
  created_at: ISO timestamp
  updated_at: ISO timestamp
  version_vector: {client_id: counter, ...}
  deleted: boolean (soft delete)
}

Operation {
  id: UUID
  note_id: UUID
  client_id: string
  timestamp: ISO timestamp
  type: 'create' | 'update' | 'delete'
  field: string (for updates)
  value: any
  base_version_vector: {...}
}
```

### Sync Protocol

1. **Client connects** → sends its last sync timestamp and version vectors
2. **Server responds** with operations since that timestamp
3. **Client applies** server operations, merging conflicts
4. **Client sends** its local operations since last sync
5. **Server applies** client operations, handling conflicts
6. **Server responds** with final state and any conflict resolutions

### Conflict Resolution Rules

1. If operations are causally related (one's vector dominates), apply in order
2. If concurrent:
   - For content: Use text CRDT or three-way merge
   - For title/metadata: LWW with client_id as tiebreaker
3. Deletions: Soft delete, can be "resurrected" by concurrent edit

## Implementation Log

### Step 1: Server Implementation
- Starting with Starlette + SQLite
- Implementing REST API with CORS
- Tables: notes, operations, clients

### Step 2: Client Implementation
- Single HTML file with embedded JS
- IndexedDB for local storage
- Service Worker for offline capability
- Sync manager class

### Step 3: Merge Algorithm
- Implementing simplified text CRDT for content
- Vector clock comparison utilities
- Three-way merge fallback

## Implementation Progress

### Completed Components

1. **Server (server.py)**
   - Starlette ASGI framework with CORS support
   - SQLite database with notes, operations, and clients tables
   - REST API: GET/POST notes, PUT/DELETE individual notes
   - Sync endpoint that exchanges operations and returns updated notes
   - Vector clock tracking for conflict detection

2. **Client (static/index.html)**
   - Single HTML file with embedded JavaScript
   - IndexedDB for local note storage
   - Service worker for offline caching (sw.js)
   - Sync manager that queues operations when offline
   - Conflict resolution UI with choice of local/remote/merge

3. **Diff/Merge Algorithm (diff_merge.py)**
   - Operation-based diffing that identifies insertion/deletion positions
   - Non-overlapping change detection for automatic merging
   - Character-level position tracking
   - Line-based merge for multi-line content with character-level fallback
   - Conflict markers when edits truly overlap

4. **Tests (test_notes_app.py)**
   - 27 tests covering API, sync, UI, and merge functionality
   - Playwright browser tests for UI interactions
   - httpx for API testing
   - All tests passing

## Key Learnings

1. **Operation-based sync vs state-based sync**
   - Storing operations allows replay and conflict detection
   - Position-based operations enable detecting non-overlapping edits
   - Vector clocks track causality between clients

2. **Character-level merge challenges**
   - Finding common prefix/suffix is straightforward
   - Determining overlap requires tracking positions in base text
   - Non-overlapping insertions at different positions can be auto-merged
   - Overlapping edits to the same region require conflict resolution

3. **Service Worker limitations**
   - Good for caching static assets
   - API requests should fail gracefully when offline
   - IndexedDB provides better offline storage than localStorage

4. **Test isolation with Playwright**
   - Browser tests share database state with API tests
   - Use `.first` for locators that may match multiple elements
   - Use specific text matching to avoid ambiguous selectors

