# sq-expr

`sq-expr` is sparqlite's SPARQL expression engine. It covers the value model for RDF terms,
the expression compiler and evaluator (every SPARQL 1.1 builtin, XSD constructor casts, the `=`
matrix, the `<` family, effective boolean value), the total ORDER BY key, and the aggregate
accumulators. Expressions run on rows of integer `TermId`s. Inline ids (integers, booleans,
decimals, dateTimes, dates, strings of 7 bytes or less) are decoded from the id itself; other
ids are decoded through a per-execution cache and a host-supplied `TermResolver`. Computed
values are interned only when the caller needs an id. The native executor and the reference
oracle share this one implementation, and so do the SQL helper functions behind the baseline
SPARQL-to-SQL translator. All of them therefore agree on SPARQL value semantics. The crate is
`#![forbid(unsafe_code)]` and does no I/O except through the resolver.

## Where it fits

```
                 sq-core  (TermId + codec, ir::Expr / AggSpec, eval_api: TermResolver, ExistsEval, TempDict)
                    |
                 sq-expr
     _______________|___________________________________________
    |               |                   |                        |
 sq-exec         sq-oracle           sq-api                   sq-sqlgen (dev-dependency: tests)
 (FILTER, BIND,  (reference          (SQL helpers sparql_num, sparql_str, sparql_ebv,
  LeftJoin        evaluator)          sparql_sortkey, sparql_eval, sparql_regex,
  filters, sort,                      sparql_sum/avg/min/max/sample/group_concat)
  GROUP BY)
```

`sq-engine` also lists `sq-expr` in its `Cargo.toml`. Normal dependencies are `sq-core`,
`oxrdf`, `spargebra`, `oxsdatatypes` (XSD value types), `oxiri` (`IRI()`), `regex`,
`md-5`/`sha1`/`sha2`, `rand` and `rustc-hash`. The code imports the `Function` enum through
sq-core's re-export (`sq_core::spargebra`), so the direct `spargebra` entry only pins the version.
The dev-dependencies are `sq-core` with its `testing` feature (the seeded `sq_core::testing::Rng`
used by the random tests) plus `oxrdfio` and `sparesults`, which only the W3C-driven test uses.

The crate needs Rust 1.88 or later (edition 2024). It uses let-chains (`if let … && let …`),
which are stable only from 1.88, even though the workspace `Cargo.toml` declares
`rust-version = "1.85"`. The dev VM builds with rustc 1.97.0.

## Key concepts

**1. Errors are values, failures are `Err`.** Every evaluation returns
`sq_core::Result<Option<_>>`. `Ok(None)` means a SPARQL error or an unbound result. `Err` is
fatal (resolver I/O, interrupt, internal bug). Errors propagate, with the exceptions in SPEC §6.2:
`||`/`&&` use three-valued logic, `COALESCE` skips errors, `IF` is an error when its condition
errors or when the branch it takes errors (an error in the branch it does not take is ignored, so
`IF(true, 1, 1/0)` is 1), `BOUND` and `EXISTS` never error, and `IN` errors only when nothing
matched and some comparison errored. `!=` arrives as `Not(Equal)` and `NOT IN` as `Not(In)`,
the way spargebra produces them. In a row, both `TermId::UNBOUND` (0) and the group-key sentinel
`TermId::ERROR` count as unbound (`is_present` is false and `BOUND` returns false).

