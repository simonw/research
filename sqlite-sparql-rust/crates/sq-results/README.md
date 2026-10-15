# sq-results

`sq-results` turns query results into bytes and SQL values. It writes SELECT and ASK results
in the four SPARQL 1.1 result formats (JSON, XML, CSV, TSV) through `sparesults`. It writes
CONSTRUCT and DESCRIBE results as N-Triples, N-Quads or Turtle through `oxrdfio`. It also
holds the native SQL value mapping of SPEC §8.4 (`sql_native`) and a per-row SPARQL-JSON
encoder (`solution_json`). The writers read rows only through the `SolutionSource` and
`TripleSource` traits in `sq-core`. Because of that, the native executor and the baseline SQL
translator share one output path, and this crate does not depend on either. It is small:
about 600 lines of non-test code. The rest of the roughly 1,300 lines in `src/` are tests.

## Where it fits

```
 sq-core: SolutionSource, TripleSource, Term, TermId, BATCH_ROWS
 sparesults =0.3.4 (SELECT/ASK)   oxrdfio =0.2.6 (graphs)   oxrdf =0.3.4 (borrowed term views)
                \                        |                        /
                                     sq-results
                /                        |                        \
 sq-engine                       sq-api                              tools/sq-w3c
 write_results, query_to_string  sparql_query / sparql_query_sql,    (optional, "mem" feature:
                                 sparql_rows `json` column,           ResultFormat for
                                 §8.4 native values (value.rs)        sq_engine::query_to_string)
```

The sources that the writers consume are implemented elsewhere. `sq_exec::Execution`
implements both `SolutionSource` and `TripleSource`. In `sq-api`, `BaselineRun` (the `sq-sqlgen`
path) implements `SolutionSource` only. The baseline path has no `TripleSource`:
`sq_sqlgen::translate` rejects CONSTRUCT and DESCRIBE, and `sq-api`'s `Run::triples` has the same
guard. So `sparql_query_sql` on a CONSTRUCT or DESCRIBE query returns `Error::Unsupported`
before any `sq-results` writer runs. The default format for each query form is chosen by
`sq-api`, not by this crate: `Form::default_format` gives JSON for SELECT and ASK, and
N-Triples for CONSTRUCT and DESCRIBE. `sq-engine` always requires an explicit format.

## Key concepts

**1. Pull, batch, decode once.** `write_solutions` calls `SolutionSource::next_rows(&mut ids,
BATCH_ROWS)`, where `BATCH_ROWS` is 1024. Each row is `variables().len()` `TermId`s, and an
unbound value is `TermId::UNBOUND`, which is 0. The writer then makes one `decode` call for the
whole batch and serializes straight from the decoded `Arc<Term>`s. `write_triples` does the same
with `next_triples` and `[TermId; 3]`. The `ids` and `terms` vectors are allocated once and
reused. `write_solutions` and `write_triples` put a 64 KiB `BufWriter` (`OUTPUT_BUFFER_BYTES`)
in front of the caller's writer, because sparesults and oxrdfio issue many small writes.
`write_boolean` writes its short document unbuffered and then flushes the caller's writer. The
tests check the call pattern: 2 × 1024 + 17 rows take 4 `next_rows` calls (the last one returns
0) and 3 `decode` calls.

**2. Borrowed oxrdf views (`convert.rs`).** Four crate-private helpers (not public API) borrow
decoded terms as oxrdf views without copying: `term_ref(&Term) -> TermRef`,
`literal_ref(&Literal) -> LiteralRef`, and the fallible `subject_ref(&Term) ->
Result<NamedOrBlankNodeRef>` and `predicate_ref(&Term) -> Result<NamedNodeRef>`, which return
`Error::Internal` for a literal subject or a non-IRI predicate. They use oxrdf's
`*_unchecked` constructors, so IRIs, blank node labels and language tags are trusted rather than
validated. These terms come from the store dictionary, the codec or the execution's `TempDict`.
A literal with a language tag becomes a language-tagged literal. A literal whose datatype is
`xsd:string` becomes a simple literal. Any other literal becomes a typed literal.

**3. One format enum, two families.** `ResultFormat` covers both result families. Each writer
checks the family before it reads anything from the source, and a format from the wrong family
returns `Error::Invalid`. `solutions_format()` and `graph_format()` map a format to the
sparesults or oxrdfio format, or to `None`. They return `Option<sparesults::QueryResultsFormat>`
and `Option<oxrdfio::RdfFormat>`, and `sq-results` re-exports neither crate. A caller that uses
these methods needs its own `sparesults` or `oxrdfio` dependency, at the workspace pins
`=0.3.4` and `=0.2.6`.

