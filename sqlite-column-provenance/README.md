# Mapping SQLite result columns back to their source `table.column`

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

**Question:** given an arbitrary SQL query executed against SQLite from Python,
how do we figure out which columns in the result set correspond to specific
columns in the underlying tables?

```
select id, name from t1                              -> t1.id, t1.name
select * from t1                                     -> t1.id, t1.name, t1.extra
select t1.id, t1.name, age from t1 join t2 ...       -> t1.id, t1.name, t2.age
select id, name || '-suffix' from t1                 -> t1.id, (expression — no clean source)
```

**Short answer:** SQLite already computes exactly this. Compiled with
`SQLITE_ENABLE_COLUMN_METADATA` (the default on virtually every Linux distro and
in `apsw`), it exposes three C functions —
`sqlite3_column_origin_name`, `sqlite3_column_table_name`,
`sqlite3_column_database_name` — that report, per result column, the **real base
table and column** the value came from, or `NULL` if the column is a computed
expression. You don't even have to run the query; *preparing* it is enough.

The catch: Python's standard-library `sqlite3` module does **not** surface these
(its `cursor.description` is name-only). This report shows four ways to get at
the answer anyway, three of which need nothing but the standard library.

## TL;DR recommendation

| If you can…                        | Use                                              | Robustness |
|------------------------------------|--------------------------------------------------|------------|
| add a dependency                   | **`apsw`** → `cursor.description_full`            | ★★★★★ best, one line |
| use only the stdlib                | **`column_provenance.py`** (ctypes → libsqlite3)  | ★★★★★ matches apsw 10/10 |
| use only the stdlib, no native API | `explain_provenance.py` (EXPLAIN bytecode)        | ★★★☆☆ simple queries |
| just need the *dependency set*     | `authorizer_lineage.py` (authorizer hook)         | ★★★☆☆ set, not mapping |
| no execution, expression lineage   | `sqlglot` `qualify()`                             | ★★★★☆ needs schema dict |

The column-metadata approach (apsw or the ctypes bridge) is the clear winner: it
operates on SQLite's resolved query AST, so it transparently handles `*`
expansion, joins, `AS` aliases, **and even subqueries / CTEs / unions**, while
correctly reporting "no source" for expressions.

## The five approaches

### 1. `apsw.Cursor.description_full` — the reference (needs `apsw`)
APSW exposes the metadata API directly. Each entry is
`(name, declared_type, database, table, origin_column)`:

```python
import apsw
db = apsw.Connection("my.db")
for name, decltype, dbname, table, origin in db.execute(sql).description_full:
    print(name, "<-", f"{table}.{origin}" if table else "(expression)")
```

### 2. `column_provenance.py` — stdlib + a clever ctypes bridge ⭐
Pure standard library: we `ctypes`-load the system `libsqlite3`, call
`sqlite3_prepare_v2`, and read the origin/table/database functions ourselves.
Two entry points:

```python
from column_provenance import resolve_columns, resolve_columns_for_connection

# (a) against a database file (opened read-only, query only prepared, never run):
for c in resolve_columns("my.db", "select id, name from t1"):
    print(c)                       # id <- t1.id

# (b) against a *live stdlib sqlite3.Connection* — including :memory: databases:
import sqlite3
conn = sqlite3.connect(":memory:"); conn.executescript(schema)
for c in resolve_columns_for_connection(conn, sql):
    print(c.output_name, c.table, c.origin_column, c.source)
```

The neat trick for in-memory connections: stdlib `sqlite3` can't expose the
statement to ctypes, so `resolve_columns_for_connection` snapshots the database
with `Connection.serialize()` (Python 3.11+) and loads those bytes into a
private ctypes connection via `sqlite3_deserialize`. File-backed connections
skip the copy and just open the file read-only. Verified to match `apsw`
`description_full` on **10/10** battery queries, in-memory and on disk.

### 3. `explain_provenance.py` — pure stdlib via `EXPLAIN` bytecode (heuristic)
No native calls at all. We parse the VDBE program from `EXPLAIN <sql>`:

* `OpenRead p1=cursor p2=rootpage` → map cursor → table via `sqlite_master.rootpage`
* `Column p1=cursor p2=colidx p3=reg` → register `reg` now holds `table.colidx`
* `Rowid  p1=cursor p2=reg` → register holds the `INTEGER PRIMARY KEY` alias
* `ResultRow p1=start p2=count` → output = registers `start … start+count-1`

For each output register we find its last writer. A `Column`/`Rowid` straight
off a base-table cursor is a clean source; anything else (`Concat`, `Function`,
`String8`, `Count`, …) is a computed expression. We set
`PRAGMA automatic_index=OFF` so the optimizer doesn't route joined columns
through an ephemeral index. This **agrees with the metadata oracle on 9/10**
battery queries — it even resolves the subquery and CTE (the optimizer flattens
them); only `UNION` misses, because the compound select reads through an
ephemeral co-routine cursor with no base-table rootpage. Treat it as best-effort
for the simple cases, which was the brief.