**2. Lazy values (`value.rs`).** `Val` is `Id(TermId)`, `V(Value)` (a computed value whose term
is the canonical term of that value) or `Typed { value, datatype, lex }`. `Typed` holds a computed
literal whose term is *not* canonical. Examples are `xsd:int("05")`, which is `"5"^^xsd:int`, and
`STRDT("05", xsd:integer)`. `Value` is the decoded form: IRIs, blank nodes, strings, language
strings, the numeric tower, the date/time and duration types, plus `Other` (an unknown datatype)
and `IllTyped` (a known XSD type with a bad lexical form or a failed facet, such as
`"300"^^xsd:byte`). Inline INT/BOOL/DECIMAL/DATETIME/DATE ids decode from the payload with no
cache and no I/O (`inline_value`). SHORTSTR ids also decode from the payload, with no I/O.
SHORTSTR, dictionary, BNODE and TEMP ids are looked up in the `ValueCache` first, and on a miss
SHORTSTR is decoded from the payload and the others with `TermResolver::term`. The id's tag
(for TEMP ids, `temp_tag`) gives the value class of IRIs, blank nodes, strings and language
strings: a STR id is always an xsd:string literal and a LANG id a language-tagged one
(`codec::dict_tag`, `TempDict::intern`). Only TYPED literals are classified by their datatype
IRI. A decoded value is cached from the second decode of its id (concept 8).

**3. Interning only when an id is needed.** Intermediate results stay as `Val::V`/`Val::Typed`.
Only `CompiledExpr::eval`, `id_of_val`/`id_of_value` and `Accumulator::finish` assign ids to
computed values, so FILTER and ORDER BY intermediates do not grow the execution's append-only
`TempDict`. The exception is `BNODE()` and `BNODE(str)`: they allocate TEMP blank nodes through
`TermResolver::fresh_bnode` whenever they are evaluated, including inside `ebv`, `eval_val` and
`sort_key` (so `FILTER(isBlank(BNODE()))` or `ORDER BY BNODE()` add TEMP rows). A computed
value gets its inline id whenever it is inline-encodable (`inline_id_of_value`), and those paths
never call the resolver. Otherwise `TermResolver::intern(term, canonical)` is called.
`canonical = true` returns the dictionary id when the term is stored. Callers need it whenever
the value can meet stored ids (joins, DISTINCT, GROUP keys, aggregate arguments).

**4. The sameTerm ⇔ id-equality invariant.** Inline ids are canonical, computed values use
canonical lexical forms, and query constants are canonical. A TEMP constant is, by the IR
contract, absent from the store. Under these rules two distinct ids denote distinct terms, with
one exception: a slot that the planner left non-canonical can hold a TEMP id for a term that also
has a dictionary row. `ambiguous_identity` (row against row) and `may_alias_const` (row against
constant) detect that case, and only then are the two terms decoded and compared. Under
`eval(.., canonical = true)` a pass-through TEMP id from a slot is re-interned. A TEMP query
constant is returned unchanged, and so are TEMP blank nodes.

**5. Compile-time fast paths (`compile.rs`).** `compile(&Expr, &TempDict)` resolves constants,
pre-decodes the ones available without I/O (inline, SHORTSTR, TEMP), and chooses one of these
specialised nodes:

| Node | Pattern | Work per row |
|---|---|---|
| `IntCmp` | `?x op k` / `k op ?x` / `?x = k`, `k` an inline INT | payload compare |
| `IdEq` | `?x = c`, `c` an IRI or simple literal | id compare (a TYPED row id still decodes) |
| `InIds` | `?x IN (<a>, <b>, ...)`, all IRI constants | id-set test |
| `TagIs` | `isIRI` / `isBlank` / `isLiteral` | tag read (`temp_tag` for TEMP ids) |
| `LangEq` | `LANG(?x) = "c"`, `c` a SHORTSTR or TEMP constant | cached language tag |
| `LangMatchesConst` | `LANGMATCHES(e, "range")`, `range` a SHORTSTR or TEMP constant | no range decode |
| `Regex` / `Replace` (const) | constant pattern and flags | compiled once on first use; literal and `^literal` patterns become string ops; a literal prefix (`^abc…`) and suffix (`…xyz$`) of a single-branch pattern reject texts before the automaton runs, under the `i` flag too (see below); a constant replacement is prepared once |

Arithmetic and numeric-comparison subtrees run directly on `Num`, with no `Value` built in
between. Ordering operands that share no ordering class (an IRI against a number, a string
against a number) are type errors decided from the tags, without decoding. The same holds for
string arguments (the text of REGEX and REPLACE, the arguments of CONTAINS, STRLEN, UCASE,
CONCAT, LANGMATCHES, MD5 and the other string functions): only STR, SHORTSTR and LANG ids hold
string literals, so an IRI, a blank node, a number or a TYPED literal there is an error found
from the tag, and a bare variable is read straight from the row. A constant REGEX pattern's
compiled regex is borrowed from the node, so a row costs no reference-count update for it.

