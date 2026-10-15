# sparqlite benchmark harness

The benchmark of docs/SPEC.md §12.4. It is written in Python: two deterministic data
generators, 17 benchmark queries, and a harness that times sparqlite-native,
sparqlite-baseline, pyoxigraph and rdflib on the same data and cross-checks every
result.

| Path | What it is |
|---|---|
| `py/gen_social.py` | *social* dataset generator (N-Triples, seeded) |
| `py/gen_bsbm.py` | *bsbm-lite* dataset generator (N-Triples, seeded) |
| `py/queries/q01-…q17-*.rq` | the benchmark queries, each with a small comment header |
| `py/bench.py` | the harness (a thin entry point to `bench_lib.cli`) |
| `py/bench_lib/` | harness library: `generation`, `terms`, `checksum`, `queries`, `datasets`, `engines/`, `runner`, `report`, `envinfo`, `cli`, `selftest` |
| `py/bench_lib/tests/` | unit tests, including a pyoxigraph-backed fake of the extension's SQL surface |

Everything runs with the project venv (Python 3.13, pyoxigraph 0.5.11, rdflib 7.6). Only the
standard library is used apart from those two packages.

```bash
PY=/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/venv/bin/python   # any Python >= 3.10 with pyoxigraph (+ rdflib) works
DATA=/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/data            # generated data never goes into the repo
```

## Quick start

```bash
# 1. Self-test: unit tests, then all 17 queries on tiny generated data with every engine
#    that is available (sparqlite is reported "unavailable" until ext/ is built).
$PY py/bench.py --self-test

# 2. Generate the datasets.
$PY py/gen_social.py --persons 200000 --out $DATA/social-200k.nt   # 1.6M triples
$PY py/gen_social.py --persons 625000 --out $DATA/social-625k.nt   # 5M triples
$PY py/gen_bsbm.py                     --out $DATA/bsbm-1m.nt       # ~1M triples
$PY py/gen_social.py --persons 10000  --out $DATA/social-10k.nt    # 80K triples (rdflib)

# 3. Build the extension and run everything.
CARGO_TARGET_DIR=$SCRATCH/targets/ext cargo build --release --manifest-path ext/Cargo.toml
export SPARQLITE_EXT=$SCRATCH/targets/ext/release/libsparqlite
$PY py/bench.py --social $DATA/social-200k.nt --bsbm $DATA/bsbm-1m.nt \
    --work-dir $SCRATCH/work/bench --out-md results.md --out-json results.json
```

The markdown table is printed to stdout and written to `--out-md`. The JSON with every raw
timing goes to `--out-json`. Both default to `<work-dir>/bench-results.{md,json}`.
Progress lines go to stderr. The exit status is 0 on success, and 2 for usage errors. It is
1 if engines disagree on a result (unless `--allow-mismatch`), or if an engine that is
present failed: in its probe, while loading a dataset, or on a query (unless
`--allow-errors`).

## Datasets

Both generators are deterministic: the same `--seed` (default 42) gives byte-identical
output on any CPython 3, because they draw only through `random.Random.random()` and
`shuffle`. Output is written to a temporary file and renamed into place. Shared helpers are
in `bench_lib/generation.py`.

### social (`gen_social.py --persons N [--seed S] [--cities 1000] --out F`)

Namespace `http://example.org/social/` (prefix `:`). Every person `:p{i}` (`i = 0..N-1`) has
exactly **8 triples**:

| Predicate | Object | Distribution |
|---|---|---|
| `rdf:type` | `:C{i % 10}` | 10 classes, 10% each |
| `:name` | `"Person {i}"` | simple literal |
| `:age` | `"{a}"^^xsd:integer` | 0–99: flat to 59, then exponential decay (about 0.7% are over 90) |
| `:knows` ×3 | `:p{j}` | 3 distinct targets, never `i`; in-degree follows a power law (exponent 0.8) over popularity ranks scattered by a seeded shuffle, so the top person gets ~1.4% of all edges |
| `:city` | `:city{k}` | Zipf (s = 1) over `--cities` (default 1000); `:city1` is the most populous |
| `:joined` | `"YYYY-MM-DD"^^xsd:date` | uniform over 2005–2024 |

