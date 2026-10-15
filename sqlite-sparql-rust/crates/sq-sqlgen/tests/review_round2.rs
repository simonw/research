//! Adversarial review tests, round 2 (WP9b). Each test pins the answer SPARQL 1.1 (and
//! docs/SPEC.md §5.5 / §5.6, DECISIONS "zero-length paths treat seeds as constants") requires,
//! or a property the translator claims (lib.rs: "Every accepted statement prepares on the
//! host's SQLite 3.45"), for a query inside the supported subset. A failure means a wrong
//! answer, an unbounded statement, or one SQLite refuses to prepare.
mod common;

use common::{Baseline, DataFile, Engine, Outcome};
use sq_core::Error;

const PATH_DATA: &str = r#"
@prefix : <http://ex.org/> .
:alice :name "Alice" .
:p1 :sub :p2 . :p2 :sub :p3 .
:g1 { :alice :likes :tea . :p1 :sub :p9 . }
"#;

/// The baseline's rows (sorted, N-Triples), or None when the query is unsupported.
fn answer(engine: &Engine, q: &str) -> (Option<Vec<Vec<String>>>, String) {
    let query = sq_plan::parse_query(&format!("{PREFIX}{q}"), None).unwrap();
    match engine.baseline(&query).unwrap() {
        Baseline::Unsupported(_) => (None, String::new()),
        Baseline::Ran { sql, outcome } => {
            let rows = match outcome {
                Outcome::Solutions { rows, .. } => rows,
                Outcome::Boolean(b) => vec![vec![Some(oxrdf::Literal::from(b).into())]],
            };
            let mut out: Vec<Vec<String>> = rows
                .iter()
                .map(|r| r.iter().map(|t| t.as_ref().map_or("UNDEF".to_owned(), |t| t.to_string())).collect())
                .collect();
            out.sort();
            (Some(out), sql.sql)
        }
    }
}