Regex affixes (`regex.rs`, `Plan::Affix`). A single-branch pattern without the `m` and `x`
flags keeps the literal run after a leading `^` (minus a quantified last character) and the
literal run before a final unescaped `$` (minus a character that belongs to a backslash
escape). A text shorter than the two runs together, or whose bytes there differ from them, is
rejected without running the automaton; a text that passes still goes to the automaton. Under the `i`
flag only the ASCII ends of the runs are kept, lower-cased, and they decide only where the
text's bytes are ASCII. An ASCII character matches an ASCII pattern character exactly when
they are equal up to ASCII case, because the other members of a case-folding orbit (U+212A
KELVIN SIGN for `k`, U+017F LONG S for `s`) are not ASCII. A non-ASCII byte in that region
leaves the decision to the automaton. The comparison runs 8 bytes at a time as `(text |
mask) == literal`, with mask 0x20 on the lower-case letters. Q7's `^person 1[0-9]*7$` with
`i` thus runs the automaton on about 11K of 200K names instead of all of them. Unit tests
compare plan and automaton on fixed cases and on random patterns built from literals in both
cases, escapes, classes, quantifiers, groups, alternations and anchors, under seven flag
combinations, against texts that contain the Kelvin sign and the long s. One run checks more
than 20,000 pattern and text pairs, including over 200 case-sensitive and 200
case-insensitive affix plans. Three more seeds at ten times that size also passed when the
plan was written.

**6. Numerics (`numeric.rs`).** Promotion runs integer → decimal → float → double.
`xsd:integer` and the derived integer types are `i64`; larger values are `IllTyped` (a documented
divergence). `xsd:decimal` is oxsdatatypes' fixed-point `Decimal` (an i128 scaled by 10^18).
Decimal `*` and `/` are exact through a 256-bit intermediate and are then truncated to 18
fractional digits. The crate does not use oxsdatatypes' `checked_mul`/`checked_div`, because they
fail on cases such as `0 / 2.5`. `integer / integer` is the exact truncated decimal quotient.
Integer or decimal overflow and division by zero are errors, while float and double give ±INF or
NaN. Decimal → double conversion is correctly rounded (`dec_to_f64`), which keeps it monotone for
the sort key. `ROUND` rounds half toward +∞.

**7. `SortKey` (`sortkey.rs`).** The ORDER BY key is a byte string ordered by memcmp:
`(class, primary, lex, datatype, lang)`. Classes are ordered unbound/error < blank node < IRI <
literal. Literals are grouped into families: numeric, boolean, string, language string, the
date/time types, durations, then `Other`/`IllTyped`. The order is total by construction, so
`sort_by` cannot panic, and it agrees with `<` wherever `<` is defined. The same bytes are the
`sparql_sortkey(id)` BLOB in sq-api. For inline INT ids, raw payload order equals key order;
sq-exec's sort fast path depends on this. MIN and MAX use the same order.

**8. Per-execution state lives in `ValueCache`.** It holds the decoded values (131,072 entries by
default and cleared when full; `ValueCache::with_limit`, minimum 16), the dynamic regex cache (256
`(pattern, flags)` entries, cleared when full) and the `BNODE(str)` scope. A decoded value is
admitted only on the second decode of its id. The first decode records the id in a
direct-mapped table of 4,096 recently decoded ids (32 KiB, allocated on the first miss). The
next decode of an id still in that table caches the value. A FILTER over distinct values
(REGEX over 200K names) therefore never inserts into the map, which used to cost an insert and
its rehashes per row with no hit. Repeated ids are cached from their second use. Two ids that
share a table slot evict each other, which only delays caching. The `BNODE(str)` scope is keyed
by row content, with blank nodes from earlier `BNODE(str)` calls on the same solution masked out
of the key (the "lineage" rule in `bnode.rs`). Its memory is bounded by two generations of about
8 MiB each. Create one `ValueCache` per execution and never share one between executions,
because TEMP ids differ. `ValueCache::clear()` empties only the decoded values; the regex cache,
the BNODE scope and the admission table (which holds ids, not values) survive it.

