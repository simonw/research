# Notes: Mapping SQLite result columns back to source table columns

Goal: given an arbitrary SQL query run against SQLite from Python, figure out
which result columns correspond to which `table.column` in the source schema.

## Environment
- Python 3.11.15, stdlib sqlite3 module 2.6.0
- SQLite library version 3.45.1
- **Bundled SQLite compiled WITH `ENABLE_COLUMN_METADATA`** (confirmed via PRAGMA compile_options)
- apsw: NOT installed (will test pip)
- sqlglot: NOT installed (will test pip)

## Approaches to investigate
1. SQLite C "column metadata" API: sqlite3_column_origin_name / _table_name /
   _database_name. Requires SQLITE_ENABLE_COLUMN_METADATA. The stdlib sqlite3
   module does NOT surface these in cursor.description (only the column name).
   - Option A: access via APSW (exposes description_full)
   - Option B: access via ctypes directly against libsqlite3
2. Pure-Python static analysis with sqlglot's lineage/qualify (no execution).
3. EXPLAIN bytecode parsing (last resort, fragile).


## Approach 1 result: APSW `description_full` — WINNER for execution-based
`cursor.description_full` returns 5-tuples:
  (output_name, declared_type, database, table, origin_column)
When a column is a clean reference, table+origin are populated. When it's an
expression (||, function, literal, arithmetic) both are None.

Surprising power (all CORRECT, verified):
- `select *`          -> expands to real columns (t1.id, t1.name, ...)
- joins              -> each column attributed to its real table
- `as` aliases       -> traced back to true origin column
- expressions        -> (None, None) i.e. "no clean source" (exactly desired)
- SUBQUERIES         -> `select age from (select * from t2) s` -> t2.age  ✅
- CTEs               -> `with c as (select id,name from t1) select * from c` -> t1.id/t1.name ✅
- UNION              -> reports source from the first SELECT branch (t1.name) ✅
Why so powerful: the origin_name/table_name functions report the TRUE underlying
base-table column after SQLite resolves views/subqueries/CTEs internally.

Limitation found:
- AMBIGUOUS columns are a hard SQL error. `select id, ... from t1 join t2`
  where both have `id` -> SQLite raises "ambiguous column name: id" at prepare
  time (so you must disambiguate). This is the user's "harder still" example;
  it only works when the bare column exists in exactly one joined table.

## stdlib sqlite3 cannot do this directly
Python's stdlib sqlite3 cursor.description is name-only (other 6 fields always
None) and there is NO API to reach sqlite3_column_origin_name. So for stdlib
environments we drop to ctypes against libsqlite3 (system lib HAS metadata).

## Approach 2: pure-stdlib ctypes bridge (column_provenance.py) — RECOMMENDED
stdlib sqlite3 can't reach the metadata API, so we ctypes-load libsqlite3 and
call sqlite3_prepare_v2 + the *_origin_name/_table_name/_database_name funcs.
Two entry points:
- resolve_columns(db_path, sql): opens the file read-only, prepares (never
  steps), reads metadata.
- resolve_columns_for_connection(conn, sql): the "works with stdlib Connection"
  path. If main db is a file -> open the file. If :memory:/temp -> bridge via
  conn.serialize() (py3.11+) + sqlite3_deserialize into a private ctypes mem db.
Verified: matches apsw description_full 10/10 on the battery, in-memory + file.
Only prepares, so zero rows are read from the user's data.

## Approach 3: EXPLAIN bytecode parsing (explain_provenance.py) — pure stdlib, heuristic
Parse VDBE program from `EXPLAIN <sql>`:
- OpenRead p1=cursor p2=rootpage  -> cursor->table via sqlite_master.rootpage
- Column   p1=cursor p2=colidx p3=reg -> reg holds table.colidx
- Rowid    p1=cursor p2=reg       -> reg holds INTEGER PRIMARY KEY alias
- ResultRow p1=start p2=count     -> output regs start..start+count-1
Trace each output register's last writer. Column/Rowid off a base cursor =>
clean source; anything else (Concat/Function/String8/Count...) => expression.
Gotcha: the optimizer can route columns through OpenAutoindex / ephemeral
cursors. `PRAGMA automatic_index=OFF` fixes the common join case.
Result: agrees with metadata 9/10 (even subquery + CTE, which get flattened);
only UNION misses (compound co-routine -> ephemeral cursor). Good enough for
"simple queries", which was the brief.

## Approach 4: authorizer callback (authorizer_lineage.py) — pure stdlib
set_authorizer fires SQLITE_READ(table,column) per column access at PREPARE time.
Gives the SET of columns the query depends on (incl. WHERE/JOIN/ORDER and *
expansion). GREAT for column-level lineage / access control (DENY a column).
NOT a per-output mapping: reads != outputs once there's WHERE/JOIN/ORDER, and an
expression like name||'-suffix' just shows a read of t1.name with no transform
signal (naive_output_mapping gives a FALSE POSITIVE there). naive mapping only
trusted when #reads == #outputs (single table, no filter, no expr).

## Approach 5 (bonus): sqlglot static qualify — no execution, needs schema dict
qualify() resolves * and aliases, errors on ambiguous columns like SQLite.
Unique strength: attributes EXPRESSION columns to their INPUT columns
(name||'-suffix' -> "uses t1.name"), which the C API cannot. Downsides: extra
dependency, its own SQL parser/dialect quirks, must supply schema, no data-level
truth. Compljugate to the metadata approach for expression lineage.

## Bottom line / recommendation
- Best correctness with least code: APSW description_full (if you can add the dep).
- Best for stdlib-only: column_provenance.py (ctypes bridge) — robust, matches APSW.
- All-stdlib, no native calls: explain_provenance.py (simple queries) +
  authorizer_lineage.py (dependency set). 
- Want expression-input lineage or no execution at all: sqlglot.
