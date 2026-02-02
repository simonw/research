# sqlite-history-json

A Python library for tracking SQLite table history using a JSON audit log.

Based on the pattern described in [Tracking SQLite table history using a JSON audit log](https://til.simonwillison.net/sqlite/json-audit-log) by Simon Willison.

## How it works

`sqlite-history-json` uses SQLite triggers to automatically record every INSERT, UPDATE, and DELETE operation on a tracked table into a companion audit log table. Changed values are stored as JSON, using SQLite's built-in `json_patch()` and `json_object()` functions.

This is the "updated values" approach: each audit entry records the **new** values of changed columns (not the old ones). This means:

- **INSERT** entries record all column values for the new row
- **UPDATE** entries record only the columns that changed, with their new values
- **DELETE** entries just record that the row was deleted (the PK identifies which row)

The audit log is self-contained: given only the audit table, you can fully reconstruct the tracked table's state at any point in history.

### JSON encoding conventions

| Value | JSON representation |
|-------|-------------------|
| Regular value | Stored directly: `"Widget"`, `42`, `3.14` |
| `NULL` | `{"null": 1}` (because `json_patch()` treats bare `null` as "remove key") |
| BLOB | `{"hex": "DEADBEEF"}` (hex-encoded binary) |

### Audit table schema

For a table called `items` with primary key `id`, the audit table `_history_json_items` looks like:

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER PRIMARY KEY | Auto-incrementing version number |
| `timestamp` | TEXT | ISO-8601 datetime with microsecond precision |
| `operation` | TEXT | `'insert'`, `'update'`, or `'delete'` |
| `pk_id` | (matches source PK type) | The primary key of the tracked row (prefixed with `pk_`) |
| `updated_values` | TEXT | JSON object of changed columns (NULL for deletes) |

Primary key columns in the audit table are always prefixed with `pk_` to distinguish them from the audit table's own columns. For compound primary keys, each PK column gets its own `pk_`-prefixed column (e.g., `pk_user_id`, `pk_role_id`).

Indexes are automatically created on `timestamp` and the PK column(s) for efficient querying.

## Installation

```bash
pip install sqlite-history-json
```

Or with [uv](https://docs.astral.sh/uv/):

```bash
uv add sqlite-history-json
```

## Usage

### Enable tracking on a table

```python
import sqlite3
from sqlite_history_json import enable_tracking, disable_tracking, populate, restore

conn = sqlite3.connect("mydb.db")

# Create your table
conn.execute("""
    CREATE TABLE items (
        id INTEGER PRIMARY KEY,
        name TEXT,
        price FLOAT,
        quantity INTEGER
    )
""")

# Start tracking changes
enable_tracking(conn, "items")

# Now all INSERT, UPDATE, DELETE operations are automatically logged
conn.execute("INSERT INTO items VALUES (1, 'Widget', 9.99, 100)")
conn.execute("UPDATE items SET price = 12.99 WHERE id = 1")
conn.execute("DELETE FROM items WHERE id = 1")
```

### Snapshot existing data

By default, `enable_tracking()` automatically populates the audit log with a snapshot of all existing rows. This means the audit log is complete from the moment tracking starts:

```python
# Table already has rows in it...
enable_tracking(conn, "items")  # automatically snapshots existing rows

# From this point on, the audit log has a complete record
```

You can opt out of auto-population if you want to control when the snapshot happens:

```python
enable_tracking(conn, "items", populate_table=False)
# ... do something else ...
populate(conn, "items")  # manually snapshot when ready
```

### Restore to a point in time

```python
# Restore table state to a specific timestamp (creates a new table)
restored_name = restore(conn, "items", timestamp="2024-06-15 14:30:00.000000")

# Query the restored table
rows = conn.execute(f"SELECT * FROM [{restored_name}]").fetchall()
```

### Restore to a specific version (audit entry ID)

Since `datetime('now')` in SQLite has second-level precision in some contexts, you can use `up_to_id` to get exact version-level restore using the audit log's auto-incrementing primary key:

```python
# Restore to the state after audit entry #42
restored_name = restore(conn, "items", up_to_id=42)
```

### Restore with atomic swap

Replace the original table with the restored version:

```python
# Atomically replaces `items` with the restored state
restore(conn, "items", up_to_id=42, swap=True)

# `items` now contains the restored data
```

### Custom restored table name

```python
restored_name = restore(
    conn, "items", timestamp="2024-06-15 14:30:00",
    new_table_name="items_backup"
)
```

### Disable tracking

```python
# Drops the triggers but keeps the audit table and its data
disable_tracking(conn, "items")
```

### Compound primary keys

Tables with compound primary keys are fully supported:

```python
conn.execute("""
    CREATE TABLE user_roles (
        user_id INTEGER,
        role_id INTEGER,
        granted_by TEXT,
        active INTEGER,
        PRIMARY KEY (user_id, role_id)
    )
""")

enable_tracking(conn, "user_roles")

# The audit table `_history_json_user_roles` will have
# `pk_user_id` and `pk_role_id` columns
```

### Tables with special characters in names

Table names containing spaces, hyphens, dots, and other special characters are handled correctly:

```python
conn.execute('CREATE TABLE "order items" (id INTEGER PRIMARY KEY, product TEXT)')
enable_tracking(conn, "order items")
```

## API reference

### `enable_tracking(conn, table_name, *, populate_table=True)`

Creates the audit table `_history_json_{table_name}` and installs INSERT, UPDATE, and DELETE triggers on the source table. Also creates indexes on the audit table for timestamp and primary key columns.

By default, snapshots all existing rows into the audit log (equivalent to calling `populate()` automatically). Pass `populate_table=False` to skip this.

Idempotent: calling it twice has no additional effect (auto-populate only runs if the audit table is empty).

**Requirements:** The table must have an explicit `PRIMARY KEY` (not just `rowid`).

### `disable_tracking(conn, table_name)`

Drops the triggers. The audit table and its data are preserved.

Idempotent: calling it when no triggers exist is a no-op.

### `populate(conn, table_name)`

Inserts one `'insert'` audit entry per existing row, creating a baseline snapshot. Usually not needed directly since `enable_tracking()` calls this automatically, but useful if you passed `populate_table=False` and want to snapshot later.

### `restore(conn, table_name, *, timestamp=None, up_to_id=None, new_table_name=None, swap=False)`

Replays audit log entries to reconstruct the table state. All parameters after `table_name` are keyword-only.

- **`timestamp`**: Restore up to this ISO-8601 timestamp (inclusive)
- **`up_to_id`**: Restore up to this audit entry ID (inclusive). More precise than timestamp for operations within the same second.
- **`new_table_name`**: Name for the restored table (default: `{table_name}_restored`)
- **`swap`**: If `True`, atomically replaces the original table

Returns the name of the restored table.

## Development

```bash
# Clone and set up
git clone https://github.com/simonw/sqlite-history-json
cd sqlite-history-json
uv sync

# Run tests
uv run pytest tests/ -v
```

## How the triggers work

The UPDATE trigger uses nested `json_patch()` calls to build a JSON object containing only the columns that actually changed:

```sql
INSERT INTO _history_json_items (timestamp, operation, pk_id, updated_values)
VALUES (
    strftime('%Y-%m-%d %H:%M:%f', 'now'),
    'update',
    NEW.id,
    json_patch(
        json_patch(
            json_patch(
                '{}',
                CASE
                    WHEN OLD.name IS NOT NEW.name THEN
                        CASE
                            WHEN NEW.name IS NULL THEN json_object('name', json_object('null', 1))
                            ELSE json_object('name', NEW.name)
                        END
                    ELSE '{}'
                END
            ),
            CASE
                WHEN OLD.price IS NOT NEW.price THEN
                    CASE
                        WHEN NEW.price IS NULL THEN json_object('price', json_object('null', 1))
                        ELSE json_object('price', NEW.price)
                    END
                ELSE '{}'
            END
        ),
        CASE
            WHEN OLD.quantity IS NOT NEW.quantity THEN
                CASE
                    WHEN NEW.quantity IS NULL THEN json_object('quantity', json_object('null', 1))
                    ELSE json_object('quantity', NEW.quantity)
                END
            ELSE '{}'
        END
    )
);
```

Each column gets a `CASE` expression that:
1. Checks if the old and new values differ (`IS NOT` handles NULL correctly)
2. If different, creates a JSON object with the column name and new value
3. If unchanged, returns `'{}'` (empty object)

These are combined with `json_patch()` which merges JSON objects together, building up the final diff.