## Public API tour

Everything public is re-exported from `lib.rs`. The modules themselves are private, except
`testing`, which the `testing` feature exposes. The host builds an `EvalCtx` for each execution
(or each batch):

```rust
use sq_expr::{EvalCtx, ValueCache};

let mut values = ValueCache::new();            // one per execution
let mut cx = EvalCtx {
    resolver: &mut resolver,                   // &mut dyn sq_core::eval_api::TermResolver
    exists: &mut exists,                       // &mut dyn sq_core::eval_api::ExistsEval
    values: &mut values,
    now,                                       // oxsdatatypes::DateTime, fixed for the query
    base_iri: Some("http://example.org/"),     // IRI() / URI()
    rng: &mut rng,                             // &mut dyn rand::RngCore (RAND, UUID, STRUUID)
};
```

Compile once, then evaluate per row. This snippet uses the `testing` doubles. `TestEnv::with`
builds the `EvalCtx`:

```rust
use sq_core::eval_api::TermResolver;
use sq_core::ir::Expr;
use sq_core::spargebra::algebra::Function;
use sq_core::{Literal, Term, TermId};
use sq_expr::{compile, testing::TestEnv};

let mut env = TestEnv::new();
// FILTER(?x > 500), ?x in slot 0: compiles to IntCmp.
let e = Expr::Greater(Box::new(Expr::Slot(0)), Box::new(Expr::Const(TermId::from_int(500).unwrap())));
let filter = compile(&e, &env.resolver.temps)?;
assert_eq!(env.with(|cx| filter.ebv(&[TermId::from_int(742).unwrap()], cx))?, Some(true));
let alice = env.resolver.iri("http://example.org/alice");
assert_eq!(env.with(|cx| filter.ebv(&[alice], cx))?, None); // type error: FILTER drops the row

// BIND(CONCAT(?name, "!") AS ?greeting). Constants are canonical ids (inline, dictionary or TEMP).
let bang = env.resolver.intern(&Term::Literal(Literal::simple("!")), true)?;
let bind = compile(&Expr::Call(Function::Concat, vec![Expr::Slot(0), Expr::Const(bang)]), &env.resolver.temps)?;
let name = env.resolver.simple("Alice Liddell");
let id = env.with(|cx| bind.eval(&[name], cx, true))?;   // Some(TEMP id of "Alice Liddell!")
let lazy = env.with(|cx| bind.eval_val(&[name], cx))?;   // Some(Val::V(Value::Str(..))), not interned
let key = env.with(|cx| bind.sort_key(&[name], cx))?;    // ORDER BY key, not interned
```

`CompiledExpr` also has `as_slot()` and `as_const()`, which let hosts recognise a bare variable
or a constant (for a pass-through BIND or `ORDER BY ?x`). `string_slots()` names the slots whose
strings the expression decodes first on every row: the text of REGEX or REPLACE, or the first
argument of a string function, reached from the root through the operand each node evaluates
first (`!`, the left operand of `&&`, `||`, `<`, `+` and IN, IF's condition, the first COALESCE
argument, a cast's argument, the non-constant side of `=` and sameTerm, and nested string
functions such as `REGEX(LCASE(?n), ..)`). For such a slot, every row whose value is a STR or
LANG id decodes it before anything can end the evaluation. sq-exec's Filter and Extend
therefore decode those ids of a whole input batch in one store call (see
[../sq-exec/README.md](../sq-exec/README.md)). The analysis returns nothing when the first
operand may decide alone, as in `?age > 3 && REGEX(?n, ..)` or `BOUND(?n) && ..`.
`CompiledExpr` is `Send` but not `Sync`, because constant regexes are cached in a `OnceCell`
on first use.

