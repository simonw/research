//! Golden SQL: the exact statement for representative queries (reviewed by hand; update
//! deliberately when the translation changes).
use super::sql;

/// (what it pins, query, SQL).
const GOLDEN: &[(&str, &str, &str)] = &[
    (
        "star BGP with an inline-int range filter (§9 fast path)",
        "SELECT ?x ?n WHERE { ?x a :Person ; :name ?n ; :age ?a FILTER(?a >= 18) }",
        "SELECT t1.s AS v0, t2.o AS v1 FROM main.rdf_quad AS t1, main.rdf_quad AS t2, main.rdf_quad AS t3 WHERE \
         t1.g = 0 AND t1.p = ?1 AND t1.o = ?2 AND t2.g = 0 AND t1.s = t2.s AND t2.p = ?3 AND t3.g = 0 AND t1.s = \
         t3.s AND t3.p = ?4 AND ((t3.o & 15) = 6 AND (t3.o >> 4) >= ?5 OR (t3.o & 15) <> 6 AND sparql_num(t3.o) \
         >= ?5)",
    ),
    (
        "decimal comparison: sparql_num when only truth matters, sparql_eval under NOT",
        "SELECT ?x WHERE { ?x :score ?s FILTER(?s > 1.5 && !(?s < 0.5)) }",
        "SELECT t1.s AS v0 FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?1 AND ((sparql_num(t1.o) > \
         sparql_num(?2)) AND (NOT (sparql_eval('<', t1.o, ?3) = 23)))",
    ),
    (
        "simple-literal equality by id; exact form under NOT",
        "SELECT ?x WHERE { ?x :name ?n FILTER(?n = \"Alice\" || !(?n = \"Bob the Builder\")) }",
        "SELECT t1.s AS v0 FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?1 AND ((t1.o = ?2) OR (NOT CASE \
         WHEN t1.o = ?3 THEN 1 WHEN (t1.o & 15) IN (5, 14) THEN sparql_eval('=', t1.o, ?3) = 23 WHEN t1.o IS NOT \
         NULL THEN 0 END))",
    ),
    (
        "variable equality: id shortcuts, values otherwise",
        "SELECT ?x ?y WHERE { ?x :knows ?y FILTER(?x != ?y) }",
        "SELECT t1.s AS v0, t1.o AS v1 FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?1 AND (NOT CASE WHEN \
         t1.s IS NULL OR t1.o IS NULL THEN NULL WHEN t1.s = t1.o AND (t1.s & 15) NOT IN (5, 14) THEN 1 WHEN (t1.s \
         & 15) IN (1, 2) OR (t1.o & 15) IN (1, 2) THEN 0 ELSE sparql_eval('=', t1.s, t1.o) = 23 END)",
    ),
    (
        "IN over integers and IRIs",
        "SELECT ?x WHERE { ?x :age ?a FILTER(?a IN (1, 2) && ?x IN (:a, :b)) }",
        "SELECT t1.s AS v0 FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?1 AND ((((t1.o & 15) = 6 AND t1.o \
         = ?2 OR (t1.o & 15) <> 6 AND sparql_num(t1.o) = ?3) OR ((t1.o & 15) = 6 AND t1.o = ?4 OR (t1.o & 15) <> \
         6 AND sparql_num(t1.o) = ?5)) AND (t1.s IN (?6, ?7)))",
    ),
    (
        "BOUND on a certain variable, EBV of a variable",
        "SELECT ?x WHERE { ?x :score ?s FILTER(BOUND(?s) && ?s) }",
        "SELECT t1.s AS v0 FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?1 AND (1 AND sparql_ebv(t1.o))",
    ),
    (
        "regex, contains via sparql_str, isIRI on the tag",
        "SELECT ?x WHERE { ?x :name ?n FILTER(REGEX(?n, \"^A\", \"i\") && CONTAINS(?n, \"li\") && isIRI(?x)) }",
        "SELECT t1.s AS v0 FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?1 AND ((sparql_regex(t1.o, ?2, ?3) \
         AND (instr(sparql_str(t1.o), sparql_str(?4)) > 0)) AND ((t1.s & 15) = 1 OR (t1.s & 15) = 14 AND ((t1.s \
         >> 4) & 7) = 1))",
    ),
    (
        "OPTIONAL: one pattern inline, a group as a derived table, filter on nullable bindings",
        "SELECT ?x ?y WHERE { ?x :knows ?y OPTIONAL { ?y :name ?n } OPTIONAL { ?y :knows ?z . ?z :name ?m \
         FILTER(?m != ?n) } }",
        "SELECT t1.s AS v0, t1.o AS v1 FROM main.rdf_quad AS t1 LEFT JOIN main.rdf_quad AS t2 ON t2.g = 0 AND \
         t2.p = ?1 AND t1.o = t2.s LEFT JOIN (SELECT t3.s AS v1, t3.o AS v3, t4.o AS v4 FROM main.rdf_quad AS t3, \
         main.rdf_quad AS t4 WHERE t3.g = 0 AND t3.p = ?2 AND t4.g = 0 AND t3.o = t4.s AND t4.p = ?1) AS d5 ON \
         t1.o = d5.v1 AND (NOT CASE WHEN d5.v4 IS NULL OR t2.o IS NULL THEN NULL WHEN d5.v4 = t2.o AND (d5.v4 & \
         15) NOT IN (5, 14) THEN 1 WHEN (d5.v4 & 15) IN (1, 2) OR (t2.o & 15) IN (1, 2) THEN 0 ELSE \
         sparql_eval('=', d5.v4, t2.o) = 23 END) WHERE t1.g = 0 AND t1.p = ?2",
    ),
    (
        "UNION with NULL padding",
        "SELECT ?x WHERE { { ?x :p ?y } UNION { ?x :q ?z } }",
        "SELECT d3.v0 AS v0 FROM (SELECT t1.s AS v0, t1.o AS v1, NULL AS v2 FROM main.rdf_quad AS t1 WHERE t1.g = \
         0 AND t1.p = ?1 UNION ALL SELECT t2.s AS v0, NULL AS v1, t2.o AS v2 FROM main.rdf_quad AS t2 WHERE t2.g \
         = 0 AND t2.p = ?2) AS d3",
    ),
    (
        "MINUS with a certainly shared variable",
        "SELECT ?x WHERE { ?x a :Person MINUS { ?x :knows ?y } }",
        "SELECT t1.s AS v0 FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?1 AND t1.o = ?2 AND NOT EXISTS \
         (SELECT 1 FROM main.rdf_quad AS t2 WHERE t2.g = 0 AND t2.p = ?3 AND t1.s = t2.s)",
    ),
    (
        "MINUS with a nullable shared variable: the shared-domain test",
        "SELECT ?x ?v WHERE { ?x a :Person OPTIONAL { ?x :score ?v } MINUS { ?y :score ?v } }",
        "SELECT t1.s AS v0, t2.o AS v1 FROM main.rdf_quad AS t1 LEFT JOIN main.rdf_quad AS t2 ON t2.g = 0 AND \
         t2.p = ?1 AND t1.s = t2.s WHERE t1.g = 0 AND t1.p = ?2 AND t1.o = ?3 AND NOT EXISTS (SELECT 1 FROM \
         main.rdf_quad AS t3 WHERE t3.g = 0 AND t3.p = ?1 AND t2.o = t3.o)",
    ),
    (
        "correlated NOT EXISTS",
        "SELECT ?x WHERE { ?x a :Person FILTER NOT EXISTS { ?x :knows ?y FILTER(?y != ?x) } }",
        "SELECT t1.s AS v0 FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?1 AND t1.o = ?2 AND (NOT EXISTS \
         (SELECT 1 FROM main.rdf_quad AS t2 WHERE t2.g = 0 AND t1.s = t2.s AND t2.p = ?3 AND (NOT CASE WHEN t2.o \
         IS NULL OR t2.s IS NULL THEN NULL WHEN t2.o = t2.s AND (t2.o & 15) NOT IN (5, 14) THEN 1 WHEN (t2.o & \
         15) IN (1, 2) OR (t2.s & 15) IN (1, 2) THEN 0 ELSE sparql_eval('=', t2.o, t2.s) = 23 END)))",
    ),
    (
        "VALUES with UNDEF",
        "SELECT ?x ?v WHERE { VALUES (?x ?v) { (:a 1) (:b UNDEF) } ?x :p ?w }",
        "SELECT d1.column1 AS v0, d1.column2 AS v1 FROM (VALUES (?1, ?2), (?3, NULL)) AS d1, main.rdf_quad AS t2 \
         WHERE t2.g = 0 AND t2.p = ?4 AND d1.column1 = t2.s",
    ),
    (
        "BIND with the inline-int arithmetic fast path",
        "SELECT ?x ?w WHERE { ?x :age ?a BIND(?a + 1 AS ?w) }",
        "SELECT t1.s AS v0, CASE WHEN (t1.o & 15) = 6 AND ((t1.o >> 4) + ?1) BETWEEN -576460752303423488 AND \
         576460752303423487 THEN (((t1.o >> 4) + ?1) << 4) | 6 ELSE sparql_eval('+', t1.o, ?2) END AS v2 FROM \
         main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?3",
    ),
    (
        "GROUP BY, COUNT in SQL, SUM as a helper aggregate, HAVING, ORDER BY sort keys, LIMIT/OFFSET",
        "SELECT ?n (COUNT(DISTINCT ?x) AS ?c) (SUM(?a) AS ?t) WHERE { ?x :name ?n ; :age ?a } GROUP BY ?n HAVING \
         (COUNT(*) > 1) ORDER BY DESC(?c) LIMIT 5 OFFSET 2",
        "SELECT d3.v1 AS v1, d3.v3 AS v6, d3.v4 AS v7 FROM (SELECT t1.o AS v1, (COUNT(DISTINCT t1.s) << 4) | 6 AS \
         v3, sparql_sum(t2.o) AS v4, (COUNT(*) << 4) | 6 AS v5 FROM main.rdf_quad AS t1, main.rdf_quad AS t2 \
         WHERE t1.g = 0 AND t1.p = ?1 AND t2.g = 0 AND t1.s = t2.s AND t2.p = ?2 GROUP BY t1.o) AS d3 WHERE \
         ((d3.v5 & 15) = 6 AND (d3.v5 >> 4) > ?3 OR (d3.v5 & 15) <> 6 AND sparql_num(d3.v5) > ?3) ORDER BY \
         sparql_sortkey(d3.v3) DESC LIMIT ?4 OFFSET ?5",
    ),
    (
        "COUNT(*) without GROUP BY",
        "SELECT (COUNT(*) AS ?c) WHERE { ?s ?p ?o }",
        "SELECT d2.v3 AS v4 FROM (SELECT (COUNT(*) << 4) | 6 AS v3 FROM main.rdf_quad AS t1 WHERE t1.g = 0) AS d2",
    ),
    (
        "GROUP_CONCAT with a short separator",
        "SELECT ?n (GROUP_CONCAT(?x; SEPARATOR=\", \") AS ?all) WHERE { ?x :name ?n } GROUP BY ?n",
        "SELECT d2.v1 AS v1, d2.v2 AS v3 FROM (SELECT t1.o AS v1, sparql_group_concat(t1.s, ?1) AS v2 FROM \
         main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?2 GROUP BY t1.o) AS d2",
    ),
    (
        "DISTINCT ordered by a non-projected variable: first position in the ORDER BY sequence",
        "SELECT DISTINCT ?x WHERE { ?x :age ?a } ORDER BY ?a",
        "SELECT v0 FROM (SELECT t1.s AS v0, row_number() OVER (ORDER BY sparql_sortkey(t1.o)) AS rn FROM \
         main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.p = ?1) GROUP BY v0 ORDER BY min(rn)",
    ),
    (
        "p* from a constant: seeded WITH RECURSIVE",
        "SELECT ?y WHERE { :a :sub* ?y }",
        "SELECT d3.o AS v0 FROM (WITH RECURSIVE r2(n) AS (SELECT ?1 UNION SELECT k1.o FROM r2, main.rdf_quad AS \
         k1 WHERE k1.s = r2.n AND k1.g = 0 AND k1.p = ?2) SELECT ?1 AS s, n AS o FROM r2) AS d3",
    ),
    (
        "p+ with both ends free: full closure",
        "SELECT ?x ?y WHERE { ?x :sub+ ?y }",
        "SELECT d3.s AS v0, d3.o AS v1 FROM (WITH RECURSIVE r2(s, o) AS (SELECT s, o FROM (SELECT s, o FROM \
         main.rdf_quad WHERE g = 0 AND p = ?1) UNION SELECT r2.s, k1.o FROM r2, main.rdf_quad AS k1 WHERE k1.s = \
         r2.o AND k1.g = 0 AND k1.p = ?1) SELECT s, o FROM r2) AS d3",
    ),
    (
        "GRAPH ?g pushed into the pattern",
        "SELECT ?g ?x WHERE { GRAPH ?g { ?x :likes :tea } }",
        "SELECT t1.g AS v0, t1.s AS v1 FROM main.rdf_quad AS t1 WHERE t1.g <> 0 AND t1.p = ?1 AND t1.o = ?2",
    ),
    (
        "GRAPH <c> over an empty pattern: dataset membership",
        "SELECT * WHERE { GRAPH :g1 { } }",
        "SELECT NULL AS u WHERE (EXISTS (SELECT 1 FROM main.rdf_graph WHERE g = ?1) OR EXISTS (SELECT 1 FROM \
         main.rdf_quad WHERE g = ?1))",
    ),
    (
        "FROM with two graphs: RDF merge",
        "SELECT ?x FROM :g1 FROM :g2 WHERE { ?x :likes ?y }",
        "SELECT t1.s AS v0 FROM (SELECT DISTINCT s, p, o FROM main.rdf_quad WHERE g IN (?1, ?2)) AS t1 WHERE t1.p \
         = ?3",
    ),
    (
        "ASK",
        "ASK { :a :knows :b }",
        "SELECT NULL AS u FROM main.rdf_quad AS t1 WHERE t1.g = 0 AND t1.s = ?1 AND t1.p = ?2 AND t1.o = ?3 LIMIT \
         ?4",
    ),
];

