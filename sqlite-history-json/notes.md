# sqlite-history-json development notes

## Approach

Implementing the "updated values" approach from Simon Willison's TIL on JSON audit logs for SQLite.

### Design decisions

- **Second approach only**: Record new/updated values on INSERT and UPDATE. DELETE just records that a row was deleted.
- **Interface**: Plain `sqlite3.Connection` objects
- **Audit table naming**: `_history_json_{table_name}`
- **Primary keys**: Required (no rowid), compound PKs supported. PK columns appear in audit table.
- **Restore**: Creates a new table by default. Optional `swap=True` atomically replaces the original.
- **Populate**: Creates INSERT-style snapshot entries for all existing rows.

### Audit table schema

```
_history_json_{table_name}:
  id INTEGER PRIMARY KEY
  timestamp TEXT
  operation TEXT  -- 'insert', 'update', 'delete'
  {pk_col1} {type}  -- PK columns from original table
  {pk_col2} {type}  -- (if compound PK)
  updated_values TEXT  -- JSON
```

### JSON encoding conventions (from the TIL)

- NULL values: `{"null": 1}` (because json_patch treats null as "remove key")
- BLOB values: `{"hex": "AABBCC"}`
- Regular values: stored directly

### API

- `enable_tracking(conn, table_name)` - creates audit table + triggers
- `disable_tracking(conn, table_name)` - drops triggers (keeps audit table)
- `populate(conn, table_name)` - snapshot current state into audit log
- `restore(conn, table_name, timestamp, new_table_name=None, swap=False)` - replay audit log to point in time

### TDD approach

Writing tests first (RED), then implementing (GREEN).

## Progress log

- Initialized project with `uv init --lib`
- Added pytest as dev dependency
- RED phase: wrote 59 comprehensive tests covering all API functions, edge cases, parameterized column types
- Bug fix: PRAGMA table_info pk field is at index 5, not 4 (index 4 is dflt_value)
- GREEN phase: all 59 tests passing after pk index fix
- Fixed timestamp precision: switched from `datetime('now')` (second resolution) to `strftime('%Y-%m-%d %H:%M:%f', 'now')` (microsecond resolution)
- Added `up_to_id` parameter to `restore()` for precise version-based restore using audit log entry IDs
- Updated tests that relied on sub-second precision to use `up_to_id` instead of timestamp
- Added tests for tables with spaces, dots, quotes, and hyphens in names (4 new tests)
- Added indexes on audit tables: timestamp index + PK column index (2 new tests)
- Final: 65 tests all passing
- For single-PK tables, audit table uses `row_id` column name; for compound PKs, original column names are preserved
- `restore()` works by replaying audit entries in order: INSERT creates rows, UPDATE modifies them, DELETE removes them
- `swap=True` uses SQLite's ALTER TABLE RENAME for atomic replacement