Aggregates. The host evaluates the argument itself with `canonical = true` and passes the id to
`add`, or passes `None` when the argument is unbound or an error. `accumulator` ignores
`spec.arg`. DISTINCT deduplicates by id inside the accumulator:

```rust
use sq_core::ir::{AggFunc, AggSpec};
use sq_expr::accumulator;

let mut sum = accumulator(&AggSpec { func: AggFunc::Sum, distinct: false, arg: None });
let inputs = [TermId::from_int(2).unwrap(), env.resolver.typed("2.5", "decimal")];
let total = env.with(|cx| -> sq_core::Result<Option<TermId>> {
    for id in inputs {
        sum.add(Some(id), cx)?;      // add(None, cx) would make SUM unbound (agg-err-01)
    }
    sum.finish(cx)                   // "4.5"^^xsd:decimal, an inline DECIMAL id
})?;
// COUNT(*) / COUNT(DISTINCT *): feed whole rows; `visible` = the user-visible in-scope slots.
// count.add_row(row, visible, cx)?;
```

Call `finish` once per group: `GROUP_CONCAT` moves its buffer out on the first call.

Value-level helpers (all take `&mut EvalCtx` when they may need to decode):

| Area | Items |
|---|---|
| ids and values | `value_of`, `value_of_val`, `inline_value`, `term_of_id`, `term_of_value`, `id_of_val`, `id_of_value`, `inline_id_of_val`, `inline_id_of_value`, `kind_of_val`, `is_present` |
| comparison | `equals` / `equals_values` (§6.3), `compare` / `compare_values` → `Option<Order>` (`Less`/`Equal`/`Greater`/`Unordered` for NaN; `None` = type error), `ebv` / `ebv_value`, `same_term` / `same_term_ids` |
| ordering | `sort_key(id, cx)`, `sort_key_val`, `sort_key_value`, `SortKey::{lowest, as_bytes, into_bytes, is_lowest}` |
| types | `Value`, `Val` (`computed_value`, `computed_term`), `Num`, `XsdType` (`from_iri`, `iri`, `iri_id`, `integer_range`), `well_known_iri_id` |

Hosts (sq-exec, sq-oracle, sq-api) should call `value_of_val`, `kind_of_val` and `id_of_val`
instead of matching on `Val` variants. The feature `testing` exports `testing::{TestResolver,
TestEnv, FnExists, NoExists}`. `TestResolver` follows the store's id discipline: inline ids,
`WELL_KNOWN` dictionary rows, counter blank nodes, and TEMP ids for everything else. Inside this
crate's own tests the module is always available. Another crate's tests (and the second snippet
above) need the feature turned on, in that crate's `Cargo.toml`:

```toml
[dev-dependencies]
sq-expr = { workspace = true, features = ["testing"] }
```

## Testing

```sh
cargo test -p sq-expr                    # 108 tests: 103 run, 5 ignored
git clone https://github.com/w3c/rdf-tests /path/to/rdf-tests
SQ_W3C_DIR=/path/to/rdf-tests/sparql cargo test -p sq-expr w3c -- --nocapture
cargo test --release -p sq-expr bench -- --ignored --nocapture        # micro-benchmarks
SQ_VALUE_DIFF_N=20000 SQ_VALUE_DIFF_OUT=/tmp/x.tsv cargo test -p sq-expr value_diff -- --ignored
```

- Module unit tests cover XSD lexical spaces and facets, the `WELL_KNOWN` row table, decimal
  arithmetic and rounding, inline dateTime/date decoding (checked against the parser on 2,000
  pseudo-random values), regex translation and flags, regex plans against the automaton (fixed
  and random patterns), the string helpers, the cast table and the BNODE scope (lineage and
  memory bound).