#[test]
fn golden_sql() {
    let mut failures = Vec::new();
    for (what, query, expected) in GOLDEN {
        let actual = sql(query).sql;
        if actual != *expected {
            failures.push(format!("{what}
  query:    {query}
  expected: {expected}
  actual:   {actual}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("
"));
}

#[test]
fn parameters_and_columns() {
    use sq_core::TermId;
    let q = sql("SELECT ?x ?w WHERE { ?x :age ?a BIND(?a + 1 AS ?w) }");
    // The integer payload 1, the id of "1"^^xsd:integer, then :age.
    assert_eq!(q.params[..2], [1, TermId::from_int(1).unwrap().raw()]);
    assert_eq!(q.columns, ["x", "w"]);
    // Every placeholder ?1..?n appears, numbered by first use.
    for n in 1..=q.params.len() {
        assert!(q.sql.contains(&format!("?{n}")), "?{n} missing from {}", q.sql);
    }
    assert!(!q.sql.contains(&format!("?{}", q.params.len() + 1)));
    let ask = sql("ASK { :a :knows :b }");
    assert!(ask.columns.is_empty());
    assert_eq!(*ask.params.last().unwrap(), 1, "ASK stops at the first row");
}

#[test]
fn queries_with_no_projected_variable_have_one_placeholder_column() {
    let q = sql("SELECT * WHERE { :a :knows :b }");
    assert!(q.columns.is_empty());
    assert!(q.sql.starts_with("SELECT NULL AS u FROM"), "{}", q.sql);
}
