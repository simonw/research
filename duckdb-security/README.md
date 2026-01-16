# DuckDB Security Constraints for Untrusted Query Execution

This investigation explores how to safely run DuckDB queries from untrusted users, enforcing:
1. **Read-only access** to preconfigured data sources
2. **File/network access restrictions** to prevent access to unauthorized files
3. **Query time limits** (e.g., 500ms)

## Summary

DuckDB provides robust security configuration options that, when combined properly, create a sandboxed environment suitable for running untrusted queries. The key findings are:

| Feature | Support | Method |
|---------|---------|--------|
| Read-only mode | Native | `read_only=True` or `access_mode='READ_ONLY'` |
| Block file access | Native | `enable_external_access=false` |
| Allowlist files | Native | `allowed_paths` / `allowed_directories` |
| Block HTTP/HTTPS | Native | `disabled_filesystems='HTTPFileSystem'` |
| Query timeout | **Workaround** | `connection.interrupt()` from timer thread |
| Lock config | Native | `lock_configuration=true` (v0.8.1+) |

## Quick Start

```python
import duckdb
import threading

# Create secure connection
con = duckdb.connect(":memory:")

# Allow only specific files
con.execute("SET allowed_paths = ['/data/public.csv']")
con.execute("SET enable_external_access = false")

# Set resource limits
con.execute("SET threads = 2")
con.execute("SET memory_limit = '256MB'")

# Lock configuration (cannot be changed by queries)
con.execute("SET lock_configuration = true")

# Execute with timeout
def run_with_timeout(query, timeout_sec=0.5):
    timer = threading.Timer(timeout_sec, con.interrupt)
    timer.start()
    try:
        return con.execute(query).fetchall()
    except duckdb.InterruptException:
        raise TimeoutError(f"Query exceeded {timeout_sec}s")
    finally:
        timer.cancel()
```

## Detailed Findings

### 1. Read-Only Access

Two equivalent methods for opening a database in read-only mode:

```python
# Method 1: Connection parameter
con = duckdb.connect("mydb.duckdb", read_only=True)

# Method 2: Config option
con = duckdb.connect("mydb.duckdb", config={"access_mode": "READ_ONLY"})
```

Both prevent: `INSERT`, `UPDATE`, `DELETE`, `CREATE TABLE`, `DROP TABLE`, etc.

**Verified:** ✅ See `demo_readonly.py`

### 2. File/Network Access Restrictions

#### Option A: Disable All External Access

```python
con = duckdb.connect(":memory:", config={"enable_external_access": "false"})
```

Blocks: `read_csv()`, `read_parquet()`, `COPY TO/FROM`, `ATTACH`, etc.

#### Option B: Disable Specific Filesystems

```sql
-- Must use SET after connection (not via config)
SET disabled_filesystems = 'LocalFileSystem';
SET disabled_filesystems = 'HTTPFileSystem';
SET disabled_filesystems = 'LocalFileSystem,HTTPFileSystem';
```

#### Option C: Allowlist Approach (Recommended)

```sql
-- Allow specific directories
SET allowed_directories = ['/data/public', '/tmp/cache'];

-- Or allow specific files
SET allowed_paths = ['/data/users.parquet', '/data/products.csv'];

-- Then disable external access (allowed paths still work!)
SET enable_external_access = false;
```

**Verified:** ✅ See `demo_file_restrictions.py`

### 3. Configuration Locking

Prevent untrusted queries from changing security settings:

```sql
SET lock_configuration = true;
```

After this, any `SET` statement will fail with `InvalidInputException`.

**Note:** Added in DuckDB v0.8.1 (June 2023). Known issue: also blocks `USE` command.

**Verified:** ✅ See `demo_file_restrictions.py` (Demo 5)

### 4. Query Timeouts

**DuckDB has no native query timeout setting!**

Workaround using `connection.interrupt()`:

```python
import threading
import duckdb

def execute_with_timeout(con, query, timeout_sec):
    timer = threading.Timer(timeout_sec, con.interrupt)
    timer.start()
    try:
        return con.execute(query).fetchall()
    except duckdb.InterruptException:
        raise TimeoutError(f"Query exceeded {timeout_sec}s timeout")
    finally:
        timer.cancel()
```

**Verified:** ✅ See `demo_timeout.py`

### 5. Resource Limits

```sql
SET threads = 2;                        -- Limit CPU parallelism
SET memory_limit = '256MB';             -- Limit memory usage
SET max_temp_directory_size = '100MB';  -- Limit temp storage
```

### 6. Asyncio Support

DuckDB's Python API is synchronous. For async applications:

- Third-party library: [`aioduckdb`](https://pypi.org/project/aioduckdb/)
- Uses thread pool internally (similar to `aiosqlite`)

## Complete Sandboxed Executor

See [`sandboxed_duckdb.py`](sandboxed_duckdb.py) for a production-ready wrapper class:

```python
from sandboxed_duckdb import SandboxedDuckDB

with SandboxedDuckDB(
    allowed_paths=["/data/public.parquet"],
    timeout_ms=500,
    memory_limit="256MB"
) as db:
    # Safe to run untrusted queries
    result = db.fetchall("SELECT * FROM read_parquet('/data/public.parquet')")
```

## Demo Scripts

| Script | Description |
|--------|-------------|
| `demo_readonly.py` | Read-only connection modes |
| `demo_file_restrictions.py` | File/network access controls |
| `demo_timeout.py` | Query timeout implementation |
| `sandboxed_duckdb.py` | Complete sandboxed executor |

Run any demo:
```bash
uv run --with duckdb python demo_readonly.py
```

## Security Checklist

For maximum security when running untrusted queries:

- [ ] Use in-memory database or read-only mode for existing DBs
- [ ] Set `allowed_paths` or `allowed_directories` for permitted files
- [ ] Set `enable_external_access = false`
- [ ] Set resource limits (`threads`, `memory_limit`, `max_temp_directory_size`)
- [ ] Set `lock_configuration = true` **last** (locks all settings)
- [ ] Implement query timeout using `interrupt()` + timer thread

## References

- [Securing DuckDB](https://duckdb.org/docs/stable/operations_manual/securing_duckdb/overview) - Official security guide
- [Configuration Options](https://duckdb.org/docs/stable/configuration/overview) - All settings
- [Python API](https://duckdb.org/docs/stable/clients/python/overview) - Python client docs
- [Query Timeout Discussion](https://github.com/duckdb/duckdb/issues/8564) - GitHub issue
- [lock_configuration Release](https://github.com/duckdb/duckdb/releases/tag/v0.8.1) - v0.8.1 release notes
