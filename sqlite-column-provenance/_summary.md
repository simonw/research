Determining the source `table.column` for each result column in arbitrary SQLite queries is feasible because SQLite computes this internally and exposes it via its column-metadata API when compiled with `SQLITE_ENABLE_COLUMN_METADATA`. While Python’s standard `sqlite3` module doesn’t surface this information, robust methods exist: using the third-party `apsw` library provides direct access with `cursor.description_full`, or a pure-stdlib ctypes bridge (`column_provenance.py`) can retrieve the metadata via direct calls to the system SQLite library—both accurately map even complex queries, recognizing expressions and handling joins, subqueries, and CTEs. Alternative approaches using `EXPLAIN` bytecode or the authorizer hook give partial information and are best for simple cases or dependency checks. For static analysis, `sqlglot` can resolve lineage using a supplied schema, including expressions.

**Key Tools and Projects:**
- [`apsw`](https://github.com/rogerbinns/apsw): exposes SQLite's column metadata directly for per-column provenance.
- [`column_provenance.py`](https://github.com/simonmichael/sqlite-column-provenance): pure-Python ctypes bridge that mirrors APSW’s results with no extra dependencies.

**Highlights:**
- Metadata APIs accurately resolve base column sources, expressions, and complex query structures without query execution.
- The standard library alone can suffice via clever ctypes usage, including handling in-memory databases.
- EXPLAIN-based approaches are best-effort and may falter on compound queries (like UNION).
- Authorizer methods over-report for output mapping but are valuable for access control.
