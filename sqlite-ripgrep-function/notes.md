# SQLite Ripgrep Function Development Notes

## Goal
Build a custom SQLite SQL function for running ripgrep searches with:
- Configurable base directory
- Multiple arity variants (1 arg for search term, 2 args for search term + file filter)
- Time limit implementation (based on datasette-ripgrep)
- Two versions: Pure Python and SQLite C extension

## Progress

### Step 1: Analyze datasette-ripgrep for time limit implementation

Cloned `https://github.com/simonw/datasette-ripgrep` to `/tmp/datasette-ripgrep`.

Key findings from `datasette_ripgrep/__init__.py`:
1. Uses `asyncio.create_subprocess_exec` to run `rg` with `--json` output
2. Time limit implemented via `asyncio.wait_for(inner(results), timeout=time_limit)`
3. When timeout occurs, catches `asyncio.TimeoutError` and sets `time_limit_hit = True`
4. Always kills the process after getting results: `proc.kill()`
5. Uses `--glob` for file filtering
6. Default time limit is 1.0 second, max_lines is 2000

### Step 2: Build pure Python SQLite ripgrep function

Created `sqlite_ripgrep_python.py` with:
- `run_ripgrep()` - Core function that spawns rg subprocess
- Uses `threading.Timer` for synchronous time limit (since sqlite3 callbacks are synchronous)
- `create_ripgrep_function()` - Creates a closure with configured base directory
- `register_ripgrep_function()` - Registers with sqlite3 connection using `create_function`
- Supports variable arity via `nargs=-1`:
  - `ripgrep(pattern)` - search only
  - `ripgrep(pattern, glob)` - with file filter
  - `ripgrep(pattern, glob, time_limit)` - with custom time limit

**Table-valued approach for Python:**
Since Python's sqlite3 doesn't support true table-valued functions, implemented a workaround:
- `register_ripgrep_with_table_helper()` registers `ripgrep_results(pattern)` that returns JSON array
- Can be expanded using SQLite's built-in `json_each()`:
```sql
SELECT json_extract(value, '$.path') as path,
       json_extract(value, '$.line_number') as line_number
FROM json_each(ripgrep_results('pattern', '*.py'))
```

### Step 3: Build SQLite C extension table-valued function

Created `sqlite_ripgrep_ext.c` implementing a proper SQLite virtual table module.

Key components:
1. **Virtual table structure** (`ripgrep_vtab`) - stores base_directory and default_time_limit
2. **Cursor structure** (`ripgrep_cursor`) - stores results array and iteration state
3. **xBestIndex** - Handles query planning, identifies pattern/glob/time_limit/base_dir constraints
4. **xFilter** - Executes ripgrep when query starts, using constraints passed from xBestIndex
5. **xNext/xColumn/xEof** - Standard iteration methods

**Time limit in C:**
- Uses `fork()` to spawn ripgrep process
- Parent sets up `SIGALRM` with `setitimer(ITIMER_REAL, ...)`
- Signal handler kills child process when time limit exceeded
- Timer cancelled after reading results

**Schema:**
```sql
CREATE TABLE ripgrep(
    path TEXT,
    line_number INTEGER,
    line_text TEXT,
    match_text TEXT,
    match_start INTEGER,
    match_end INTEGER,
    pattern TEXT HIDDEN,      -- Required parameter
    glob TEXT HIDDEN,         -- Optional file filter
    time_limit REAL HIDDEN,   -- Optional time limit
    base_dir TEXT HIDDEN      -- Optional base directory override
);
```

**Usage:**
```sql
SELECT * FROM ripgrep WHERE pattern = 'hello' AND base_dir = '/path/to/search';
SELECT * FROM ripgrep WHERE pattern = 'def \w+' AND glob = '*.py';
```

### Step 4: Testing

Created comprehensive test suite in `test_sqlite_ripgrep.py`:
- `TestRunRipgrep` - Direct function tests
- `TestPythonSQLiteFunction` - SQLite scalar function tests
- `TestPythonTableHelper` - json_each expansion tests
- `TestCExtension` - C virtual table tests
- `TestTimeLimitBehavior` - Timeout tests
- `TestEdgeCases` - Edge case handling

All 26 tests pass.

## Key Learnings

1. **Python sqlite3 limitations**: No native support for table-valued functions. Workaround using JSON + `json_each()` works but is less elegant than C extension.

2. **SQLite virtual table constraints**: When joining a virtual table with another table, constraints from the other table don't get pushed through to `xFilter`. Must use direct WHERE clauses or subqueries.

3. **Time limit implementation**:
   - Python: `threading.Timer` for synchronous code
   - C: `setitimer(ITIMER_REAL)` + `SIGALRM` handler

4. **idxNum bitmask pattern**: Using a bitmask in `idxNum` to track which optional parameters were provided allows flexible parameter ordering.

5. **ripgrep JSON output**: The `--json` flag produces JSON lines with `type` field. Match results have `type: "match"` with nested `data.path.text`, `data.line_number`, `data.lines.text`, and `data.submatches[]`.

## Files Created

- `sqlite_ripgrep_python.py` - Pure Python implementation
- `sqlite_ripgrep_ext.c` - C extension source
- `sqlite3ext.h` - SQLite extension header (downloaded)
- `Makefile` - Build configuration
- `test_sqlite_ripgrep.py` - Unit tests