### 4. `authorizer_lineage.py` — pure stdlib via the authorizer hook
`Connection.set_authorizer(cb)` fires `SQLITE_READ(table, column)` for every
column access *while preparing* the statement. This yields the **set of columns
the query depends on** — perfect for column-level lineage or access control
(return `SQLITE_DENY` to block a column). It is **not** a per-output mapping:
the reads also include WHERE/JOIN/ORDER/GROUP columns and every `*`-expanded
column, and an expression like `name || '-suffix'` merely shows a read of
`t1.name` with no signal that the output was transformed. `naive_output_mapping`
only trusts the read order when `#reads == #outputs` (single table, no filter,
no expression) and returns `None` otherwise.

### 5. `sqlglot` `qualify()` — static, no execution (bonus)
Parses and resolves the query against a supplied schema dict without touching a
database. Resolves `*` and aliases and, uniquely, attributes **expression**
columns to their **input** columns (`name || '-suffix'` → "uses `t1.name`"),
which the C API cannot. Downsides: extra dependency, its own SQL dialect parser,
and you must hand it the schema.

## Side-by-side results

From `compare_all.py` (all driven off a stdlib in-memory connection):

| Query | metadata / apsw | EXPLAIN | authorizer (dependency set) |
|-------|-----------------|---------|------------------------------|
| `select id, name from t1` | `t1.id, t1.name` | ✅ same | `{t1.id, t1.name}` |
| `select * from t1` | `t1.id, t1.name, t1.extra` | ✅ same | `{t1.id, t1.name, t1.extra}` |
| `select t1.id, t1.name, age from t1 join t2 …` | `t1.id, t1.name, t2.age` | ✅ same | `{t1.id, t1.name, t2.age, t2.name}` |
| `select id, name \|\| '-suffix' from t1` | `t1.id, EXPR` | ✅ same | `{t1.id, t1.name}` |
| `select t1.*, t2.age from t1 join t2 …` | `t1.id, t1.name, t1.extra, t2.age` | ✅ same | full read set |
| `select id as the_id, name as nm from t1` | `t1.id, t1.name` (aliased) | ✅ same | `{t1.id, t1.name}` |
| `select count(*), max(age) from t2` | `EXPR, EXPR` | ✅ same | `{t2.age}` |
| `select age from (select * from t2) sub` | `t2.age` | ✅ same | `{t2.id, t2.name, t2.age}` |
| `with c as (select id,name from t1) select * from c` | `t1.id, t1.name` | ✅ same | `{t1.id, t1.name}` |
| `select name from t1 union select name from t2` | `t1.name` | ❌ EXPR (ephemeral cursor) | `{t1.name, t2.name}` |

## Key findings & gotchas

* **You don't need to execute the query.** Both the metadata and EXPLAIN
  approaches only *prepare* it, so no user rows are read and side-effect-free.
* **Expressions correctly resolve to "no source."** `name || '-suffix'`,
  `upper(x)`, arithmetic, literals, and aggregates all report `NULL`
  table/column — exactly the desired "this is no longer a clean match" signal.
* **`*`, joins, aliases, subqueries, CTEs, and unions all work** with the
  metadata API, because it reports the *true underlying base-table column* after
  SQLite resolves the query internally.
* **Ambiguous bare columns are a hard error.** `select id … from t1 join t2`
  when both tables have `id` raises `ambiguous column name: id` at prepare time
  (the user's "harder still" example only resolves when the bare column lives in
  exactly one joined table; qualify it as `t1.id` and it's fine).
* **EXPLAIN is optimizer-dependent.** `PRAGMA automatic_index=OFF` keeps joined
  columns on their base cursors; compound selects (UNION) still escape into
  ephemeral cursors. Use it only for simple queries.
* **The authorizer over-reports for mapping** (it includes filter columns and
  can't see that an output was transformed) but is ideal for lineage/redaction.

## Files

| File | What it is |
|------|------------|
| `column_provenance.py` | ⭐ stdlib ctypes bridge to the column-metadata API (file + live Connection) |
| `explain_provenance.py` | pure-stdlib EXPLAIN bytecode parser (heuristic) |
| `authorizer_lineage.py` | pure-stdlib authorizer-hook dependency set |
| `compare_all.py` | runs all techniques side by side on the battery |
| `test_provenance.py` | exercises the ctypes bridge (file + memory) and cross-checks apsw |
| `schema.sql` | the two-table demo schema |
| `notes.md` | working notes / research log |

## Reproduce

```bash
python3 compare_all.py        # side-by-side of all approaches (apsw optional)
python3 test_provenance.py    # stdlib ctypes bridge vs apsw oracle, 10/10
python3 explain_provenance.py my.db "select id, name from t1"
python3 authorizer_lineage.py # authorizer dependency-set demo
```

Environment used: Python 3.11, SQLite 3.45.1 (system lib and stdlib both built
with `ENABLE_COLUMN_METADATA`), `apsw` 3.53 and `sqlglot` 30 for cross-checks.