The spec lists five properties plus three `:knows` and asks for about 8 triples per person
(1.6M at 200K persons, 5M at 625K), so the eighth triple is `:joined`, an inline `xsd:date`.
Because every out-degree is exactly 3, chain-2 (Q3) always has `9 × N` solutions.

### bsbm-lite (`gen_bsbm.py --products P [--seed S] --out F`)

A scaled-down Berlin SPARQL Benchmark shape. Vocabulary `http://example.org/bsbm/vocabulary/`
(`bsbm:`), instances `http://example.org/bsbm/instances/` (`inst:`), plus rdfs, foaf, dc and rev.

- **Product types:** a 4-level hierarchy under `bsbm:Product` with branching 3×4×4×5, giving
  303 types linked by `rdfs:subClassOf`. Products are typed with a leaf type (90%) or a
  level-3 type (10%) only, so Q14 needs the closure.
- **Products** (P): label, `@en` comment, producer, 4–10 features (Zipf popularity over
  `max(32, P/4)` features), integer `productPropertyNumeric1..3` (1–2000; the 2nd and 3rd are
  optional), textual properties, and a publication `xsd:date`.
- **Producers** (`P/40`) and **vendors** (`P/80`): label, country, homepage.
- **Offers** (`10 P`): product, vendor, price, `validFrom`/`validTo` `xsd:dateTime` (2008, no
  time zone, so every comparison is defined), and delivery days.
  - 85% of prices are `xsd:decimal` with exactly two decimals. About 9% of those are
    non-canonical, like `"12.50"`, which sparqlite stores as a dictionary TYPED literal and
    not as an inline DECIMAL.
  - The other 15% are `xsd:integer`.
  - Decimal cents are never `00`, so a decimal price never equals an integer price, and
    `ORDER BY ?price ?offer` is a total order on every engine.
- **Reviews** (`5 P`, power-law over products): reviewer (`P/2` reviewers), title, review
  `xsd:dateTime`, and ratings 1–10 (each of four present with probability 3/4).

The default of `--products 7800` gives 1,004,103 triples.

### Measured on this machine (4 vCPU Xeon @ 2.8 GHz)

| Command | Triples | Size | SHA-256 (prefix) | Time |
|---|---:|---:|---|---:|
| `gen_social.py --persons 200000` | 1,600,000 | 175.0 MB | `b0c8f3a5adeb` | 3.3–3.8 s |
| `gen_social.py --persons 625000` | 5,000,000 | 549.8 MB | `f55718e7a0c0` | 13.0 s |
| `gen_bsbm.py` (7800 products) | 1,004,103 | 146.9 MB | `6586dcc57a51` | 2.5–3.1 s |
| `gen_social.py --persons 10000` | 80,000 | 8.6 MB | `a6bc6da36ce2` | 0.1–0.3 s |

The SHA-256 prefixes are those of the default seed (42). The benchmark records the full
content hash of every dataset, so a run elsewhere can be checked against these.

## Queries

One query per file in `py/queries/`. The header is made of ordinary SPARQL comments, so each
file is sent to every engine unchanged:

```
# Q8: GROUP BY + COUNT + ORDER BY DESC LIMIT 10      <- id and title (first line)
# dataset: social                                    <- social | bsbm
# ordered: true                                      <- the ORDER BY is total: compare the sequence too
# free-form description ...
PREFIX : <http://example.org/social/>
SELECT ...
```

All queries use explicit `PREFIX` declarations and only standard SPARQL 1.1. pyoxigraph,
rdflib and the spec's parser (spargebra) all accept them. Every `ORDER BY … LIMIT` has a
tie-breaker, so the selected rows are the same on every engine.