- `src/tests/` uses a harness that parses SPARQL expression text with spargebra, lowers it the
  way sq-plan does (stored constants become dictionary ids, absent ones TEMP ids) and evaluates it
  on a `TestEnv`. It contains per-function tables with W3C expected values (`functions.rs`), the
  open-eq-07/08 `=` matrix and NaN (`equality.rs`), SortKey totality and transitivity
  (`sorting.rs`), aggregates (`aggregates.rs`) and interning and identity rules
  (`interning.rs`). `review.rs` and `review2.rs` hold adversarial tests from two review rounds;
  some of these check that the fast paths agree with the general path for every id class.
  `decoding.rs` covers the decoding discipline. It checks the `ValueCache` admission (nothing
  cached on a first decode, cached from the second, single-use ids never fill the map) and that
  tag-classified values equal datatype-classified ones for every id class. It checks that string
  arguments of non-string ids are errors with no resolver call, and what `string_slots` returns
  for 25 expressions.
- `w3c.rs` runs the query-evaluation tests of 16 expression-heavy W3C directories through a small
  nested-loop reference evaluator. Clone https://github.com/w3c/rdf-tests and set
  `SQ_W3C_DIR=<clone>/sparql`. The built-in default is a scratchpad path that exists only on the
  original dev VM. Anywhere else the test prints a note to stderr and passes without checking
  anything, so a green `cargo test -p sq-expr` says nothing about W3C conformance unless
  `SQ_W3C_DIR` is set. The last run gave 291 passes, 8 lenient numeric passes (`"3"` against
  `"3.0"`), 2 skips (one named-graph test, one query spargebra cannot parse) and 0 failures. The
  test fails if fewer than 290 tests pass (strict or lenient).
- Ignored tools: `bench_hot_paths` (`SQ_BENCH_ROWS`, default 200,000; `SQ_BENCH_ONLY` filters
  case names; the case "over distinct strings (single use)" builds `SQ_BENCH_ROWS` distinct
  dictionary strings and decodes each once on a fresh `ValueCache`), `value_diff` (writes
  `expression \t result` TSV to `SQ_VALUE_DIFF_OUT`; `SQ_VALUE_DIFF_N` sets the number of
  expressions, default 5,000),
  `review_string_function_diff` / `review_date_function_diff` (`SQ_REVIEW_DIFF_OUT`) and
  `review_probe_expressions` (`SQ_REVIEW_PROBE_IN` → `SQ_REVIEW_PROBE_OUT`). The two
  `review_*_diff` tests write the same `SQ_REVIEW_DIFF_OUT` file, so a plain `-- --ignored` run
  overwrites one result with the other. Run them one at a time by name, for example
  `SQ_REVIEW_DIFF_OUT=/tmp/dates.tsv cargo test -p sq-expr review_date_function_diff -- --ignored`.

## Performance

Release micro-benchmarks (`bench_hot_paths`) were run on the project's 4-vCPU dev VM. sq-expr
does not touch SQLite. The "logged" column comes from quiet runs recorded in `../../notes.md` and
the crate's author notes. The "range" column is the spread of three runs made while writing this
README, while other builds were loading the VM. It is noise, not a bound: later runs, on both a
quiet and a loaded VM, landed above and below it (for example 38.5-40.6 ns for `?x < 10.5` over
strings, 283-485 ns for the dynamic REGEX, 130 ns for BNODE, 47.7 ns for COALESCE). Expect up to
about ±2x between runs on the shared VM, and compare cases only within one run.

| Case (ns/row) | logged | range |
|---|---|---|
| `?x > 500` on inline ints (IntCmp) | 10 | 7.5-14 |
| `?x = <iri>` (IdEq) | 8.7 | 7.6-16 |
| `?x > 10` over IRIs (type error from tags) | 22 | 16-41 |
| `?x < 10.5` over strings (type error from tags) | 17 | 18-36 |
| `REGEX(?x, "^abc string number 1")` (literal prefix) | 87 | 101-117 |
| `REGEX(?x, ?y)` (dynamic pattern, cache hit) | | 244-252 |
| `BIND(COALESCE(?u, "no label available"))`, canonical | 53 | 50-67 |
| `BIND(CONCAT(?x, "!"))` (TEMP intern) | | 364-637 |
| `BIND(BNODE(?x))` (scope hit) | 211 | 169-248 |
| `SUM` over inline ints | 12 | 12-17 |
| `sort_key` of a dictionary string | | 99-233 |

