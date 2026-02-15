# Investigation Notes: sqlite-chronicle + sqlite-history-json on the Same Table

## Goal
Investigate what happens when both `sqlite-chronicle` and `sqlite-history-json` are enabled on the same SQLite database table. Do they coexist? Do they interfere with each other? Are there trigger conflicts?

## Background Research

### sqlite-chronicle
- Creates `_chronicle_{table_name}` companion table
- Tracks: `__added_ms`, `__updated_ms`, `__version`, `__deleted`
- Uses triggers: BEFORE INSERT, AFTER INSERT, AFTER UPDATE, AFTER DELETE
- Designed for sync/replication use cases

### sqlite-history-json
- Creates `_history_json_{table_name}` audit table + shared `_history_json` groups table
- Tracks full operation history as JSON: operation type, changed values, timestamps
- Uses triggers: AFTER INSERT, AFTER UPDATE, AFTER DELETE
- Designed for full audit log / point-in-time restoration

### Key Differences
- Chronicle tracks current state + version number (sync-oriented)
- History-json tracks full operation history (audit-oriented)
- Both use triggers on the same events
- Both require explicit primary keys

## Investigation Plan
1. Install both libraries
2. Create a test table
3. Enable chronicle first, then history-json (and vice versa)
4. Test INSERT, UPDATE, DELETE operations
5. Check trigger ordering and potential conflicts
6. Test edge cases: INSERT OR REPLACE, compound keys, deletions
7. Test disable/re-enable scenarios

## Key Findings

### 1. They coexist perfectly for basic operations
- Both libraries create separate trigger sets with distinct naming conventions
- Chronicle: `chronicle_{table}_{bi,ai,au,ad}` (4 triggers)
- History-json: `_history_json_{table}_{insert,update,delete}` (3 triggers)
- Total: 7 triggers per table, no naming conflicts
- INSERT, UPDATE, DELETE all work correctly with both active
- Enable order doesn't matter (chronicle-first or history-json-first both work)

### 2. Both work with compound primary keys
- Chronicle creates PK columns matching the source table
- History-json creates `pk_`-prefixed columns for each PK component
- Operations on compound PK tables tracked correctly by both

### 3. No-op detection differs
- **Chronicle**: Intelligently detects no-op UPDATEs via WHEN clause (no version bump)
- **Chronicle**: Detects no-op INSERT OR REPLACE via BEFORE INSERT snapshot mechanism
- **History-json**: Records no-op UPDATEs as update entries (with empty `{}` updated_values)
- **History-json**: Records no-op INSERT OR REPLACE as a new insert entry

### 4. INSERT OR REPLACE behavior depends on recursive_triggers
- With `recursive_triggers = OFF` (default): only BEFORE INSERT and AFTER INSERT triggers fire
  - History-json records as two consecutive "insert" operations (no "delete")
  - Chronicle uses BEFORE INSERT snapshot to detect it was a REPLACE
- With `recursive_triggers = ON`: DELETE triggers also fire
  - History-json records: insert, delete, insert (3 entries)
  - Chronicle's AFTER DELETE is suppressed by snapshot WHEN clause
- Both libraries handle both settings correctly

### 5. restore(swap=True) destroys ALL triggers (CRITICAL FINDING)
- History-json's `restore(swap=True)` drops the original table and renames the restored one
- This destroys ALL triggers on the table — both chronicle's and history-json's own
- `is_chronicle_enabled()` returns True (misleading — checks table, not triggers)
- **Fix**: After swap, must disable+re-enable chronicle, then re-enable history-json

### 6. Disable/re-enable works cleanly
- Disabling chronicle leaves history-json functioning (and vice versa)
- History-json preserves audit data when disabled (only drops triggers)
- Chronicle drops the companion table when disabled
- Re-enabling either library while the other is active works without issues

### 7. Change groups span both libraries
- History-json's `change_group()` context manager works across multiple tables
- Changes within a group are tracked by chronicle too (as individual version bumps)
- The `_history_json` groups table is shared across all tracked tables

### 8. Performance overhead is roughly additive
- Chronicle only: ~1.5x baseline
- History-json only: ~1.7x baseline
- Both together: ~2.3x baseline (roughly the sum of individual overheads)
- For 1000 ops each of INSERT/UPDATE/DELETE: 55ms vs 24ms baseline

### 9. Multiple tables work fine
- Both libraries can track many tables simultaneously
- Chronicle: separate `_chronicle_{table}` per table + shared `_chroniclesnapshots`
- History-json: separate `_history_json_{table}` per table + shared `_history_json` groups table
- 7 triggers per tracked table, 11 tables total for 3 source tables
