# SQLite crash: eponymous virtual tables across `sqlite3_deserialize`

On one connection, use an eponymous virtual table, replace `main` with `sqlite3_deserialize`
(Python's `Connection.deserialize`), then prepare a statement that uses an eponymous virtual table
again. SQLite reads through a stale schema pointer and usually segfaults. The affected tables
include every `pragma_*` table-valued function, `json_each`, and sparqlite's own `sparql_rows`.
SQLite fails while preparing the statement, before any extension code runs.

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
- `rust/` is the same check against the SQLite bundled with rusqlite 0.40.2 (3.53.2). Run
  `cargo run --release -- "<sql>"`.

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

On 3.45.1, gdb puts the crash in `sqlite3_prepare_v3`, then `sqlite3GenerateColumnNames`, then
`strlen`.

On 3.53.2 the bug is still present, but whether a case actually crashes depends on heap layout:

- `SELECT name FROM pragma_database_list`, `pragma_index_xinfo` and `pragma_function_list`
  segfaulted.
- `pragma_index_info`, `pragma_schema_version`, `pragma_table_xinfo` and `json_each` happened to
  survive.

Under valgrind, every one of those cases shows an invalid 8-byte read during `sqlite3_prepare_v3`,
in `selectExpander` (or `columnTypeImpl` for `SELECT name FROM pragma_database_list`). For
`pragma_schema_version` the read lands 24 bytes past an 800-byte block. The `PRAGMA` statement form
reports 0 errors.

## What sparqlite does about it

- Its own SQL reads catalog information only through `PRAGMA` statements, which are not virtual
  tables:
  - `PRAGMA main.index_info`, `PRAGMA main.schema_version`, `PRAGMA main.table_xinfo`,
    `PRAGMA main.index_list` and `PRAGMA main.index_xinfo` in `sq-sqlite`;
  - `PRAGMA database_list` in `sq-api`'s writer guard.
- So sparqlite never touches an eponymous table behind the user's back. Its scalar functions
  (`sparql_query`, `sparql_update`, ...) keep working after a deserialize, and they answer from
  the new database because the caches are keyed on the main file's identity.
- `sparql_rows` is an eponymous table, and table-valued-function syntax requires one, so the
  extension cannot avoid this. Workarounds:
  - deserialize into a fresh connection;
  - or deserialize before the connection's first use of `sparql_rows` (or of any `pragma_*` or
    `json_each` table);
  - or use `sparql_query` after a deserialize.