### Decoding discipline and regex affixes (2026-10-15)

The `ValueCache` admission rule, tag-classified values, tag-decided string arguments, the
borrowed constant regex and the regex affixes were measured against the same tree with this
crate's sources at HEAD (only `tests/bench.rs` kept, so that both builds have the same cases).
The time is the best of 7 interleaved runs of `bench_hot_paths` (ns/row) on the shared 4-vCPU VM
at a load average of about 2.5. The instruction counts per row come from callgrind with the
harness setup subtracted, and they are deterministic. Timings are not: cases near 10 ns moved by
±1-2 ns in either direction between builds (for example `?x = <iri>` read 9.1 → 7.7 in one A/B
pair and 8.2 → 9.5 in the next) while their library code ran the same instructions. The extra
1-5 instructions that `bench_hot_paths` itself spends in some loops differ between the two
builds, for example the `SUM` loop around an identical `Sum::add`.

| Case | ns/row before | ns/row after | instr/row before | instr/row after |
|---|---|---|---|---|
| `REGEX(?x, "1$")` over 200K distinct strings, fresh cache | 717 | 243 | 5,018 | 4,424 |
| `REGEX(?x, "^abc string number 1")` (literal prefix) | 98.0 | 60.4 | 459 | 350 |
| `REGEX(?x, "number [0-9]5$")` (suffix "5") | 117.8 | 64.5 | 673 | 387 |
| `REGEX(?x, "^ABC string number 1[0-9]*5$", "i")` | 135.6 | 65.9 | 803 | 430 |
| `REGEX(?x, ?y)` (dynamic pattern, cached) | 228.2 | 172.2 | 1,197 | 950 |
| `CONTAINS(?x, "99")` / `STRLEN(?x) > 20` | 155.6 / 144.4 | 129.6 / 122.3 | 821 / 852 | 729 / 759 |
| `BIND(UCASE(?x))` / `BIND(BNODE(?x))` | 305.5 / 143.8 | 248.2 / 100.0 | 1,748 / 703 | 1,646 / 614 |
| `sort_key` of a dictionary string / `MIN(string)` | 104.2 / 73.7 | 91.4 / 70.6 | 822 / 341 | 810 / 319 |
| every other case | | within the noise above | | equal, or +1-5 in the harness loop |

The single-use case's instruction count includes building its 200K dictionary strings, which
dominates it; its time does not. End to end, Q7 (`REGEX(?n, "^person 1[0-9]*7$", "i")` over
200K names, sparqlite extension, social dataset, count mode, best of 5 CPU) went from 178 ms to
65 ms warm and from 485 ms to 208 ms cold; pyoxigraph takes about 170 ms. Its warm run went
from 362.7M to 207.9M instructions (callgrind). This crate's share of the savings: `ValueCache`
inserts and rehashes (29M, plus the value clones and drops that went with them), the datatype
comparison in `Value::from_literal` and `XsdType::from_iri` (17M), and about 50M in the regex,
whose automaton now runs only on the 11,111 names that pass both affixes (the matches) instead
of all 200K. sq-exec removed its decoded-term map and batches the decodes (see
[../sq-exec/README.md](../sq-exec/README.md)).

Value agreement with pyoxigraph, as logged in `../../notes.md`: on 20,000 random arithmetic, cast
and comparison expressions (`SQ_VALUE_DIFF_N=20000`), 97.8% matched exactly and 100% matched
after excluding documented pyoxigraph divergences (NaN comparisons raising errors, ROUND half
away from zero, decimal `x*0` and `0/2.5` failing, STR canonicalising numerics). These figures
cannot be reproduced from the repository: the `value_diff` test only writes the TSV, and the
pyoxigraph comparison script is not checked in.

## Where the code differs from docs/SPEC.md

- §6.1 shows `Val { Id, V }`; the code adds `Val::Typed`, which keeps derived-integer casts and
  non-canonical STRDT results exact without interning them.