| Format | `parse` names (ASCII case and surrounding whitespace ignored) | `media_type()` |
|---|---|---|
| `Json` | `json` | `application/sparql-results+json` |
| `Xml` | `xml` | `application/sparql-results+xml` |
| `Csv` | `csv` | `text/csv; charset=utf-8` |
| `Tsv` | `tsv` | `text/tab-separated-values; charset=utf-8` |
| `NTriples` | `ntriples`, `nt` | `application/n-triples` |
| `NQuads` | `nquads`, `nq` | `application/n-quads` |
| `Turtle` | `turtle`, `ttl` | `text/turtle` |

What the golden tests pin down for each format:

- JSON and XML are compact, with no indentation. An unbound variable is left out of its row.
- CSV uses CRLF line endings. Values are plain: an IRI without `<>`, a blank node as `_:b1`, and
  a literal as its lexical form only, as the SPARQL CSV format specifies.
- TSV has a `?s\t?o` header and writes terms in N-Triples or Turtle syntax, so an `xsd:integer`
  appears as a bare `42`.
- An ASK result in CSV or TSV is the bare word `true` or `false`, with no header and no trailing
  newline. This is how sparesults writes it.
- N-Quads output is the triples in the default graph, byte for byte the same as N-Triples.
- Turtle always declares the prefixes `rdf`, `rdfs` and `xsd` (`TURTLE_PREFIXES`), and lets
  oxrdfio group subjects with `;` and write `a` for `rdf:type`.

**4. The source contract is enforced, not assumed.** The writers handle several cases
explicitly:

- In `write_solutions` only, the internal `TermId::ERROR` group-key sentinel is rewritten to
  `UNBOUND` before `decode`, so an error-valued GROUP BY key is written as unbound (SPEC §3 tag
  table, §5.3). `write_triples` passes ids to `decode` unchanged.
- When there are zero projected variables (`SELECT * {}`), the writer writes one empty solution
  per row and makes no `decode` call.
- These contract violations return `Error::Internal`:
  - `ids.len() != rows * width` after `next_rows`;
  - `next_triples` returning a count that differs from the number of triples it appended;
  - a `decode` call that returns the wrong number of terms;
  - a triple with an unbound position, a literal subject or a non-IRI predicate;
  - a projected variable name that `oxrdf::Variable::new` rejects.
- Errors from the source (for example `Error::Interrupted`) are passed on unchanged.
- An I/O error becomes `Error::Internal("I/O error while writing results: …")`.
- One part of the contract is assumed, not checked. The `TripleSource` trait doc in `sq-core`
  says a source yields a set of triples, already deduplicated, in first-seen order.
  `write_triples` does no deduplication and writes duplicates as given, so a new `TripleSource`
  or a test fake must deduplicate itself.

**5. Per-row SPARQL-JSON (`json.rs`).** `sparql_rows` has a `json` column (SPEC §8.3), and
building a full sparesults document for every row would be wasteful. So `solution_json`,
`append_solution_json` and `append_term_json` encode one bindings object by hand:

- The object has the same shape as one element of `results.bindings`, with `type` set to
  `uri`, `bnode` or `literal`.
- A language-tagged literal gets `xml:lang`. A typed literal gets `datatype`, except for
  `xsd:string`, which gets neither.