| # | Data | Shape | Rows at default size |
|---|---|---|---:|
| Q1 | social | star-4: `?x a :C3; :age ?a; :city ?c; :name ?n` | 20,000 |
| Q2 | bsbm | product star-6 anchored on `inst:ProductFeature3` | 1,606 |
| Q3 | social | chain-2 `?a :knows ?b . ?b :knows ?c`, COUNT | 1 (1,800,000) |
| Q4 | social | chain-3 from `:p123` | 27 |
| Q5 | social | `?x a :C3 OPTIONAL { ?x :knows ?y . ?y a :C4 }` | 20,634 |
| Q6 | social | `?x :age ?a FILTER(?a > 90)` | 1,393 |
| Q7 | social | `REGEX(?n, "^person 1[0-9]*7$", "i")` | 11,111 |
| Q8 | social | `GROUP BY ?c` COUNT `ORDER BY DESC(?n) ?c LIMIT 10` (ordered) | 10 |
| Q9 | social | `ORDER BY DESC(?age) ?x LIMIT 10` (ordered) | 10 |
| Q10 | social | `DISTINCT ?c ?cls` over a 3-pattern join | 9,999 |
| Q11 | social | UNION of outgoing (`:C1`) and incoming (`:C2`) `:knows` | 127,794 |
| Q12 | social | `?x a :C5 MINUS { ?x :knows ?y . ?y a :C5 }` | 14,877 |
| Q13 | social | `:p1 :knows+ ?y`, COUNT | 1 (103,961) |
| Q14 | bsbm | `?x rdf:type/rdfs:subClassOf* bsbm:Product` | 7,800 |
| Q15 | social | point lookup `:p123 ?p ?o` | 8 |
| Q16 | social | `COUNT(*) { ?s ?p ?o }` | 1 (1,600,000) |
| Q17 | bsbm | offers with price in [500, 1000], `validTo >= 2008-06-01`, `ORDER BY ?price ?offer LIMIT 10` (ordered) | 10 |

The row counts are from the validation run at 200K persons and 7800 products; the value of
each COUNT is in parentheses. Q5's count comes from a direct computation over the file,
because pyoxigraph does not finish it.

### Reference timings (pyoxigraph and rdflib, validation runs of 2026-10-15)

These were measured before the extension existed, with pyoxigraph 0.5.11 in its isolated
worker and the default modes. Each cell is the median ms of 5 runs after 1 warm-up. The
data was social with 200K persons (1.6M triples) and bsbm-lite with 7800 products (1.0M
triples). The timeout was 300 s. Loading took 8.0 s (social) and 4.3 s (bsbm).

The machine was shared with concurrent cargo builds and other benchmark runs on 4 vCPUs,
so repeated runs differ by up to about ±30%. For example, Q1 full took 256 ms in one run
and 223 ms in the next, and loading social took 14.7 s and then 8.0 s.

