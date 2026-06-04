# DuckDB Security Constraints Investigation

## Goal
Explore options for safely running DuckDB queries from untrusted users:
1. Read-only access to preconfigured data sources
2. Prevent access to new files on disk or via HTTPS
3. Query time limits (e.g., 500ms)

## Investigation Log

### Starting Point
- Using Python with `duckdb` PyPI package (v1.4.3)
- Need to find configuration options for sandboxing queries

### Key Findings

#### 1. Read-Only Access
Two ways to enforce read-only access:
- `duckdb.connect(db_path, read_only=True)` - connection parameter
- `config={"access_mode": "READ_ONLY"}` - config option

Both prevent INSERT, UPDATE, DELETE, CREATE TABLE, DROP TABLE operations.

#### 2. File/Network Access Restrictions

**Option A: Disable all external access**
```python
con = duckdb.connect(":memory:", config={"enable_external_access": "false"})
```
Blocks read_csv(), COPY TO/FROM, ATTACH, etc.

**Option B: Disable specific filesystems**
```sql
SET disabled_filesystems = 'LocalFileSystem';
SET disabled_filesystems = 'HTTPFileSystem';
SET disabled_filesystems = 'LocalFileSystem,HTTPFileSystem';
```
Note: Must use SET after connection, not via config.

**Option C: Allowlist approach**
```sql
SET allowed_directories = ['/path/to/allowed/dir'];
SET allowed_paths = ['/path/to/specific/file.csv'];
SET enable_external_access = false;
```
Allows access only to specified directories/files even when external access is disabled.

#### 3. Configuration Locking
```sql
SET lock_configuration = true;
```
Prevents untrusted queries from changing any settings (including re-enabling external access).

**Note:** `lock_configuration` was added in DuckDB v0.8.1 (June 2023).
Known issue: also blocks `USE` command since it becomes `SET schema`.

#### 4. Query Timeouts
DuckDB has NO native query timeout setting!

Workaround: Use `connection.interrupt()` from a timer thread:
```python
timer = threading.Timer(0.5, connection.interrupt)  # 500ms
timer.start()
try:
    result = con.execute(query)
finally:
    timer.cancel()
```

#### 5. Resource Limits
- `SET threads = N` - limit CPU parallelism
- `SET memory_limit = '256MB'` - limit memory usage
- `SET max_temp_directory_size = '100MB'` - limit temp storage

#### 6. Asyncio Support
- No native asyncio support in DuckDB Python API
- Third-party library available: `aioduckdb` (https://pypi.org/project/aioduckdb/)
- Uses thread pool internally, similar to aiosqlite pattern

### Sources
- https://duckdb.org/docs/stable/operations_manual/securing_duckdb/overview
- https://duckdb.org/docs/stable/configuration/overview
- https://duckdb.org/docs/stable/clients/python/dbapi
- https://github.com/duckdb/duckdb/issues/8564 (query timeout discussion)
- https://github.com/duckdb/duckdb/releases/tag/v0.8.1 (lock_configuration added)
- https://pypi.org/project/aioduckdb/ (asyncio wrapper)