/// `path::zero_length` says `OneOrMore(x)` never matches a zero-length pair, but `(p*)+` and
/// `(p?)+` contain p* / p? and do (the translation itself produces the pair: the closure's
/// seed is x's relation, which holds `(c, c)`). Every decision keyed on that predicate is
/// therefore wrong for them, while the same queries with `p*` are answered correctly or
/// cleanly rejected:
/// * GRAPH <c>: `classify` calls the body GForm and drops the dataset-membership check, so a
///   stored IRI that is not a graph (`:alice`) yields the zero-length pair (spec 18.6: no
///   solutions; the round-1 fix covers only `p*` / `p?`);
/// * an empty active graph (FROM NAMED only): `path()` returns "no solutions" before
///   translating, although the constant endpoint matches itself (the baseline's own answer for
///   `:p1 :sub* ?y` there is `:p1`);
/// * EXISTS with an optionally substituted endpoint: `endpoint()` does not reject it (as it does
///   for `?x :sub* ?y`), the path is computed with both ends free, and the substituted constant
///   `:nowhere` — not a node of the graph — loses its zero-length pair, so NOT EXISTS keeps a
///   row it must drop.
#[test]
fn review_one_or_more_over_a_zero_length_path_is_zero_length_capable() {
    let engine = Engine::new(&[DataFile::trig(PATH_DATA)]).unwrap();
    let p1 = vec![vec!["<http://ex.org/p1>".to_owned()]];
    let cases: &[(&str, Vec<Vec<String>>)] = &[
        ("SELECT ?y WHERE { GRAPH :alice { :p1 (:sub*)+ ?y } }", vec![]),
        ("SELECT ?y WHERE { GRAPH :alice { :p1 (:sub?)+ ?y } }", vec![]),
        ("SELECT ?y FROM NAMED :g1 WHERE { :p1 (:sub*)+ ?y }", p1.clone()),
        ("SELECT ?y FROM NAMED :g1 WHERE { :p1 (:sub?)+ ?y }", p1.clone()),
        ("SELECT ?x WHERE { VALUES ?x { :nowhere UNDEF } FILTER NOT EXISTS { ?x (:sub?)+ ?y } }", vec![]),
        ("SELECT ?x WHERE { VALUES ?x { :nowhere UNDEF } FILTER EXISTS { ?x (:sub*)+ ?y } }", {
            let mut rows = vec![vec!["<http://ex.org/nowhere>".to_owned()], vec!["UNDEF".to_owned()]];
            rows.sort();
            rows
        }),
    ];
    // Controls: the p* forms, which `zero_length` recognises.
    assert_eq!(answer(&engine, "SELECT ?y WHERE { GRAPH :alice { :p1 :sub* ?y } }").0, Some(vec![]));
    assert_eq!(answer(&engine, "SELECT ?y FROM NAMED :g1 WHERE { :p1 :sub* ?y }").0, Some(p1.clone()));
    let mut failures = Vec::new();
    for (q, want) in cases {
        let (got, sql) = answer(&engine, q);
        // Unsupported is an acceptable answer; a wrong one is not.
        if got.as_ref().is_some_and(|g| g != want) {
            failures.push(format!("{q}\n  got {got:?}, want {want:?}\n  SQL: {sql}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

const DATA: &str = r#"
@prefix : <http://ex.org/> .
:alice :age 30 ; :name "Alice" .
:bob :age 25 ; :name "Bob" .
"#;

const PREFIX: &str = "PREFIX : <http://ex.org/> ";

/// `BIND(COALESCE(?b, ?b, …) AS ?b2)` chains: every reference to a BIND-bound variable pastes
/// the whole SQL expression of its binding (`Col::sql`) into the referencing expression, so k
/// references per level and n levels give k^n copies. The parser-stack check only bounds the
/// nesting depth (here n + 1 calls), not the text: this 483-byte query (n = 6, k = 10) becomes a
/// 7.1 MB statement holding 10^6 copies of `?a`'s column. One more level gives 71 MB (about
/// 30 s to prepare and run on 6 rows), n = 8 gives 711 MB and n = 9 an allocation of several
/// GB, i.e. an out-of-memory abort of the host process.
#[test]
fn review_repeated_references_to_bind_results_grow_the_sql_exponentially() {
    let engine = Engine::new(&[DataFile::turtle(DATA)]).unwrap();
    let (levels, refs) = (6, 10);
    let list = |v: &str| vec![v.to_owned(); refs].join(", ");
    let mut body = format!("?s :age ?a BIND(COALESCE({}) AS ?b1) ", list("?a"));
    for i in 2..=levels {
        body.push_str(&format!("BIND(COALESCE({}) AS ?b{i}) ", list(&format!("?b{}", i - 1))));
    }
    let text = format!("{PREFIX}SELECT ?s ?b{levels} WHERE {{ {body} }}");
    let query = sq_plan::parse_query(&text, None).unwrap();
    let lowered = engine.lower(&query).unwrap();
    match sq_sqlgen::translate(&lowered) {
        Err(Error::Unsupported(_)) => {}
        Err(e) => panic!("unexpected error: {e}"),
        Ok(sql) => {
            // Linear in the query would be a few KB; allow a generous constant factor.
            let bound = 100 * text.len();
            assert!(
                sql.sql.len() <= bound,
                "a {}-byte query was translated into a {}-byte statement ({} copies of `?a`'s column)",
                text.len(),
                sql.sql.len(),
                sql.sql.matches("t1.o").count()
            );
        }
    }
}

/// The same duplication on an ordinary three-step computation: `?b` is pasted into `?c`'s
/// expression twice, `?c` into `?d`'s twice, and `?d` into the SELECT list, the FILTER and the
/// ORDER BY key, so each row evaluates `sparql_eval('*', …)` 9 times where 2 are needed (and
/// `?a + 1` 12 times). The native engine evaluates each BIND once per row; the baseline's cost
/// grows with the number of references, which skews the comparison for BIND-heavy queries.
#[test]
fn review_bind_chain_evaluates_each_binding_once() {
    let engine = Engine::new(&[DataFile::turtle(DATA)]).unwrap();
    let text = format!(
        "{PREFIX}SELECT ?s ?d WHERE {{ ?s :age ?a BIND(?a + 1 AS ?b) BIND(?b * ?b AS ?c) BIND(?c * ?c AS ?d) \
         FILTER(?d > 0) }} ORDER BY ?d"
    );
    let query = sq_plan::parse_query(&text, None).unwrap();
    let lowered = engine.lower(&query).unwrap();
    let sql = sq_sqlgen::translate(&lowered).unwrap();
    let products = sql.sql.matches("sparql_eval('*'").count();
    assert!(
        products <= 2,
        "the two multiplications appear {products} times in the statement (each evaluated per row):\n{}",
        sql.sql
    );
    // The answer itself is right.
    let out = engine.execute(&lowered, &sql).unwrap();
    let Outcome::Solutions { rows, .. } = out else { panic!("not solutions") };
    assert_eq!(rows.len(), 2);
}

/// SQLite 3.45.1 on this host binds at most 250000 parameters (`SQLITE_LIMIT_VARIABLE_NUMBER`),
/// and every distinct constant is one `?N`. A VALUES block (or IN list, or long BGP) with more
/// distinct constants is accepted by `translate` and `limits::check`, then fails to prepare
/// with "variable number must be between ?1 and ?250000" — a message that is not in the list of
/// prepare errors notes_for_lead asks sq-api to map to `Unsupported`.
#[test]
fn review_accepted_queries_stay_within_the_sqlite_variable_limit() {
    let engine = Engine::new(&[DataFile::turtle(DATA)]).unwrap();
    let values: Vec<String> = (0..250_001).map(|i| i.to_string()).collect();
    let text = format!("{PREFIX}SELECT ?s WHERE {{ ?s :age ?a VALUES ?a {{ {} }} }}", values.join(" "));
    let query = sq_plan::parse_query(&text, None).unwrap();
    let lowered = engine.lower(&query).unwrap();
    match sq_sqlgen::translate(&lowered) {
        Err(Error::Unsupported(_)) => {}
        Err(e) => panic!("unexpected error: {e}"),
        Ok(sql) => {
            if let Err(e) = engine.execute(&lowered, &sql) {
                panic!(
                    "accepted ({} parameters) but not executable: {}",
                    sql.params.len(),
                    e.lines().next().unwrap_or("")
                );
            }
        }
    }
}