- §6.1 lists fewer fast paths than the code has. The code adds `InIds`, `LangMatchesConst`, a
  prepared REPLACE replacement, `Num`-only arithmetic subtrees and tag-decided ordering errors.
  The public API is also wider: `eval_val`, `sort_key`, `as_slot`/`as_const`, the value,
  comparison and sort helpers, `Num` and `XsdType`.
- `Accumulator` has a provided method, `add_row(row, visible, cx)`. `COUNT(DISTINCT *)`
  deduplicates rows projected onto the user-visible slots. Hidden slots, such as blank-node
  pattern variables, do not split solutions.
- §6.6 says `BNODE(str)` is "stable within one solution row". The code keys solutions by row
  content plus the lineage rule above, under a 16 MiB bound (see Limitations).
- §4.3 says pass-through expressions (`BIND(?x AS ?y)`, `COALESCE(?x, …)`, `IF(c, ?x, ?y)`)
  never intern. In the code, `eval(.., canonical = true)` re-interns a pass-through TEMP id that
  comes from a slot (`id_of_val`). Only TEMP query constants and TEMP blank nodes pass through
  unchanged.
- §1.3 lists sq-exec, sq-oracle and sq-engine as dependents. In the code sq-api is a normal
  dependent and sq-sqlgen is a dev-dependent.
- The SPEC test table names `py/value_diff.py`. That script is not in the repository; the
  ignored `value_diff` test only writes the TSV.

## Limitations and known gaps

- `xsd:integer` is `i64`, and larger integers are `IllTyped`. Decimals carry 18 fractional digits,
  and `*` and `/` truncate to that precision.
- Integral decimals print as `"2"` (oxsdatatypes `Display`). W3C expected files that use `"2.0"`
  pass only through the lenient numeric comparison.
- `BNODE(str)`: duplicate solutions of a multiset (`VALUES ?x { 1 1 }`) have identical rows and so
  share their blank nodes.
- Regex: XPath-only constructs (`\i \I \c \C`, back-references, character-class subtraction,
  `--` in a class) are errors. XSD Unicode block escapes (`\p{IsX}` / `\P{IsX}`) are passed to the
  `regex` crate unchanged, and it drops the `Is` prefix. Most block names, such as
  `\p{IsBasicLatin}`, are therefore errors. A block name that is also a script name, such as
  `\p{IsGreek}`, compiles but matches the script rather than the block, so it is silently wrong
  (Greek Extended U+1F00 matches; Coptic U+03E2 in the Greek block does not). The only flags
  are `i s m x q`. Compiled programs are capped at 1 MiB.
- `GROUP_CONCAT` returns a simple literal and becomes unbound if any input is unbound, an error
  or a blank node. MIN and MAX become unbound on any unbound or error input.
- An unknown custom function, or a cast with the wrong arity, compiles to a node that always
  errors. A custom aggregate is always unbound. `compile` returns `Err(Error::Invalid)` only for a
  builtin called with the wrong number of arguments.
- The W3C-driven test skips named graphs, property paths, EXISTS, SERVICE and CONSTRUCT/DESCRIBE.
  The full-engine runner, `tools/sq-w3c`, covers those.

## Further reading

- [docs/SPEC.md](../../docs/SPEC.md): §6 (6.1 API, 6.2 numerics and errors, 6.3 `=` matrix,
  6.4 EBV and ordering, 6.5 SortKey, 6.6 builtins, 6.7 aggregates), §3 (term identity and
  encoding), §4.3 (decode and lookup paths).
- [docs/DECISIONS.md](../../docs/DECISIONS.md): the `=` matrix, SortKey and aggregate decisions.
- [notes.md](../../notes.md): review findings and measurements.
- Sibling crates: [sq-core](../sq-core/README.md) (TermId layout, codec, `eval_api`),
  [sq-exec](../sq-exec/README.md) (the main host), [sq-oracle](../sq-oracle/README.md),
  [sq-api](../sq-api/) (the SQL helper functions), [sq-sqlgen](../sq-sqlgen/README.md).
