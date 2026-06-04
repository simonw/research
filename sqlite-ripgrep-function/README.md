# SQLite Ripgrep Function

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A custom SQLite function for running [ripgrep](https://github.com/BurntSushi/ripgrep) searches directly from SQL queries. Provides both a pure Python implementation and a C extension with table-valued function support.

## Features

- **Fast code search** via ripgrep from within SQL
- **Configurable base directory** - searches are constrained to a specific directory tree
- **Time limit support** - prevents runaway searches (inspired by [datasette-ripgrep](https://github.com/simonw/datasette-ripgrep))
- **File filtering** - use glob patterns like `*.py` or `*.js`
- **Multiple arity** - 1-3 arguments depending on what you need
- **Two implementations**:
  - Pure Python (scalar function returning JSON)
  - C extension (proper table-valued function)

## Installation

### Python Implementation

No compilation needed - just import the module:

```python
import sqlite3
from sqlite_ripgrep_python import register_ripgrep_function

conn = sqlite3.connect(':memory:')
register_ripgrep_function(conn, base_directory='/path/to/search')
```

### C Extension

Build the shared library:

```bash
# Basic build (default base directory: /tmp)
make

# Build with custom base directory
make RIPGREP_BASE_DIR=/home/user/code

# Build with custom time limit (default: 1.0 seconds)
make RIPGREP_BASE_DIR=/home/user/code RIPGREP_TIME_LIMIT=2.0
```

Load in Python:

```python
import sqlite3

conn = sqlite3.connect(':memory:')
conn.enable_load_extension(True)
conn.load_extension('./sqlite_ripgrep')
```

Or load in SQLite CLI:

```sql
.load ./sqlite_ripgrep
```

## Usage

### Python Implementation (Scalar Function)

Returns results as a JSON string:

```python
# Single argument - search pattern only
result = conn.execute("SELECT ripgrep('hello')").fetchone()[0]

# Two arguments - pattern + glob filter
result = conn.execute("SELECT ripgrep('def ', '*.py')").fetchone()[0]

# Three arguments - pattern + glob + time limit
result = conn.execute("SELECT ripgrep('TODO', '*.rs', 5.0)").fetchone()[0]
```

The JSON result structure:

```json
{
  "results": [
    {
      "path": "src/main.py",
      "line_number": 42,
      "lines": "def hello_world():\n",
      "submatches": [{"match": "def ", "start": 0, "end": 4}]
    }
  ],
  "count": 1,
  "truncated": false,
  "time_limit_hit": false,
  "error": null
}
```

### Python Table-Valued Workaround

Use `json_each()` to expand results as rows:

```python
from sqlite_ripgrep_python import register_ripgrep_with_table_helper

conn = sqlite3.connect(':memory:')
register_ripgrep_with_table_helper(conn, '/path/to/search')

# Query results as table using json_each
rows = conn.execute("""
    SELECT
        json_extract(value, '$.path') as path,
        json_extract(value, '$.line_number') as line_number,
        json_extract(value, '$.lines') as line_text
    FROM json_each(ripgrep_results('pattern', '*.py'))
""").fetchall()
```

### C Extension (Table-Valued Function)

True table-valued function with proper SQL integration:

```sql
-- Basic search
SELECT path, line_number, line_text
FROM ripgrep
WHERE pattern = 'hello';

-- With file filter
SELECT * FROM ripgrep
WHERE pattern = 'def \w+' AND glob = '*.py';

-- With time limit
SELECT * FROM ripgrep
WHERE pattern = 'TODO' AND time_limit = 5.0;

-- Override base directory
SELECT * FROM ripgrep
WHERE pattern = 'error' AND base_dir = '/var/log';
```

#### Schema

The C extension creates a virtual table with these columns:

| Column | Type | Description |
|--------|------|-------------|
| `path` | TEXT | File path (relative to base directory) |
| `line_number` | INTEGER | Line number of the match |
| `line_text` | TEXT | Full text of the matching line |
| `match_text` | TEXT | The actual matched substring |
| `match_start` | INTEGER | Start offset of match in line |
| `match_end` | INTEGER | End offset of match in line |
| `pattern` | TEXT HIDDEN | Search pattern (required constraint) |
| `glob` | TEXT HIDDEN | File glob filter (optional) |
| `time_limit` | REAL HIDDEN | Time limit in seconds (optional) |
| `base_dir` | TEXT HIDDEN | Base directory override (optional) |

## Testing

Run the test suite:

```bash
python3 test_sqlite_ripgrep.py
```

Tests cover:
- Basic search functionality
- File filtering with globs
- Time limit behavior
- Case sensitivity
- Regex patterns
- Edge cases and error handling
- Both Python and C implementations

## Requirements

- **ripgrep** (`rg`) must be installed and in PATH
- Python 3.7+
- For C extension: GCC and SQLite development headers

## API Reference

### Python Functions

#### `register_ripgrep_function(conn, base_directory, function_name='ripgrep', default_time_limit=1.0)`

Register the ripgrep scalar function with a SQLite connection.

- `conn`: SQLite connection object
- `base_directory`: Root directory for searches
- `function_name`: Name of the SQL function (default: 'ripgrep')
- `default_time_limit`: Default timeout in seconds (default: 1.0)

#### `register_ripgrep_with_table_helper(conn, base_directory, ...)`

Same as above, but also registers `{function_name}_results()` for use with `json_each()`.

#### `run_ripgrep(pattern, base_directory, glob_pattern=None, time_limit=1.0, ...)`

Low-level function to run ripgrep directly. Returns a dictionary with results.

### C Extension

The extension automatically creates a `ripgrep` virtual table when loaded. Use WHERE clauses to provide search parameters.

## Limitations

1. **Python table-valued functions**: Python's sqlite3 module doesn't support true table-valued functions. The `json_each()` workaround is functional but less elegant.

2. **Cross-join constraints**: When joining the ripgrep virtual table with other tables, constraints referencing other table columns won't be pushed through to ripgrep. Use subqueries or CTEs instead.

3. **Signal handling (C extension)**: The C extension uses `SIGALRM` for time limits, which may interfere with other signal handlers in the process.

## Examples

### Find all TODO comments in Python files

```sql
SELECT path, line_number, line_text
FROM ripgrep
WHERE pattern = 'TODO|FIXME|XXX' AND glob = '*.py'
ORDER BY path, line_number;
```

### Count matches per file

```sql
SELECT path, COUNT(*) as match_count
FROM ripgrep
WHERE pattern = 'import' AND glob = '*.py'
GROUP BY path
ORDER BY match_count DESC;
```

### Search with context (Python version)

```python
result = run_ripgrep(
    pattern='error',
    base_directory='/var/log',
    context_lines=2,  # Show 2 lines before/after
    time_limit=5.0
)
```

## License

MIT License

## Acknowledgments

- Time limit implementation inspired by [datasette-ripgrep](https://github.com/simonw/datasette-ripgrep) by Simon Willison
- Built on [ripgrep](https://github.com/BurntSushi/ripgrep) by Andrew Gallant