- An unbound variable is left out.
- Strings are escaped the same way as in sparesults: `"` and `\`, then `\b \f \n \r \t`, then
  `\u00xx` for the other C0 controls. Every other character, including `/`, is written as it is.

`sq-api` calls `append_solution_json` with a reused `String` for each row.

**6. Native SQL values (`native.rs`, SPEC §8.4).** `sql_native(Option<&Term>) -> SqlNative`
returns `Null`, `Int(i64)`, `Real(f64)` or `Text(String)`:

- An IRI becomes TEXT without `<>`.
- A blank node becomes `'_:label'`.
- Language-tagged literals, simple literals and literals with a datatype outside the XSD
  namespace become their lexical TEXT.
- The 13 integer types (`integer`, `long`, `int`, `short`, `byte`, the `unsigned*` types and
  the `*Integer` sign types) become INTEGER when the value parses as an `i64` and lies within
  the type's range. Otherwise they become TEXT.
- `decimal`, `float` and `double` become REAL. For `float` and `double`, `INF`, `+INF`, `-INF`
  and `NaN` are accepted, and `inf`, `Infinity` and `nan` are not. `decimal` accepts only an
  optional sign and digits with an optional `.` fraction (`[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)`, no
  exponent), so `"INF"^^xsd:decimal` and `"1e5"^^xsd:decimal` become TEXT.
- A `boolean` of `true` or `1` becomes 1, and `false` or `0` becomes 0.
- Every other type becomes lexical TEXT, including dates, times and durations.

Lexical forms are matched exactly, without collapsing whitespace, so `" 1"^^xsd:integer`
becomes TEXT. An ill-typed literal, such as `"300"^^xsd:byte` or `"1e5"^^xsd:decimal`, becomes
its lexical TEXT. A float is converted from its lexical form at double precision, so
`"0.1"^^xsd:float` gives `0.1`.

## Public API tour

Everything is re-exported from `lib.rs`. The usage snippets below, but not the signature list
or the `sq-engine` excerpt, were compiled and run against the crate.

```rust
use sq_results::ResultFormat;

let fmt: ResultFormat = "TTL".parse()?;           // FromStr = ResultFormat::parse
assert_eq!(fmt, ResultFormat::Turtle);
assert_eq!((fmt.name(), fmt.media_type()), ("turtle", "text/turtle"));
assert!(fmt.is_graph_format() && !fmt.is_solutions_format());
assert!(ResultFormat::parse("rdfxml").is_err());   // Error::Invalid; ResultFormat::ALL lists all 7
```

The writers take `&mut dyn SolutionSource` or `&mut dyn TripleSource`, a `ResultFormat` and a
`&mut dyn std::io::Write`:

```rust
pub fn write_solutions(src: &mut dyn SolutionSource, fmt: ResultFormat, w: &mut dyn Write) -> Result<()>;
pub fn write_boolean(b: bool, fmt: ResultFormat, w: &mut dyn Write) -> Result<()>;
pub fn write_triples(src: &mut dyn TripleSource, fmt: ResultFormat, w: &mut dyn Write) -> Result<()>;
```

The following source is a minimal one, modelled on the fakes in `src/tests.rs` and
`tests/review.rs`. The writers never look at an id's tag. They only compare ids with `UNBOUND`
and `ERROR`, and pass them back to `decode`.

```rust
use std::sync::Arc;
use sq_core::eval_api::SolutionSource;
use sq_core::{Result, Tag, Term, TermId};
use sq_results::{write_boolean, write_solutions, ResultFormat};

struct One { vars: Vec<String>, term: Arc<Term>, done: bool }

impl SolutionSource for One {
    fn variables(&self) -> &[String] { &self.vars }
    fn next_rows(&mut self, out: &mut Vec<TermId>, _max_rows: usize) -> Result<usize> {
        if std::mem::replace(&mut self.done, true) { return Ok(0); }   // 0 = exhausted
        out.push(TermId::from_dict(1, Tag::Iri));                      // width ids per row
        Ok(1)
    }
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        out.extend(ids.iter().map(|id| (*id != TermId::UNBOUND).then(|| self.term.clone())));
        Ok(())
    }
}

let mut src = One { vars: vec!["s".into()], term: Arc::new(Term::Iri("http://ex/a".into())), done: false };
let mut out = Vec::new();
write_solutions(&mut src, ResultFormat::Csv, &mut out)?;
assert_eq!(out, b"s\r\nhttp://ex/a\r\n");

