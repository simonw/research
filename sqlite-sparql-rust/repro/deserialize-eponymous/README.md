# SQLite crash: eponymous virtual tables across `sqlite3_deserialize`

The bug needs three steps on one connection:

1. Use an eponymous virtual table.
2. Replace `main` with `sqlite3_deserialize` (Python's `Connection.deserialize`).
3. Prepare a new statement that uses that table again.

SQLite then reads through a stale schema pointer and usually segfaults. The affected tables include
every `pragma_*` table-valued function, `json_each`, and sparqlite's own `sparql_rows` and
`sparql_status`. SQLite fails while preparing the statement, before any extension code runs.

This bug is in SQLite, not in sparqlite. It was found during the final audit, when sparqlite's
schema detection used `pragma_index_info` and a deserialize test segfaulted. The same root cause
was reported independently in
[photostructure/sqlite-deserialize-repro](https://github.com/photostructure/sqlite-deserialize-repro),
which reproduces it on SQLite trunk of 2026-10-02 and gives this explanation:

- `sqlite3VtabEponymousTableInit()` saves the main schema pointer in the eponymous table once.
- `sqlite3_deserialize` frees that schema and installs a new one.
- The next `sqlite3SchemaToIndex()` call then reads past the end of `db->aDb[]`.

## Files

- `repro.py` runs each case in a child process and reports `ok` or `SEGFAULT`. It needs only the
  standard library; pass `--ext path/to/libsparqlite.so` to include sparqlite's functions.
  - The statement cache is disabled so that the second query really is prepared again. With
    Python's default cache the old statement is reused, and nothing crashes.
- `rust/` is the same check against the SQLite bundled with rusqlite 0.40.2 (3.53.2). Run
  `cargo run --release -- "<sql>"`; with no argument it runs `pragma_index_info`.

Output on SQLite 3.45.1 (Ubuntu's `libsqlite3`, used by Python here):

```
$ python3 repro.py --ext ../../ext/target/release/libsparqlite.so
SQLite 3.45.1
  PRAGMA statement           ok
  pragma_index_info          SEGFAULT
  pragma_table_xinfo         SEGFAULT
  pragma_database_list       SEGFAULT
  pragma_schema_version      SEGFAULT
  json_each                  SEGFAULT
  sparql_query (scalar)      ok
  sparql_rows (eponymous)    SEGFAULT
```

On 3.45.1, gdb shows each crash inside `sqlite3_prepare_v3`:

- most cases fault in `sqlite3SchemaToIndex`, reached through `sqlite3SelectPrep`;
- a query that names a column can fault later instead, in `sqlite3GenerateColumnNames`. One
  example is `SELECT name FROM pragma_index_info(...) ORDER BY seqno` in the sq-sqlite test, which
  ended in `strlen`.

On 3.53.2 the bug is still present. Whether a given case actually segfaults varies between runs and
builds, because it depends on heap layout:

- In one run `SELECT name FROM pragma_database_list`, `pragma_index_xinfo` and
  `pragma_function_list` crashed while `pragma_index_info` survived.
- An independent rebuild crashed on `pragma_index_info` and `pragma_index_xinfo`.

Under valgrind, every eponymous case shows an invalid 8-byte read during `sqlite3_prepare_v3`, in
`selectExpander` (or `columnTypeImpl` for `SELECT name FROM pragma_database_list`). For
`pragma_schema_version` the read lands 24 bytes past an 800-byte block. The `PRAGMA` statement form
reports 0 errors.

## What sparqlite does about it

- Its own SQL reads catalog information only through `sqlite_schema` and `PRAGMA` statements,
  neither of which is a virtual table:
  - `PRAGMA main.index_info`, `PRAGMA main.schema_version`, `PRAGMA main.table_xinfo`,
    `PRAGMA main.index_list` and `PRAGMA main.index_xinfo` in `sq-sqlite`;
  - `PRAGMA database_list` in `sq-api`'s writer guard.
- The scalar functions (`sparql_query`, `sparql_update`, ...) keep working after a deserialize.
  They answer from the new database, because the caches are keyed on the main file's identity.
- The one eponymous table the extension uses itself is `sparql_status`. Loading the extension runs
  `SELECT 1 FROM main.sparql_status`, which connects the close hook, and the same query runs again
  on re-arm. So after a deserialize on that connection, querying `sparql_status` is at risk even if
  you never used it. Valgrind confirms this: loading, then deserializing, then querying
  `sparql_status` gives invalid reads.
- `sparql_rows` must be an eponymous table, because table-valued-function syntax requires one. It
  is at risk only if it was used before the deserialize.

**Workaround:** deserialize first, then load the extension. Under valgrind that order is clean for
`sparql_query`, `sparql_rows` and `sparql_status`. If the extension is already loaded, use only the
scalar functions on that connection after the deserialize, plus `sparql_rows` if it was not used
before.
