# sqlite3-wasm

A Python library that provides an `sqlite3_wasm` module which behaves exactly like Python's standard library `sqlite3`, but runs SQLite compiled to WebAssembly using wasmtime.

## Features

- Drop-in replacement for Python's `sqlite3` module
- SQLite 3.45.3 compiled to WebAssembly
- Runs via wasmtime (no native dependencies)
- Supports in-memory databases
- Full text search (FTS5)
- JSON1 extension
- R-Tree extension

## Installation

Clone and install with uv:

```bash
cd sqlite3_wasm
uv sync
```

Or build and install the wheel:

```bash
uv build
pip install dist/sqlite3_wasm-0.1.0-py3-none-any.whl
```

## Usage

```python
import sqlite3_wasm

# Connect to an in-memory database
conn = sqlite3_wasm.connect(':memory:')
cursor = conn.cursor()

# Create a table
cursor.execute('CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)')

# Insert data
cursor.execute('INSERT INTO users (name, email) VALUES (?, ?)', ('Alice', 'alice@example.com'))
cursor.execute('INSERT INTO users (name, email) VALUES (?, ?)', ('Bob', 'bob@example.com'))

# Query data
cursor.execute('SELECT * FROM users')
for row in cursor.fetchall():
    print(row)
# Output: (1, 'Alice', 'alice@example.com')
#         (2, 'Bob', 'bob@example.com')

# Use named parameters
cursor.execute('SELECT * FROM users WHERE name = :name', {'name': 'Alice'})
print(cursor.fetchone())  # (1, 'Alice', 'alice@example.com')

# Use Row factory for named column access
conn.row_factory = sqlite3_wasm.Row
cursor = conn.cursor()
cursor.execute('SELECT * FROM users WHERE id = 1')
row = cursor.fetchone()
print(row['name'])  # 'Alice'
print(row['email'])  # 'alice@example.com'

conn.close()
```

## API Compatibility

The module implements the Python DB-API 2.0 specification and provides the same API as the standard `sqlite3` module:

### Module-level

- `connect(database, ...)` - Connect to a database
- `complete_statement(sql)` - Check if SQL is complete
- `Row` - Row factory for named column access
- Exception classes: `Error`, `DatabaseError`, `IntegrityError`, `OperationalError`, `ProgrammingError`, `InterfaceError`, `DataError`, `NotSupportedError`

### Connection

- `cursor()` - Create a cursor
- `execute(sql, parameters)` - Execute SQL and return cursor
- `executemany(sql, seq_of_parameters)` - Execute SQL with multiple parameter sets
- `executescript(sql_script)` - Execute multiple SQL statements
- `commit()` - Commit transaction
- `rollback()` - Rollback transaction
- `close()` - Close connection
- `row_factory` - Property to set row factory
- `isolation_level` - Transaction isolation level
- `total_changes` - Total number of rows modified
- `in_transaction` - Whether in a transaction
- `iterdump()` - Iterate SQL statements to recreate database

### Cursor

- `execute(sql, parameters)` - Execute SQL statement
- `executemany(sql, seq_of_parameters)` - Execute with multiple parameter sets
- `executescript(sql_script)` - Execute multiple statements
- `fetchone()` - Fetch next row
- `fetchmany(size)` - Fetch next batch of rows
- `fetchall()` - Fetch all remaining rows
- `close()` - Close cursor
- `description` - Column information
- `rowcount` - Number of rows affected
- `lastrowid` - Last inserted row ID
- `arraysize` - Default fetch size

## Limitations

Due to the WebAssembly execution environment, some features are not supported:

- **User-defined functions** - Cannot create Python callbacks in WASM
- **User-defined aggregates** - Cannot create Python callbacks in WASM
- **Collation functions** - Cannot create Python callbacks in WASM
- **Progress handlers** - Cannot create Python callbacks in WASM
- **Trace callbacks** - Cannot create Python callbacks in WASM
- **Authorizer callbacks** - Cannot create Python callbacks in WASM
- **Database backup** - Requires file system access

These methods will raise `NotSupportedError` if called.

## Running Tests

```bash
uv run pytest
```

## Building

```bash
uv build
```

This produces a wheel in `dist/` that includes the pre-compiled SQLite WASM binary.

## How It Works

1. SQLite is compiled to WebAssembly using wasi-sdk
2. The WASM binary is bundled with the Python package
3. wasmtime loads and runs the WASM module
4. Python wrapper calls SQLite C API functions through WASM exports
5. Memory is managed by allocating/freeing in WASM linear memory

## Requirements

- Python 3.11+
- wasmtime >= 40.0.0

## License

MIT