let mut out = Vec::new();
write_boolean(true, ResultFormat::Json, &mut out)?;
assert_eq!(out, br#"{"head":{},"boolean":true}"#);
```

Callers pick a writer from the query form after their own format check. `sq-api`'s
`write_output` (`crates/sq-api/src/query.rs`) does this, and so does `sq_engine::write_results`
(`crates/sq-engine/src/query.rs`). This excerpt from the latter is abridged:

```rust
match kind {
    FormKind::Select => sq_results::write_solutions(exec, fmt, w),
    FormKind::Ask => { let b = exec.ask()?; sq_results::write_boolean(b, fmt, w) }
    FormKind::Graph => sq_results::write_triples(exec, fmt, w),
}
```

Per-row JSON and native values:

```rust
use std::sync::Arc;
use sq_core::{Literal, Term};
use sq_results::{append_term_json, solution_json, sql_native, SqlNative};

let vars = vec!["s".to_string(), "n".to_string()];
let row = vec![Some(Arc::new(Term::Iri("http://ex/a".into()))), None];
assert_eq!(solution_json(&vars, &row), r#"{"s":{"type":"uri","value":"http://ex/a"}}"#);
// append_solution_json(&mut buf, &vars, &row) does the same into a reused String.

let mut t = String::new();
append_term_json(&mut t, &Term::Literal(Literal::lang("chat", "FR")));   // tags are lower-cased
assert_eq!(t, r#"{"type":"literal","value":"chat","xml:lang":"fr"}"#);

let xsd = |l: &str| format!("http://www.w3.org/2001/XMLSchema#{l}");
assert_eq!(sql_native(Some(&Term::Literal(Literal::typed("42", &xsd("integer"))))), SqlNative::Int(42));
assert_eq!(sql_native(Some(&Term::Literal(Literal::typed("300", &xsd("byte"))))), SqlNative::Text("300".into()));
assert_eq!(sql_native(Some(&Term::BNode("b1".into()))), SqlNative::Text("_:b1".into()));
assert_eq!(sql_native(None), SqlNative::Null);
```

`sq-api` (`src/value.rs`) wraps `sql_native`. It handles IRIs, simple literals and
language-tagged literals itself, borrowing their text, so that the common cases avoid the
`String` allocation in `SqlNative::Text`.

## Testing

```sh
export CARGO_TARGET_DIR=/path/to/target CARGO_INCREMENTAL=0
cargo test -p sq-results                  # unit tests + tests/review.rs
cargo test -p sq-results --lib            # the 31 unit tests only
cargo test -p sq-results --test review    # the adversarial review test only
```

There are no `#[ignore]`d tests or benchmarks, and the tests read no environment variables. The
dev-dependencies are `serde_json`, used to check JSON escaping and that the JSON forms are
equivalent, and `oxttl`, used for a Turtle round trip. `scripts/check.sh` runs the crate's tests
as part of `cargo test --workspace --release`.

The 31 unit tests are spread over the source files:

- **`src/tests.rs`** (17 tests):
  - golden JSON, XML, CSV and TSV output for 5 rows that cover every term kind, an unbound value
    and characters that need escaping (`"`, tab, newline, `<&>`);
  - golden ASK output in all four formats;
  - golden N-Triples, N-Quads and Turtle output, and a Turtle round trip through `oxttl`;
  - a check that `solution_json` equals the JSON writer's bindings;
  - `next_rows` and `decode` call counts for solutions, and the `decode` call count for
    triples;
  - zero-width rows, and the ERROR sentinel written as unbound;
  - source errors passed on unchanged, and contract violations returned as `Internal`;
  - formats from the wrong family returned as `Invalid`;
  - invalid triples.
- **`src/native.rs`** (5 tests): the §8.4 mapping table (28 cases), NaN, 20 ill-typed lexical
  forms, accepted min/max bounds for several derived integer types, and `Arc<Term>` arguments.
- **`src/json.rs`** (4 tests): every term kind, empty rows and rows with every variable unbound,
  escaping (byte-exact, and round-tripped through `serde_json`), and appending to an existing
  buffer.
- **`src/format.rs`** (3 tests): every alias in any case, unknown names (`rdfxml`, `jsonld`,
  `n-triples`, `srj`, empty), and name round trips with the two families never overlapping.
- **`src/convert.rs`** (2 tests): the borrowed views equal `Term::to_oxrdf`, and the subject
  and predicate position checks.

`tests/review.rs` contains one adversarial test from the review round. **It fails at the time
of writing**, which also makes `cargo test --workspace` fail. See the first item under
Limitations. That command is the first step of `scripts/check.sh`, which runs under
`set -euo pipefail`, so the script stops there and runs none of its later gates (extension
build, SQLite API checks, pytest, W3C runs). Use `cargo test -p sq-results --lib` for a green
run of this crate.

## Performance

The project log records no measured numbers for this crate. The cost model is the one described
under Key concepts: one `next_rows` and one `decode` call per 1024 rows, no per-row allocation
in the writers' own code, and a 64 KiB output buffer for solutions and triples.

To separate engine cost from decode and serialization cost, be careful with `sparql_count(q)`
as a baseline. It does not decode or serialize (SPEC §8.2, implemented in `sq-api`), but it
may also skip work through the executor's COUNT fast paths (`fast_count` in `sq-exec`). Scans
can count inside the store index, merge joins sum run products, sorts are never built, BIND
expressions are not evaluated, and a `COUNT(*)` with no DISTINCT or GROUP BY reports its one row
without counting. For such queries, `sparql_query(q) - sparql_count(q)` overstates serialization
cost. A closer baseline is `SELECT count(*) FROM sparql_rows(q)`, which enumerates every row but
decodes no columns (SPEC §8.3), or a query whose operators have no fast path.

## Limitations and known gaps

- **XML output is not well-formed for C0 control characters.** Take a literal that contains,
  for example, U+0001, which can be loaded from N-Triples as `"a\u0001b"`. It is written raw
  inside `<literal>`. sparesults' XML writer only escapes `& < > " '`, and XML 1.0 cannot
  represent U+0001 at all, so strict parsers (expat, libxml2) reject the document.
  `tests/review.rs` records this, and the fix should either refuse such output or produce
  well-formed XML. JSON is not affected, because it escapes these characters as `\u00xx`.
- **Output streams, so an error mid-stream leaves partial output.** An error during writing
  (interrupt, timeout, memory budget) leaves a truncated document in the caller's writer. The
  `BufWriter` flushes what it holds when it is dropped. The in-tree callers (`sq-api`'s
  `write_output`, `sq_engine::query_to_string`) write into a `Vec<u8>` and throw it away on
  error. `sq_engine::write_results` passes its caller's writer straight through.
- **Terms are not re-validated.** Because of the `*_unchecked` oxrdf views, a malformed IRI,
  blank node label or language tag in the store would be written out as it is.
- **Turtle prefixes are fixed** to `rdf`, `rdfs` and `xsd`. The query's own `PREFIX`
  declarations are not reused, and no `@base` is written. There is no RDF/XML, JSON-LD or TriG
  output: `ResultFormat::parse` rejects those names even though `sparql_load` accepts them as
  input. N-Quads output always uses the default graph.
- **`xml` and `json` mean different things in different functions.** In `sparql_load`
  (`sq-api`'s `rdf_format`) they mean RDF/XML and JSON-LD. In `sparql_query` (`ResultFormat`)
  they mean SPARQL Results XML and SPARQL Results JSON.
- **The native mapping loses information**, as the spec intends:
  - Lexical TEXT drops language tags and datatypes. The full term is available as N-Triples in
    the `t*` and `__term` columns.
  - `xsd:decimal` goes through `f64`.
  - Integers outside `i64` become TEXT.
  - A NaN is returned as `Real(NaN)`. `sq-api` (`src/value.rs`) maps it to NULL, as SQLite
    would, before SQLite sees it. In `v*` columns it therefore looks the same as an unbound
    value.

## Where the code differs from docs/SPEC.md

- SPEC §1.3 says this crate depends only on `sq-core`, `sparesults` and `oxrdfio`.
  `Cargo.toml` also lists `oxrdf` (the same pinned 0.3.4 that `sq-core` uses) directly, for the
  borrowed `TermRef` views.
- The API is a superset of the SPEC §11 stub list. It adds:
  - `append_solution_json` and `append_term_json`;
  - `ResultFormat::{ALL, name, media_type, is_solutions_format, is_graph_format,
    solutions_format, graph_format}`;
  - `FromStr` and `Display` for `ResultFormat`.
- SPEC §12.1 lists "golden SPARQL JSON/XML/CSV/TSV; native mapping table". The suite covers
  those, and also the graph formats, ASK output, batching and contract errors.

## Further reading

- [`docs/SPEC.md`](../../docs/SPEC.md):
  - §1.3, the crate graph;
  - §4.1–4.3, rows, batches, late materialization and the decode path behind
    `SolutionSource::decode`;
  - §8.2, `sparql_query` formats and `sparql_count`;
  - §8.3, the `sparql_rows` `json` and `v*` columns;
  - §8.4, the native value mapping;
  - §11, the public API;
  - §12.1, per-crate tests.
- [`docs/DECISIONS.md`](../../docs/DECISIONS.md): why `sq-results` and `sq-sqlgen` depend only on `sq-core`, through the
  source traits.
- [../sq-core/README.md](../sq-core/README.md): `TermId`, `Term`, `BATCH_ROWS` and the
  `SolutionSource` and `TripleSource` traits.
- [../sq-exec/README.md](../sq-exec/README.md): `Execution`, the main implementor of both
  traits.
- [../sq-engine/src/query.rs](../sq-engine/src/query.rs) and
  [../sq-api/README.md](../sq-api/README.md): the callers (`write_results` and
  `query_to_string`; `sparql_query`, `sparql_rows` and native values).