The `count` column is the default engine-side count. `count, iterate` is
`--reference-count iterate` (the spec's `len(list(q))`), from a separate run made right
after it. The difference is the cost of building every solution and term in Python. It is
up to about 2× on queries with many result rows (Q11 1.95×, Q2 1.7×, Q1 1.55×), and lost
in the noise on the others.

| # | count | count, iterate | full | json |
|---|---:|---:|---:|---:|
| Q1 | 101 | 157 | 223 | 188 |
| Q2 | 16.3 | 27.8 | 27.9 | 38.7 |
| Q3 | 2,351 | 2,358 | 2,322 | 2,572 |
| Q4 | 0.21 | 0.19 | 0.19 | 0.17 |
| Q5 | timeout (> 300 s) | | | |
| Q6 | 148 | 116 | 140 | 113 |
| Q7 | 121 | 171 | 159 | 156 |
| Q8 | 173 | 146 | 176 | 128 |
| Q9 | 1,951 | 1,907 | 2,113 | 2,113 |
| Q10 | 3,085 | 3,082 | 3,035 | 3,111 |
| Q11 | 195 | 380 | 613 | 458 |
| Q12 | 201 | 186 | 250 | 218 |
| Q13 | 421 | 469 | 499 | 482 |
| Q14 | 150 | 199 | 159 | 153 |
| Q15 | 0.10 | 0.08 | 0.09 | 0.09 |
| Q16 | 486 | 507 | 544 | 469 |
| Q17 | 78.0 | 78.4 | 75.3 | 102 |

Q5 is super-linear in pyoxigraph. Its OPTIONAL takes 1.0 s at 10K persons and 6.5 s at
20K, so pyoxigraph never finishes it at 200K.

On social with 10K persons (80K triples), pyoxigraph and rdflib agree on every query in all
three modes, with every execution checked.

- rdflib loads the data in 3.0 s, pyoxigraph in 0.3 s.
- rdflib's count-mode times range from 3.9 ms (Q15) to 26.3 s (Q5).
- Its Q12 MINUS takes 10.3 s.
- In count mode, pyoxigraph is 12–1,900× faster on that data.

## Engines and modes

`--engines` takes any of `native`, `baseline`, `pyoxigraph`, `rdflib` (or `all`). The
default is all four. `--modes` takes any of `count`, `full`, `json`; the default is all
three, so a default run times and cross-checks both of the spec's baseline calls
(`sparql_count_sql` in count mode, `sparql_query_sql` in json mode). When a run leaves json
out, its settings line says that `sparql_query` and `sparql_query_sql` were not measured.

| Engine | `count` | `full` | `json` |
|---|---|---|---|
| sparqlite-native | `SELECT sparql_count(?)` | `SELECT t0..t{n-1} FROM sparql_rows(?)`, fetched completely | `SELECT sparql_query(?, 'json')` |
| sparqlite-baseline | `SELECT sparql_count_sql(?)` | `SELECT t0..t{n-1} FROM sparql_rows(?, '{"engine":"sql"}')`, fetched completely | `SELECT sparql_query_sql(?, 'json')` |
| pyoxigraph | `Store.query(SELECT (COUNT(*) AS ?n) { q })` | `str()` of every projected term | `.serialize(QueryResultsFormat.JSON)` |
| rdflib | `Graph.query(SELECT (COUNT(*) AS ?n) { q })` | N-Triples text of every projected term | `.serialize(format='json')` |

Each mode does the same work on every engine, so the speed-ups compare engines rather than
fetch shapes:

- **count** runs the query to completion and counts its solutions inside the engine,
  without decoding any result term or building a Python object per solution.
  - That is what `sparql_count` does (§8.2: "no decode and no serialization").
  - pyoxigraph and rdflib therefore run the query wrapped as a subquery,
    `<prologue> SELECT (COUNT(*) AS ?bench_count) WHERE { <the original SELECT> }`
    (`bench_lib.queries.count_query`). The PREFIX/BASE prologue is kept. `COUNT(*)` counts
    every solution, duplicates and all-unbound ones included, so the value equals the
    number of rows. The count variable is renamed if the query already uses it. A unit
    test checks the rewrite against the plain query on every benchmark query.
  - Counting by iterating `Store.query(q)` instead makes pyoxigraph decode every term and
    build a `QuerySolution` per row. In review that took up to about 2× as long on the
    benchmark queries at 200K persons (Q11: 363 vs 174 ms), so the count table would
    overstate sparqlite's lead by as much.
  - `--reference-count iterate` measures the spec's literal `len(list(q))` instead. The
    count table's title then says that pyoxigraph and rdflib build every solution and that
    `×o` is not like-for-like.
  - ASK is evaluated and counts as 1. CONSTRUCT/DESCRIBE (not in the benchmark) and a
    SELECT with `FROM` (a subquery cannot carry a dataset clause) are counted by iterating.
- **full** builds exactly one Python string per projected term.
  - `n` is the number of projected variables. It is parsed from the query
    (`BenchQuery.projection`), and a unit test checks it against pyoxigraph for every
    benchmark query.
  - ASK fetches `v0` and CONSTRUCT/DESCRIBE fetch `t0, t1, t2`. A `SELECT *` (not in the
    benchmark) falls back to `vars, t0..t15`.
  - The baseline uses the same `sparql_rows` output path through the §8.3 `engine` option.
    §9 says the translator shares storage, ids, dictionary and output path, so the two
    sparqlite columns differ only in evaluation strategy.
- **json** produces one serialised document per execution: SPARQL-JSON, or N-Triples for
  graph forms. The spec's `sparql_query_sql` is measured here, against `sparql_query`.

`--full-columns star` (alias `--native-full star`) switches both SQLite engines to the
literal §12.4 wording, `SELECT * FROM sparql_rows(?)`. That decodes all 51 columns per row
(`n`, `v*`, `t*`, `id*`, `vars` and the per-row `json`) and builds 51-element tuples, while
pyoxigraph still builds one string per term. The full-mode table title then says so and
warns that `×o` is not like-for-like.

How large the difference is was measured in review with a plain SQLite table shaped like
Q11's 127,794 two-variable rows:

| Fetch | Time |
|---|---:|
| `SELECT *` | 908 ms |
| `vars, t0..t15` | 352 ms |
| `t0, t1` | 149 ms |
| one JSON text | 32 ms |

pyoxigraph's whole Q11 full run is about 640 ms.

- **SQLite engines.** Both run on one file database per dataset, built once by whichever of
  them opens first. The other one's load column shows "(same DB as …)".
  - Setup: `PRAGMA page_size=8192`, `journal_mode=DELETE`, and `cache_size=-524288`, all
    before `SELECT sparql_init()`.
  - Load: `SELECT sparql_load_file(path, 'ntriples')` after
    `SELECT sparql_config('allow_files', 1)`. If `sparql_load_file` is not registered, or
    with `--sqlite-load blob`, it uses `SELECT sparql_load(<file bytes>, 'ntriples')`.
  - Then `ANALYZE main` (so the baseline's SQL gets `sqlite_stat1`) and
    `SELECT sparql_analyze()`. Each phase is timed separately.
  - Query connections are separate autocommit connections with the same `cache_size`, so
    every call gets the extension's own read scope (§8.6).
  - The database is `<work-dir>/<dataset>-<sha256 prefix>.db`, with a `.json` sidecar.
    `--reuse-db` skips the load when the sidecar matches the dataset's content SHA-256 and
    the extension file's SHA-256.
- **Native options.** `--native-config KEY=VALUE` (repeatable) runs
  `sparql_config(KEY, VALUE)` on the native connection, for example `join_policy=hash` or
  `reorder=0`. Phase 2's `native+btree` is `--native-config reader=btree`.
  - A non-default configuration is part of what the column measures, so the column heading
    says so, for example `native[reader=btree]`.
  - The configuration is also written to the settings line and to the JSON.
- **pyoxigraph** uses an in-memory `Store` with `bulk_load(path=…)`.
- **rdflib** uses an in-memory `Graph` and runs only on datasets with at most
  `--rdflib-max` (100,000) triples. Larger datasets are reported as "skipped".
- **Process isolation.** pyoxigraph and rdflib run in a child process
  (`bench_lib/engines/isolated.py`) that holds the store.
  - The child times each execution itself, the same way the runner does, so pickling and
    IPC are not measured. It frees each result right after sending it, so freeing never
    lands in the next execution's timing.
  - At `--timeout` the parent kills the child; the next query starts a new one and reloads
    the dataset.
  - Without this, a pathological query cannot be stopped: pyoxigraph's OPTIONAL on Q5 is
    super-linear (1.0 s at 10K persons, 6.5 s at 20K).
  - `--no-isolate` runs them in process instead.

### Unavailable, unsupported or error

Until the extension is built, the sparqlite engines must sit out quietly. Once it exists,
any failure of something that is there must fail the run. The harness therefore keeps
**unavailable** for things that do not exist.

The extension is probed once in `:memory:`. Every call the adapters use is made on a
one-triple store: `sparql_version`, `sparql_init`, `sparql_config`, `sparql_load`,
`sparql_analyze`, `sparql_count`, `sparql_rows`, `sparql_rows(…, '{"engine":"sql"}')`,
`sparql_query`, `sparql_count_sql` and `sparql_query_sql`.

- **Unavailable** means one of:
  - the library is missing or cannot be loaded;
  - `sparql_init` is not registered;
  - neither loader is registered;
  - for a single mode, its function or table is not registered: `no such function`,
    `no such table: sparql_*` or `no such module`.
- **Error** covers everything that is registered but fails:
  - A call that fails in the probe leaves its mode measured, so the failure shows up as
    error cells on the real queries instead of disappearing.
  - `sparql_init()` failing makes the engine an error.
  - The baseline rejecting the probe's one-pattern BGP, which is inside every subset,
    makes the engine an error.
  - Building the database, or configuring the connection (for example a bad
    `--native-config` key), failing is a load error. Every cell of that engine is then an
    error cell (`not run: …`).

At query time, SQLite errors are classified:

| Error | Recorded as |
|---|---|
| `sparql: unsupported: …` from the **baseline** (its §9 subset) | **unsupported** |
| `sparql: unsupported: …` from **native** (it implements all of SPARQL 1.1) | **error** |
| any error after the watchdog interrupted the execution at `--timeout` | **timeout** |
| `sparql: query timeout` (the extension's own `timeout_ms`, e.g. `--native-config timeout_ms=…`) | **timeout** |
| `interrupted` when no deadline passed | **error** |
| `no such function` / `no such table: sparql_*` / `no such module` | **unavailable** |
| anything else | **error** (with the message) |

The prefix match is exact: sq-core renders `Error::Unsupported` as `sparql: unsupported: …`,
and the extension must pass that text through `sqlite3_errmsg` unchanged, both from scalar
functions and from the `sparql_rows` cursor.

The extension path comes from `--ext`, then `$SPARQLITE_EXT`, then
`ext/target/release/libsparqlite`. A path with or without `.so` works.

## Protocol

For each dataset, every engine loads it once. Load times are reported in their own table.
Then, for every query of that dataset and every (engine, mode) cell:

1. Run `--warmup` executions (default 1).
2. Run `--repeat` timed executions (default 5) and report the **median** wall-clock time
   (`time.perf_counter_ns`).
   - The Python GC is paused during each timed execution, as `timeit` does.
   - If the warm-up shows that the repetitions would exceed `--cell-budget` (default 120 s),
     the cell gets fewer repetitions, at least 1. A warm-up that alone exceeds the budget is
     taken as the only sample. Such cells are marked † in the table.
3. `--timeout` (default 600 s) bounds one execution.
   - SQLite engines are interrupted with `sqlite3_interrupt` by one long-lived watchdog
     thread per engine (`bench_lib/engines/watchdog.py`).
     - Arming and disarming it costs about 1 µs inside the timed region, and never starts
       or wakes a thread. Starting a `threading.Timer` per execution cost about 0.3 ms,
       more than pyoxigraph's whole Q15.
     - With an instantaneous SQL function, the harness adds 2–3 µs to the bare
       `conn.execute(…).fetchone()` call. A unit test fails above 50 µs.
     - The interrupt is delivered only while the same execution is still armed, so it can
       never hit the next one.
   - The Python engines' worker process is killed.
   - The cell becomes `timeout`, and the same engine's remaining modes for that query are
     recorded as timeouts without running them.
4. **Every execution's result is checked, outside the timing**, warm-up included. For the
   SQLite engines, the timed part is the SQL call plus `fetchall()` (full mode) or
   `fetchone()` (count and json).
   - The first execution (cold caches) and the last one are decoded into a row count, an
     order-insensitive checksum and, for `ordered` queries, a sequence checksum.
   - Every other execution's rows are counted (`Engine.row_count`: the count itself, the
     number of fetched rows, or the bindings of the JSON document).
   - If the executions disagree, the cell is an **error** ("results differ between
     executions") and the run fails. A wrong answer on cold caches, the shape of a §4.4
     decode/lookup-cache bug, cannot hide behind correct warm repetitions.
   - Each result is freed before the next timed call, never inside it. This holds in the
     isolated worker process too, which keeps no reference to a result once it has been
     sent.

The JSON file is rewritten after every query, so a crash still leaves the partial results.

### Cross-engine checks

Each result term is canonicalised (`bench_lib/terms.py`), because engines legitimately differ
in surface syntax:

- IRIs stay as they are.
- **Blank nodes map to one constant.** Labels cannot be compared across engines.
- Numeric literals are compared by value within their datatype family. All integer-derived
  types form one family; decimal, float and double are separate. pyoxigraph and rdflib
  canonicalise `"12.50"` to `"12.5"` and `"01"` to `"1"` on load, but sparqlite keeps the
  lexical form.
- Booleans, `dateTime`, `date` and `time` are normalised (trailing fraction zeros dropped,
  `+00:00` written as `Z`).
- `xsd:string` equals a simple literal.
- Language tags are compared case-insensitively.
- Ill-typed literals keep their exact lexical form.

A row is the canonical cells keyed by variable name, so column order does not matter.

- The **multiset checksum** is the row count, plus the 64-bit sum of BLAKE2b row hashes,
  plus a hash of the variable names.
- The **sequence checksum** hashes the rows in order.

After each query, all successful cells must agree on:

- row counts (`count` mode included);
- multiset checksums (all result modes);
- sequence checksums (`ordered` queries only).

If they do not, the run prints `!!! MISMATCH` and the table marks the cells that differ from
pyoxigraph (or from the majority) **MISMATCH**. The run then fails unless `--allow-mismatch`
is given. The "Result checks" table lists the status, rows and checksum of every query.

## Output

The markdown has these sections:

1. Environment line: CPU, memory, and versions of SQLite, Python, pyoxigraph, rdflib and
   sparqlite. It also gives the git revision, how many harness files differ from it, and
   the harness digest (below).
2. **Settings line.** Everything that changes what a column measures: modes (and a note
   when json mode is off), repetitions, timeout, cell budget, `--full-columns`,
   `--reference-count`, the native `sparql_config` (or "default"), the SQLite load path,
   isolation and `--rdflib-max`. It is followed by the command line.
3. **Datasets and load times.** Each dataset row shows its content SHA-256 prefix. The
   sparqlite cell gives the total time and the database size; the phases are listed below
   the table.
4. One table per mode. Rows are queries and columns are engines. A cell reads like
   `12.3 (3.10×b, 4.02×o)`: the median in ms, then the speed-up against the baseline (`×b`)
   and against pyoxigraph (`×o`). A speed-up is the other engine's time divided by this
   one's, so >1 means faster. Each table ends with geometric-mean speed-ups.
5. **Result checks** and **Notes**, which explain every unavailable, skipped, unsupported or
   error cell.

The JSON has these keys:

| Key | Contents |
|---|---|
| `environment` | timestamp, Python, platform, CPU model and count, memory, SQLite version, pyoxigraph/rdflib versions, and the extension's path, SHA-256 prefix, `sparql_version()` and probe results (per call, plus the `missing` ones); `git_revision`; `git_harness_changes` (the `git status --porcelain` lines of harness files that differ from it); `harness_sha256` |
| `config` | modes, warmup, repeat, timeout, cell budget; `engine_settings` (`full_columns`, `reference_count`, `native_config`, `sqlite_load`, `reuse_db`, `isolate`, `rdflib_max`, `timeout_s`, extension path, work dir); `argv` (the command line) |
| `engines` | name, column heading, status, version, message |
| `datasets` | path, triples, bytes, `sha256` of the contents (two machines can confirm they ran the same bytes) |
| `queries` | id, title, dataset, ordered, path, `sha256` and the full `text` exactly as sent to the engines |
| `loads` | per dataset and engine: seconds, triples, phases, database size |
| `measurements` | per dataset, query, engine and mode: status, `median_ms`, `times_ms`, `warmup_ms`, rows, checksum, sequence, message |
| `checks` | per query: status, rows, checksum, participating cells, details |

Everything that defines a measurement is pinned in the JSON:

- the data, by content SHA-256;
- the extension, by file SHA-256;
- each query, by its text and SHA-256;
- the harness, by `harness_sha256`. That is a SHA-256 over the paths and contents of
  `bench.py`, the generators, `bench_lib/` and `queries/`, so it works without git.

A run from a working tree with local edits is not described by the git revision alone, so
`git_harness_changes` lists the harness files that differ from it. It is gathered with
`git --no-optional-locks status`, which never rewrites the index that other processes in
the same tree may be using.

## Tests

```bash
$PY py/bench.py --self-test                                          # unit tests + tiny pipeline
cd py && $PY -m unittest discover -s bench_lib/tests -t . -v         # unit tests only
```

The unit tests (126) cover:

- term parsing and canonicalisation, including the cross-engine surface forms seen in
  practice (end-of-day `24:00:00` included);
- checksum properties;
- query headers, plus a lint that every `ORDER BY … LIMIT` query is marked `ordered`, and
  that the parsed projection equals pyoxigraph's for every query;
- the count rewrite: it keeps the prologue, avoids variable clashes, skips forms it cannot
  wrap, and counts exactly the rows of every benchmark query;
- generator determinism and distributions;
- the runner's protocol and every status, including results that differ between the
  executions of one cell and results freed outside the timing;
- process isolation (identical results, kill-and-restart at the timeout, errors crossing the
  process boundary);
- the markdown and JSON output, including the recorded settings, the count-mode variant,
  and the query and harness pins;
- the watchdog (no thread per execution, one interrupt per overrun, never a late one);
- the engine adapters, including `row_count` agreeing with `decode` in every mode, and
  both count variants agreeing on every query;
- the review regressions:
  - `test_review.py`: the timing overhead of the timeout guard, load and probe failures
    that must fail the run, native "unsupported" as an error, the recorded settings, and
    end-of-day dateTimes;
  - `test_review_r2.py`: count mode not materialising terms on pyoxigraph, the isolated
    worker freeing results outside the timing, every repetition being checked, and the
    JSON pinning the queries.

The sparqlite adapters are tested against `bench_lib/tests/fake_sparqlite.py`. It is a
pyoxigraph-backed stand-in for a `sqlite3` connection with the extension loaded, and it
returns exactly the shapes of §8.2/§8.3 (`sparql_rows` with `n, v0..v15, t0..t15,
id0..id15, vars, json`). Python's `sqlite3` cannot define virtual tables, so this is how the
adapters get tested before the real extension exists. The tests check:

- the database build order (PRAGMAs before `sparql_init`, then load, `ANALYZE` and
  `sparql_analyze`);
- file and blob loading, and database reuse;
- mode-level degradation, only for calls that are not registered;
- load, configuration and probe failures being errors;
- the baseline's "unsupported" path, and native "unsupported" being an error;
- the full-mode column lists (`t0, t1` for Q15, the baseline through `{"engine":"sql"}`);
- error classification and interrupts;
- that native and baseline results match pyoxigraph's on every query.

The self-test additionally runs all 17 queries in all three modes with every available
engine on 400 persons and 40 products. It requires that every query returns rows and that
at least two engines agree on each one.
