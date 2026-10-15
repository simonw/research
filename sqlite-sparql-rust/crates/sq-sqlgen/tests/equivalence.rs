//! Baseline SQL ≡ spareval on a hand-written dataset that mixes every term kind (IRIs, blank
//! nodes, short and long simple literals, language tags, canonical and non-canonical numerics,
//! doubles, booleans, dateTimes, unknown datatypes) and named graphs, over a corpus of queries
//! covering the whole supported subset.
mod common;

use common::{DataFile, Engine, check};

const DATA: &str = r#"
@prefix : <http://ex.org/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

:alice a :Person ; :name "Alice" ; :age 30 ; :height 1.70 ; :knows :bob , :carol ;
    :born "1994-05-01T10:00:00"^^xsd:dateTime ; :email <mailto:alice@ex.org> ; :nick "ally"@en .
:bob a :Person ; :name "Bob the Builder" ; :age 25 ; :height "1.80"^^xsd:decimal ; :knows :carol ;
    :born "1999-01-01T00:00:00Z"^^xsd:dateTime ; :nick "bobby"@en , "bobi"@de .
:carol a :Person ; :name "Carol"@en ; :age "41"^^xsd:integer ; :knows :alice , _:anon ;
    :score 1.5e0 ; :active true .
:dave a :Robot ; :name "DAVE-9000" ; :age "07"^^xsd:integer ; :score "NaN"^^xsd:double ;
    :active "false"^^xsd:boolean ; :code "x1"^^<http://ex.org/dt> .
_:anon a :Person ; :name "Anonymous" ; :age 99 ; :knows :alice .
:eve a :Person ; :age "twenty"^^xsd:integer ; :knows :eve .

:p1 :sub :p2 . :p2 :sub :p3 . :p3 :sub :p1 . :p3 :sub :p4 . :p5 :other :p1 .

:g1 { :alice :likes :tea . :bob :likes :coffee . :x :inG1 "only in g1" . }
:g2 { :alice :likes :coffee . :carol :likes :tea . :alice :knows :dave . }
"#;

/// Queries every engine must agree on (prefix `:` is added).
const QUERIES: &[&str] = &[
    // ---------------- BGPs
    "SELECT * WHERE { ?s :knows ?o }",
    "SELECT ?s ?n WHERE { ?s a :Person ; :name ?n }",
    "SELECT ?x WHERE { ?x :knows ?x }",
    "SELECT ?p ?o WHERE { :alice ?p ?o }",
    "SELECT ?s WHERE { ?s :name \"Alice\" }",
    "SELECT ?s WHERE { ?s :name \"Carol\"@en }",
    "SELECT ?s WHERE { ?s :age 30 }",
    "SELECT ?s WHERE { ?s :age 7 }",
    "SELECT ?s WHERE { ?s :name \"nobody here\" }",
    "SELECT ?s WHERE { ?s :absentPredicate ?o }",
    "SELECT ?a ?b ?c WHERE { ?a :knows ?b . ?b :knows ?c . ?c :knows ?a }",
    "SELECT ?x ?y WHERE { ?x :knows [ :knows ?y ] }",
    "SELECT * WHERE { :alice :knows :bob }",
    "SELECT * WHERE { :alice :knows :dave }",
    // ---------------- FILTER: comparisons
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(?a > 26) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(26 < ?a) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(?a <= 30) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(!(?a > 26)) }",
    "SELECT ?s ?h WHERE { ?s :height ?h FILTER(?h > 1.75) }",
    "SELECT ?s ?h WHERE { ?s :height ?h FILTER(?h >= 1.7) }",
    "SELECT ?s ?h WHERE { ?s :height ?h FILTER(!(?h < 1.75)) }",
    "SELECT ?s ?v WHERE { ?s :score ?v FILTER(?v < 2) }",
    "SELECT ?s ?v WHERE { ?s :score ?v FILTER(?v != ?v) }",
    "SELECT ?s ?v WHERE { ?s :score ?v FILTER(?v = ?v) }",
    "SELECT ?a ?b WHERE { ?a :age ?x . ?b :age ?y FILTER(?x < ?y) }",
    "SELECT ?a ?b WHERE { ?a :height ?x . ?b :age ?y FILTER(?x < ?y) }",
    "SELECT ?s WHERE { ?s :born ?d FILTER(?d > \"1995-01-01T00:00:00\"^^xsd:dateTime) }",
    "SELECT ?s WHERE { ?s :born ?d FILTER(?d < \"1995-01-01T00:00:00Z\"^^xsd:dateTime) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(?n < \"B\") }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(?n >= \"Bob\") }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(?a > \"20\") }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(!(?a > \"20\")) }",
    // ---------------- FILTER: equality and IN
    "SELECT ?s WHERE { ?s :knows ?o FILTER(?o = :carol) }",
    "SELECT ?s WHERE { ?s :knows ?o FILTER(?o != :carol) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(?n = \"Alice\") }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(!(?n = \"Alice\")) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(?n != \"Alice\") }",
    "SELECT ?s ?n WHERE { ?s :nick ?n FILTER(?n = \"ally\"@en) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(?a = 30.0) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(?a = 7) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(?a != 7) }",
    "SELECT ?s ?c WHERE { ?s :code ?c FILTER(?c = \"x1\") }",
    "SELECT ?s ?c WHERE { ?s :code ?c FILTER(!(?c = \"x1\")) }",
    "SELECT ?s ?c WHERE { ?s :code ?c FILTER(?c != \"x1\") }",
    "SELECT ?a ?b WHERE { ?a :age ?x . ?b :age ?y FILTER(?x = ?y && ?a != ?b) }",
    "SELECT ?a ?b WHERE { ?a :likes ?x . ?b :likes ?y FILTER(?x = ?y) }",
    "SELECT ?s WHERE { ?s :knows ?o FILTER(?o IN (:bob, :dave)) }",
    "SELECT ?s WHERE { ?s :knows ?o FILTER(?o NOT IN (:bob, :dave)) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(?a IN (25, 41, \"x\")) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(?a NOT IN (25, 41)) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(?n IN (\"Alice\", \"Carol\"@en)) }",
    "SELECT ?s WHERE { ?s :name ?n FILTER(?s IN ()) }",
    "SELECT ?a ?b WHERE { ?a :knows ?b FILTER(sameTerm(?a, ?b)) }",
    // ---------------- FILTER: logic, EBV, BOUND, errors
    "SELECT ?s WHERE { ?s :active ?b FILTER(?b) }",
    "SELECT ?s WHERE { ?s :active ?b FILTER(!?b) }",
    "SELECT ?s ?v WHERE { ?s :score ?v FILTER(?v) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(?n) }",
    "SELECT ?s WHERE { ?s :age ?a FILTER(?a > 26 || ?a = \"twenty\") }",
    "SELECT ?s WHERE { ?s :age ?a FILTER(?a > 100 || ?s = :eve) }",
    "SELECT ?s WHERE { ?s :age ?a FILTER(!(?a > 100 && ?a < 0)) }",
    "SELECT ?s WHERE { ?s :age ?a FILTER(!(?a > 100) || !(?a < 50)) }",
    "SELECT ?s ?o WHERE { ?s :name ?n OPTIONAL { ?s :score ?o } FILTER(!BOUND(?o)) }",
    "SELECT ?s ?o WHERE { ?s :name ?n OPTIONAL { ?s :score ?o } FILTER(BOUND(?o)) }",
    "SELECT ?s WHERE { ?s :name ?n FILTER(BOUND(?nope)) }",
    "SELECT ?s WHERE { ?s :name ?n FILTER(?nope > 1) }",
    "SELECT ?s WHERE { ?s :name ?n FILTER(!(?nope > 1)) }",
    "SELECT ?s WHERE { ?s :name ?n FILTER(true) }",
    "SELECT ?s WHERE { ?s :name ?n FILTER(0) }",
    "SELECT ?s WHERE { ?s :name ?n FILTER(\"\") }",
    // ---------------- FILTER: builtins
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(REGEX(?n, \"^b\", \"i\")) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(REGEX(?n, \"o\")) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(!REGEX(?n, \"o\")) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(REGEX(?o, \"ex\")) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(REGEX(STR(?o), \"ex.org/[a-c]\")) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(CONTAINS(?n, \"o\")) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(!CONTAINS(?n, \"o\")) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(STRSTARTS(?n, \"Bo\")) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(STRENDS(?n, \"ol\")) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(STRENDS(?n, \"\")) }",
    "SELECT ?s ?n WHERE { ?s :nick ?n FILTER(STRSTARTS(?n, \"bob\"@en)) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(CONTAINS(?o, \"a\")) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(isIRI(?o)) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(isBlank(?o)) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(isLiteral(?o)) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(isNumeric(?o)) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(!isLiteral(?o)) }",
    "SELECT ?s ?n WHERE { ?s :nick ?n FILTER(LANG(?n) = \"en\") }",
    "SELECT ?s ?n WHERE { ?s :nick ?n FILTER(LANGMATCHES(LANG(?n), \"DE\")) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(DATATYPE(?o) = xsd:integer) }",
    "SELECT ?s ?o WHERE { ?s ?p ?o FILTER(?o + 1 > 30) }",
    "SELECT ?s WHERE { ?s :age ?a FILTER(IF(?a > 30, true, false)) }",
    "SELECT ?s WHERE { ?s :age ?a FILTER(COALESCE(?a > 30, true)) }",
    // ---------------- OPTIONAL
    "SELECT ?s ?v WHERE { ?s a :Person OPTIONAL { ?s :score ?v } }",
    "SELECT ?s ?n ?v WHERE { ?s a :Person OPTIONAL { ?s :name ?n ; :score ?v } }",
    "SELECT ?s ?o ?n WHERE { ?s :knows ?o OPTIONAL { ?o :nick ?n FILTER(LANG(?n) = \"en\") } }",
    "SELECT ?s ?a ?b WHERE { ?s :age ?a OPTIONAL { ?s :knows ?o . ?o :age ?b FILTER(?b > ?a) } }",
    "SELECT ?s ?v ?w WHERE { ?s a :Person OPTIONAL { ?s :score ?v } OPTIONAL { ?s :nick ?w } }",
    "SELECT ?s ?o ?z WHERE { ?s a :Person OPTIONAL { ?s :knows ?o } OPTIONAL { ?o :knows ?z } }",
    "SELECT ?s ?o ?n WHERE { ?s a :Person OPTIONAL { ?s :knows ?o OPTIONAL { ?o :nick ?n } } }",
    "SELECT ?s ?v WHERE { ?s a :Person OPTIONAL { ?s :score ?v } ?s :age ?a }",
    "SELECT ?s ?v WHERE { ?s a :Person OPTIONAL { ?s :score ?v } FILTER(?s != :carol) }",
    "SELECT ?x ?y WHERE { OPTIONAL { ?x :likes ?y } }",
    "SELECT ?x ?y WHERE { ?x :age ?a OPTIONAL { BIND(?a + 1 AS ?y) } }",
    "SELECT ?x ?y WHERE { ?x :age ?a OPTIONAL { ?x :knows ?y } OPTIONAL { ?y :name ?a } }",
    "SELECT ?x ?c WHERE { ?x :age ?a OPTIONAL { VALUES ?c { :carol } ?x :knows ?c } }",
    "SELECT ?x ?k WHERE { ?x :age ?a OPTIONAL { ?x :knows :carol BIND(\"k\" AS ?k) } }",
    // ---------------- UNION
    "SELECT ?s ?x ?y WHERE { { ?s :age ?x } UNION { ?s :score ?y } }",
    "SELECT ?s WHERE { { ?s a :Robot } UNION { ?s :age 30 } UNION { ?s :age 30 } }",
    "SELECT ?s ?n WHERE { { ?s :name ?n } UNION { ?s :nick ?n } FILTER(LANG(?n) != \"\") }",
    "SELECT ?s ?n ?k WHERE { { ?s :name ?n } UNION { ?s :nick ?k } ?s :knows :carol }",
    "SELECT ?s ?v WHERE { ?s a :Person OPTIONAL { { ?s :score ?v } UNION { ?s :height ?v } } }",
    // ---------------- MINUS
    "SELECT ?s WHERE { ?s a :Person MINUS { ?s :score ?v } }",
    "SELECT ?s WHERE { ?s a :Person MINUS { ?x :score ?v } }",
    "SELECT ?s ?o WHERE { ?s :knows ?o MINUS { ?o a :Person } }",
    "SELECT ?s ?v WHERE { ?s a :Person OPTIONAL { ?s :score ?v } MINUS { ?x :score ?v } }",
    "SELECT ?s WHERE { ?s a :Person MINUS { ?s :knows ?o . ?o :age ?a FILTER(?a > 40) } }",
    // ---------------- EXISTS
    "SELECT ?s WHERE { ?s a :Person FILTER EXISTS { ?s :score ?v } }",
    "SELECT ?s WHERE { ?s a :Person FILTER NOT EXISTS { ?s :score ?v } }",
    "SELECT ?s WHERE { ?s :age ?a FILTER NOT EXISTS { ?s :knows ?o . ?o :age ?b FILTER(?b > ?a) } }",
    "SELECT ?s WHERE { ?s :age ?a FILTER EXISTS { ?o :age ?b FILTER(?b = ?a + 11) } }",
    "SELECT ?s ?v WHERE { ?s a :Person OPTIONAL { ?s :score ?v } FILTER NOT EXISTS { ?x :score ?v } }",
    "SELECT ?s WHERE { ?s a :Person FILTER EXISTS { ?s :knows ?o FILTER NOT EXISTS { ?o :knows ?s } } }",
    "SELECT ?s WHERE { ?s a :Person FILTER EXISTS { ?s :knows ?o OPTIONAL { ?o :score ?v } FILTER(!BOUND(?v)) } }",
    "SELECT ?s WHERE { ?s a :Person FILTER EXISTS { { ?s :score ?v } UNION { ?s :height ?v } } }",
    "SELECT ?s (EXISTS { ?s :score ?v } AS ?has) WHERE { ?s a :Person }",
    "SELECT ?s WHERE { ?s :name ?n FILTER(EXISTS { ?s :age 30 } || EXISTS { ?s :age 25 }) }",
    "SELECT ?s ?o WHERE { ?s :knows ?o OPTIONAL { ?o :age ?a FILTER EXISTS { ?o :knows ?s } } }",
    // ---------------- VALUES
    "SELECT ?s ?a WHERE { VALUES ?s { :alice :bob } ?s :age ?a }",
    "SELECT ?s ?a WHERE { ?s :age ?a VALUES ?s { :carol } }",
    "SELECT ?s ?a WHERE { VALUES (?s ?a) { (:alice 30) (:bob UNDEF) (:zed 1) } ?s :age ?a }",
    "SELECT ?s ?n WHERE { VALUES (?s ?n) { (:alice UNDEF) (UNDEF \"Bob the Builder\") } ?s :name ?n }",
    "SELECT ?x WHERE { VALUES ?x { } }",
    "SELECT ?x ?y WHERE { VALUES ?x { 1 2 } VALUES ?y { \"a\" \"b\" } }",
    "SELECT ?s ?a WHERE { ?s :age ?a } VALUES ?a { 30 41 }",
    "SELECT ?s ?v WHERE { ?s a :Person OPTIONAL { VALUES (?s ?v) { (:alice 1) (:bob 2) } } }",
    // ---------------- BIND and SELECT expressions
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a * 2 AS ?b) }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a - 30 AS ?b) FILTER(?b >= 0) }",
    "SELECT ?s ?b WHERE { ?s :height ?h BIND(?h + 1 AS ?b) }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a / 2 AS ?b) }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(-?a AS ?b) }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a + 576460752303423487 AS ?b) }",
    "SELECT ?s ?u WHERE { ?s :name ?n BIND(UCASE(?n) AS ?u) }",
    "SELECT ?s ?c WHERE { ?s :name ?n BIND(CONCAT(?n, \"!\") AS ?c) }",
    "SELECT ?s ?c WHERE { ?s :name ?n BIND(SUBSTR(?n, 2, 3) AS ?c) }",
    "SELECT ?s ?l WHERE { ?s :name ?n BIND(STRLEN(?n) AS ?l) FILTER(?l > 4) }",
    "SELECT ?s ?t WHERE { ?s :age ?a BIND(IF(?a > 30, \"old\", \"young\") AS ?t) }",
    "SELECT ?s ?t WHERE { ?s :name ?n OPTIONAL { ?s :score ?v } BIND(COALESCE(?v, ?n) AS ?t) }",
    "SELECT ?s ?d WHERE { ?s ?p ?o BIND(DATATYPE(?o) AS ?d) FILTER(isLiteral(?o)) }",
    "SELECT ?s ?k WHERE { ?s :name ?n BIND(STR(?s) AS ?k) }",
    "SELECT ?x ?n WHERE { :alice :name ?n BIND(:alice AS ?x) }",
    "SELECT ?s ?o WHERE { ?s :name ?n BIND(STR(\"Alice\") AS ?m) ?s :name ?m BIND(?m AS ?o) }",
    "SELECT ?s ?i WHERE { ?s :age ?a BIND(xsd:integer(?a) AS ?i) }",
    "SELECT ?s ?i WHERE { ?s :height ?h BIND(xsd:string(?h) AS ?i) }",
    "SELECT ?s (?a + 1 AS ?b) WHERE { ?s :age ?a }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a > 26 AS ?b) }",
    "SELECT ?s ?b WHERE { ?s :name ?n BIND(isLiteral(?n) AS ?b) }",
    "SELECT ?s ?b WHERE { ?s :name ?n BIND(REGEX(?n, \"a\") AS ?b) }",
    // ---------------- computed bindings read by column (settled blocks)
    "SELECT ?s ?d WHERE { ?s :age ?a BIND(?a + 1 AS ?b) BIND(?b * ?b AS ?c) BIND(?c - ?b AS ?d) FILTER(?d > 0) } \
     ORDER BY ?d",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a + 1 AS ?b) } ORDER BY DESC(?b) ?s",
    "SELECT ?s ?c WHERE { ?s :name ?n BIND(UCASE(?n) AS ?u) BIND(CONCAT(?u, \"/\", ?u) AS ?c) }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(COALESCE(?a, ?a) AS ?b) BIND(COALESCE(?b, ?b) AS ?c) FILTER(BOUND(?c)) }",
    "SELECT ?s ?b ?o WHERE { ?s :age ?a BIND(?a - 5 AS ?b) ?o :age ?b }",
    "SELECT ?s ?b ?o WHERE { ?s :age ?a BIND(?a * 1 AS ?b) OPTIONAL { ?o :age ?b FILTER(?o != ?s) } }",
    "SELECT ?x ?y WHERE { ?x :age ?a OPTIONAL { ?x :knows ?o BIND(STR(?o) AS ?y) FILTER(STRLEN(?y) > 17) } }",
    "SELECT ?x ?y WHERE { ?x :age ?a OPTIONAL { ?x :knows ?o BIND(STR(?o) AS ?y) } FILTER(!BOUND(?y) || ?y > \"h\") }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a + 0 AS ?b) MINUS { ?o :age ?b FILTER(?o = :bob) } }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a + 0 AS ?b) FILTER EXISTS { ?o :age ?b FILTER(?o != ?s) } }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a + 11 AS ?b) FILTER NOT EXISTS { ?o :age ?b } }",
    "SELECT ?s WHERE { ?s :age ?a FILTER EXISTS { BIND(?a AS ?a2) BIND(?a2 + 1 AS ?x) FILTER(?x > 30) } }",
    "SELECT (SUM(?b) AS ?t) (MAX(?b) AS ?m) (COUNT(DISTINCT ?b) AS ?c) WHERE { ?s :age ?a BIND(?a * 2 AS ?b) }",
    "SELECT ?k (COUNT(*) AS ?c) WHERE { ?s :age ?a BIND(?a > 30 AS ?k) } GROUP BY ?k",
    "SELECT ?s ?b WHERE { { ?s :age ?a BIND(?a * 2 AS ?b) } UNION { ?s :height ?h BIND(?h + 1 AS ?b) } \
     FILTER(?b > 3) }",
    "SELECT ?s ?b WHERE { { SELECT ?s (?a * 2 AS ?b) WHERE { ?s :age ?a } } FILTER(?b > 55 || ?b < 20) }",
    "SELECT DISTINCT ?b WHERE { ?s :age ?a BIND(?a - ?a AS ?b) } ORDER BY ?b",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND(?a * 3 AS ?b) } ORDER BY ?b LIMIT 2",
    // ---------------- IN over a computed value; STRENDS
    "SELECT ?s WHERE { ?s :name ?n FILTER(!(LCASE(?n) IN (\"alice\", \"carol\", \"bob the builder\"))) }",
    "SELECT ?s WHERE { ?s :name ?n FILTER(LCASE(?n) IN (\"alice\", \"bob the builder\")) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER((?a + 1) IN (26, 31, \"x\", ?a)) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER(!((?a + 1) IN (26, 31.0, \"x\"))) }",
    "SELECT ?s ?a WHERE { ?s :age ?a FILTER((?a IN (25, 30)) IN (true, ?nope)) }",
    "SELECT ?s ?b WHERE { ?s :age ?a BIND((?a * 2) IN (50, 60) AS ?b) }",
    "SELECT ?s ?n WHERE { ?s :name ?n FILTER(STRENDS(?n, \"the Builder\") || STRENDS(UCASE(?n), \"ICE\")) }",
    "SELECT ?s ?n WHERE { ?s :nick ?n FILTER(STRENDS(?n, \"by\") && !STRENDS(?n, \"\")) }",
    // ---------------- aggregates
    "SELECT (COUNT(*) AS ?c) WHERE { ?s ?p ?o }",
    "SELECT (COUNT(*) AS ?c) WHERE { ?s :nope ?o }",
    "SELECT (COUNT(?o) AS ?c) WHERE { ?s a :Person OPTIONAL { ?s :score ?o } }",
    "SELECT (COUNT(DISTINCT ?o) AS ?c) WHERE { ?s :knows ?o }",
    "SELECT (COUNT(DISTINCT *) AS ?c) WHERE { ?s :likes ?o }",
    "SELECT ?s (COUNT(*) AS ?c) WHERE { ?s :knows ?o } GROUP BY ?s",
    "SELECT ?s (COUNT(*) AS ?c) WHERE { ?s :knows ?o } GROUP BY ?s HAVING (COUNT(*) > 1)",
    "SELECT ?s (SUM(?a) AS ?t) WHERE { ?s :age ?a } GROUP BY ?s",
    "SELECT (SUM(?a) AS ?t) (AVG(?a) AS ?m) WHERE { ?s :age ?a FILTER(isNumeric(?a)) }",
    "SELECT (SUM(?h) AS ?t) (AVG(?h) AS ?m) (MIN(?h) AS ?lo) (MAX(?h) AS ?hi) WHERE { ?s :height ?h }",
    "SELECT (SUM(?a) AS ?t) WHERE { ?s :nope ?a }",
    "SELECT (AVG(?a) AS ?t) WHERE { ?s :nope ?a }",
    "SELECT (MIN(?a) AS ?t) WHERE { ?s :nope ?a }",
    "SELECT ?p (MIN(?o) AS ?lo) (MAX(?o) AS ?hi) WHERE { VALUES ?p { :age :height :born :active :sub } ?s ?p ?o } \
     GROUP BY ?p",
    "SELECT ?s (GROUP_CONCAT(?n; SEPARATOR=\",\") AS ?all) WHERE { ?s :name ?n FILTER(?s = :bob) } GROUP BY ?s",
    "SELECT (SAMPLE(?a) AS ?x) WHERE { :alice :age ?a }",
    "SELECT (COUNT(*) AS ?c) WHERE { ?s :age ?a } GROUP BY ?s",
    "SELECT ?s WHERE { ?s :age ?a } GROUP BY ?s",
    "SELECT ?k (COUNT(*) AS ?c) WHERE { ?s :age ?a } GROUP BY (?a > 30 AS ?k)",
    "SELECT ?o (COUNT(?s) AS ?c) WHERE { ?s :knows ?o } GROUP BY ?o ORDER BY DESC(?c) ?o",
    "SELECT (SUM(?a) AS ?t) WHERE { ?s :age ?a }",
    "SELECT (COUNT(*) AS ?c) WHERE { { ?s :age ?a } UNION { ?s :score ?a } }",
    "SELECT ?x (COUNT(?y) AS ?c) WHERE { ?x a :Person OPTIONAL { ?x :knows ?y } } GROUP BY ?x",
    // ---------------- ORDER BY / DISTINCT / slices
    "SELECT ?s ?a WHERE { ?s :age ?a } ORDER BY ?a ?s",
    "SELECT ?s ?a WHERE { ?s :age ?a } ORDER BY DESC(?a) ?s",
    "SELECT ?s ?o WHERE { ?s ?p ?o } ORDER BY ?o ?s ?p",
    "SELECT ?s ?v WHERE { ?s a :Person OPTIONAL { ?s :score ?v } } ORDER BY ?v ?s",
    "SELECT ?s ?v WHERE { ?s a :Person OPTIONAL { ?s :score ?v } } ORDER BY DESC(?v) ?s",
    "SELECT ?s ?a WHERE { ?s :age ?a } ORDER BY ?a ?s LIMIT 3",
    "SELECT ?s ?a WHERE { ?s :age ?a } ORDER BY ?a ?s LIMIT 2 OFFSET 2",
    "SELECT ?s ?a WHERE { ?s :age ?a } ORDER BY ?a ?s OFFSET 4",
    "SELECT ?s ?n WHERE { ?s :name ?n } ORDER BY STRLEN(?n) ?s",
    "SELECT DISTINCT ?s WHERE { ?s :knows ?o }",
    "SELECT DISTINCT ?o WHERE { ?s :likes ?o }",
    "SELECT DISTINCT ?o WHERE { GRAPH ?g { ?s :likes ?o } } ORDER BY ?o",
    "SELECT REDUCED ?s WHERE { ?s :knows ?o }",
    "SELECT ?s WHERE { ?s :knows ?o } LIMIT 0",
    // ---------------- sub-selects
    "SELECT ?s ?c WHERE { ?s a :Person { SELECT ?s (COUNT(*) AS ?c) WHERE { ?s :knows ?o } GROUP BY ?s } }",
    "SELECT ?s ?o WHERE { { SELECT ?s WHERE { ?s :age ?a } ORDER BY ?a LIMIT 2 } ?s :knows ?o }",
    "SELECT ?s ?a WHERE { ?s :name ?n { SELECT ?s WHERE { ?s :knows ?a } } }",
    "SELECT * WHERE { { SELECT (MAX(?a) AS ?m) WHERE { ?s :age ?a FILTER(isNumeric(?a)) } } ?s :age ?m }",
    "SELECT ?s WHERE { ?s a :Person FILTER EXISTS { SELECT ?s WHERE { ?s :score ?v } } }",
    // ---------------- property paths
    "SELECT ?x WHERE { :p1 :sub+ ?x }",
    "SELECT ?x WHERE { :p1 :sub* ?x }",
    "SELECT ?x WHERE { ?x :sub+ :p1 }",
    "SELECT ?x WHERE { ?x :sub* :p4 }",
    "SELECT ?x WHERE { :p1 :sub? ?x }",
    "SELECT ?x ?y WHERE { ?x :sub+ ?y }",
    "SELECT ?x ?y WHERE { ?x :sub* ?y }",
    "SELECT ?x ?y WHERE { ?x :sub? ?y }",
    "SELECT ?x WHERE { ?x :sub+ ?x }",
    "SELECT ?x WHERE { ?x :sub* ?x }",
    "SELECT * WHERE { :p1 :sub+ :p4 }",
    "SELECT * WHERE { :p4 :sub+ :p1 }",
    "SELECT * WHERE { :p1 :sub* :p1 }",
    "SELECT ?x WHERE { ?x ^:sub :p1 }",
    "SELECT ?x WHERE { :p1 (^:sub)+ ?x }",
    "SELECT ?x WHERE { :p1 :sub/:sub ?x }",
    "SELECT ?x WHERE { :p5 :other/:sub+ ?x }",
    "SELECT ?x WHERE { :p1 (:sub|:other)+ ?x }",
    "SELECT ?x WHERE { :p1 (:sub|^:other)* ?x }",
    "SELECT ?x WHERE { ?x (:sub/:sub)+ :p1 }",
    "SELECT ?x ?y WHERE { ?x (:sub|:other) ?y }",
    "SELECT ?x WHERE { :p1 !(:other) ?x }",
    "SELECT ?x WHERE { :p1 !(:sub|^:sub) ?x }",
    "SELECT ?x WHERE { :p1 !^:other ?x }",
    "SELECT ?s ?x WHERE { ?s :knows/:knows ?x }",
    "SELECT ?s ?x WHERE { ?s a :Person ; :knows+ ?x }",
    "SELECT ?s WHERE { ?s a :Person FILTER EXISTS { ?s :knows+ :eve } }",
    "SELECT ?s WHERE { ?s a :Person FILTER EXISTS { ?s :knows* :alice } }",
    // x+ is zero-length capable when x is
    "SELECT ?x WHERE { :p1 (:sub*)+ ?x }",
    "SELECT ?x ?y WHERE { ?x (:sub?)+ ?y }",
    "SELECT ?x WHERE { ?x (:sub*)+ :p4 }",
    "SELECT ?x WHERE { ?x ^(:sub*)+ :p1 }",
    "SELECT ?x WHERE { VALUES ?x { :p5 :p4 } FILTER NOT EXISTS { ?x (:sub?)+ :p1 } }",
    // ---------------- GRAPH and datasets
    "SELECT ?s ?o WHERE { GRAPH :g1 { ?s :likes ?o } }",
    "SELECT ?g ?s ?o WHERE { GRAPH ?g { ?s :likes ?o } }",
    "SELECT ?g ?s WHERE { GRAPH ?g { ?s :likes :tea . ?s :likes ?x } }",
    "SELECT ?g ?s ?o ?n WHERE { GRAPH ?g { ?s :likes ?o OPTIONAL { ?s :knows ?n } } }",
    "SELECT ?g ?s WHERE { GRAPH ?g { { ?s :likes :tea } UNION { ?s :inG1 ?x } } }",
    "SELECT ?g ?s WHERE { GRAPH ?g { ?s :likes ?o MINUS { ?s :knows ?x } } }",
    "SELECT ?g ?s WHERE { GRAPH ?g { ?s :likes ?o FILTER NOT EXISTS { ?s :knows ?x } } }",
    "SELECT ?g WHERE { GRAPH ?g { } }",
    "SELECT ?s WHERE { GRAPH :nope { ?s ?p ?o } }",
    "SELECT * WHERE { GRAPH :nope { } }",
    "SELECT * WHERE { GRAPH :g2 { } }",
    "SELECT ?x WHERE { GRAPH :g2 { BIND(1 AS ?x) } }",
    "SELECT ?s ?g WHERE { ?s :knows :carol . GRAPH ?g { ?s :likes ?o } }",
    "SELECT ?s ?o WHERE { GRAPH :g2 { ?s :knows ?o } ?o a ?t }",
    "SELECT ?s WHERE { ?s a :Person FILTER EXISTS { GRAPH ?g { ?s :likes :tea } } }",
    "SELECT ?x WHERE { GRAPH :g1 { :x :inG1 ?x } }",
    "SELECT ?s ?o FROM :g1 WHERE { ?s :likes ?o }",
    "SELECT ?s ?o FROM :g1 FROM :g2 WHERE { ?s :likes ?o }",
    "SELECT ?s ?o FROM :nope WHERE { ?s ?p ?o }",
    "SELECT ?g ?s FROM NAMED :g2 WHERE { GRAPH ?g { ?s :likes ?o } }",
    "SELECT ?g ?s FROM NAMED :g2 FROM NAMED :nope WHERE { GRAPH ?g { } }",
    "SELECT ?s FROM NAMED :g2 WHERE { GRAPH :g1 { ?s ?p ?o } }",
    "SELECT ?s FROM NAMED :g2 WHERE { ?s ?p ?o }",
    "SELECT ?s ?g FROM :g1 FROM NAMED :g2 WHERE { ?s :likes :tea . GRAPH ?g { ?s :likes ?x } }",
    "SELECT ?x FROM :g1 FROM :g2 WHERE { :alice :likes/^:likes ?x }",
    "SELECT ?x FROM :g1 WHERE { :alice :likes* ?x }",
    "SELECT ?x FROM :g1 FROM :g2 WHERE { :alice (:likes|:knows)+ ?x }",
    "SELECT ?x ?y FROM :g1 FROM :g2 WHERE { ?x :likes+ ?y }",
    "SELECT ?x FROM :g1 FROM :g2 WHERE { ?x :likes? :tea }",
    "SELECT ?y WHERE { GRAPH :alice { :p1 (:sub*)+ ?y } }",
    "SELECT ?y WHERE { GRAPH :alice { :p1 (:sub?)+ ?y } }",
    // ---------------- ASK
    "ASK { :alice :knows :bob }",
    "ASK { :alice :knows :eve }",
    "ASK { ?s :age ?a FILTER(?a > 90) }",
    "ASK { }",
    "ASK { GRAPH :g1 { ?s :likes :coffee } }",
];

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// Queries where spareval 0.2.7 deviates from the spec (the same reference bugs as in
/// tools/sq-w3c/reference_divergences.txt): (query, why, the spec's answer as N-Triples rows).
const SPEC_ANSWERS: &[(&str, &str, &[&[&str]])] = &[
    (
        "SELECT ?s ?v WHERE { ?s :score ?v FILTER(!(?v < 2)) }",
        "NaN < 2 is false (op:numeric-less-than), so its negation holds",
        &[&["<http://ex.org/dave>", "\"NaN\"^^<http://www.w3.org/2001/XMLSchema#double>"]],
    ),
    (
        "SELECT ?s WHERE { ?s :active ?b FILTER(?b < true) }",
        "xsd:boolean is ordered: false < true",
        &[&["<http://ex.org/dave>"]],
    ),
    (
        "SELECT ?o WHERE { ?s ?p ?o FILTER(isNumeric(?o) && STRLEN(STR(?o)) < 4) }",
        "lexical forms are kept: STR(\"1.70\"^^xsd:decimal) has four characters",
        &[&["25"], &["30"], &["41"], &["99"], &["\"07\""], &["\"NaN\"^^xsd:double"]],
    ),
    (
        "SELECT ?s (GROUP_CONCAT(?n) AS ?all) WHERE { ?s :name ?n FILTER(isIRI(?s)) } GROUP BY ?s",
        "GROUP_CONCAT returns a simple literal (SPEC 6.7)",
        &[
            &["<http://ex.org/alice>", "\"Alice\""],
            &["<http://ex.org/bob>", "\"Bob the Builder\""],
            &["<http://ex.org/carol>", "\"Carol\""],
            &["<http://ex.org/dave>", "\"DAVE-9000\""],
        ],
    ),
    (
        "SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o FILTER(false) }",
        "an aggregate without GROUP BY over no rows still has one group (SPEC 6.7)",
        &[&["0"]],
    ),
    (
        "SELECT ?x WHERE { :absent :sub* ?x }",
        "a constant endpoint of a zero-length path matches itself even when absent from the graph",
        &[&["<http://ex.org/absent>"]],
    ),
    (
        "SELECT ?x WHERE { ?x :sub* :absent }",
        "a constant endpoint of a zero-length path matches itself even when absent from the graph",
        &[&["<http://ex.org/absent>"]],
    ),
    (
        "SELECT ?x WHERE { :absent :sub? ?x }",
        "a constant endpoint of a zero-length path matches itself even when absent from the graph",
        &[&["<http://ex.org/absent>"]],
    ),
    (
        "SELECT ?x WHERE { :absent (:sub?)+ ?x }",
        "x+ starts with a step of x, here the zero-length pair of the constant (SPEC 18.6, ALP)",
        &[&["<http://ex.org/absent>"]],
    ),
    (
        "SELECT ?x WHERE { ?x ^(:sub*)+ :absent }",
        "x+ starts with a step of x, here the zero-length pair of the constant (SPEC 18.6, ALP)",
        &[&["<http://ex.org/absent>"]],
    ),
    (
        "SELECT ?x WHERE { VALUES ?x { :absent :p5 :x } FILTER NOT EXISTS { ?x (:sub?)+ ?y } }",
        "every substituted constant has its zero-length pair, so nothing is left",
        &[],
    ),
    (
        "SELECT ?y WHERE { GRAPH :g1 { :p1 (:sub?)+ ?y } }",
        "the zero-length pair of a constant exists in every active graph (here a named graph \
         without :p1)",
        &[&["<http://ex.org/p1>"]],
    ),
    (
        "SELECT ?y FROM NAMED :g1 WHERE { :p1 (:sub*)+ ?y }",
        "the zero-length pair of a constant exists in an empty default graph too",
        &[&["<http://ex.org/p1>"]],
    ),
    (
        "SELECT ?y FROM NAMED :g1 WHERE { :p1 (:sub?)+ ?y }",
        "the zero-length pair of a constant exists in an empty default graph too",
        &[&["<http://ex.org/p1>"]],
    ),
    (
        "SELECT ?g ?x WHERE { GRAPH ?g { VALUES ?x { 1 2 } } }",
        "GRAPH ?g evaluates its pattern once per named graph",
        &[
            &["<http://ex.org/g1>", "1"],
            &["<http://ex.org/g1>", "2"],
            &["<http://ex.org/g2>", "1"],
            &["<http://ex.org/g2>", "2"],
        ],
    ),
    (
        "SELECT ?s WHERE { ?s a :Person FILTER(isIRI(?s)) FILTER EXISTS { ?s :knows ?o MINUS { ?s :knows :carol } } }",
        "substitution: inside EXISTS ?s is a constant, so the MINUS operands share no variable \
         (docs/DECISIONS.md)",
        &[&["<http://ex.org/alice>"], &["<http://ex.org/bob>"], &["<http://ex.org/carol>"], &["<http://ex.org/eve>"]],
    ),
];

/// Expands the short forms used in `SPEC_ANSWERS` (bare integers, `^^xsd:`).
fn expand(cell: &str) -> String {
    if !cell.is_empty() && cell.bytes().all(|b| b.is_ascii_digit()) {
        return format!("\"{cell}\"^^<{XSD}integer>");
    }
    if cell == "\"07\"" {
        return format!("\"07\"^^<{XSD}integer>");
    }
    match cell.split_once("^^xsd:") {
        Some((lex, local)) => format!("{lex}^^<{XSD}{local}>"),
        None => cell.to_owned(),
    }
}

/// Queries outside the baseline subset.
const UNSUPPORTED: &[&str] = &[
    "CONSTRUCT { ?s :n ?o } WHERE { ?s :name ?o }",
    "DESCRIBE :alice",
    "SELECT ?s ?b WHERE { ?s :name ?n BIND(BNODE() AS ?b) }",
    "SELECT ?s ?r WHERE { ?s :name ?n BIND(RAND() AS ?r) }",
    "SELECT ?s WHERE { SERVICE <http://ex.org/sparql> { ?s ?p ?o } }",
    "SELECT ?g ?s WHERE { GRAPH ?g { ?s :likes ?o FILTER(?g = :g1) } }",
    "SELECT ?g ?s WHERE { GRAPH ?g { { ?s :likes ?o } UNION { BIND(1 AS ?s) } } }",
    "SELECT ?g ?x WHERE { GRAPH ?g { ?x :sub+ ?y } }",
    "SELECT ?g ?y WHERE { GRAPH ?g { :p1 (:sub*)+ ?y } }",
    "SELECT ?x WHERE { VALUES ?x { :absent UNDEF } FILTER NOT EXISTS { ?x (:sub?)+ ?y } }",
    "SELECT ?s (GROUP_CONCAT(DISTINCT ?n; SEPARATOR=\",\") AS ?all) WHERE { ?s :name ?n } GROUP BY ?s",
    "SELECT ?s (GROUP_CONCAT(?n; SEPARATOR=\"a long separator\") AS ?all) WHERE { ?s :name ?n } GROUP BY ?s",
    "SELECT (COUNT(DISTINCT *) AS ?c) (SUM(?a) AS ?t) WHERE { ?s :age ?a }",
];

const PREFIXES: &str = "PREFIX : <http://ex.org/> PREFIX xsd: <http://www.w3.org/2001/XMLSchema#> ";

#[test]
fn corpus_matches_spareval() {
    let engine = Engine::new(&[DataFile::trig(DATA)]).unwrap();
    let mut failures = Vec::new();
    for q in QUERIES {
        let text = format!("{PREFIXES}{q}");
        match check(&engine, &text, None) {
            Ok(Some(_)) => {}
            Ok(None) => failures.push(format!("{q}\n  unexpectedly unsupported")),
            Err(e) => failures.push(format!("{q}\n  {e}")),
        }
    }
    assert!(failures.is_empty(), "{} of {} queries failed:\n{}", failures.len(), QUERIES.len(), failures.join("\n"));
}

#[test]
fn spec_answers_where_spareval_deviates() {
    let engine = Engine::new(&[DataFile::trig(DATA)]).unwrap();
    for (q, why, expected) in SPEC_ANSWERS {
        let text = format!("{PREFIXES}{q}");
        let query = sq_plan::parse_query(&text, None).unwrap();
        let common::Baseline::Ran { sql, outcome } = engine.baseline(&query).unwrap() else {
            panic!("{q}: unsupported")
        };
        let common::Outcome::Solutions { rows, .. } = outcome else { panic!("{q}: not solutions") };
        let mut ours: Vec<Vec<String>> = rows
            .iter()
            .map(|r| r.iter().map(|t| t.as_ref().map_or("UNDEF".to_owned(), |t| t.to_string())).collect())
            .collect();
        let mut want: Vec<Vec<String>> = expected.iter().map(|r| r.iter().map(|c| expand(c)).collect()).collect();
        ours.sort();
        want.sort();
        assert_eq!(ours, want, "{q} ({why})\nSQL: {}", sql.sql);
    }
}

#[test]
fn queries_outside_the_subset_are_rejected() {
    let engine = Engine::new(&[DataFile::trig(DATA)]).unwrap();
    for q in UNSUPPORTED {
        let text = format!("{PREFIXES}{q}");
        let query = sq_plan::parse_query(&text, None).unwrap();
        let lowered = engine.lower(&query).unwrap();
        match sq_sqlgen::translate(&lowered) {
            Err(sq_core::Error::Unsupported(msg)) => assert!(msg.starts_with("baseline: "), "{q}: {msg}"),
            other => panic!("{q}: expected Unsupported, got {other:?}"),
        }
    }
}

/// Triples repeated across the default graph and named graphs, for `union_default_graph`.
const UNION_DATA: &str = r#"
@prefix : <http://ex.org/> .
:a :p :b . :b :p :c . :a :q 1 . :c :q 2 .
:g1 { :a :p :b . :b :p :d . :d :q 1 . :a :q 1 . }
:g2 { :a :p :b . :b :p :c . :c :p :a . :e :q 2 . }
:g3 { :c :p :a . }
"#;

/// Queries over the merge of every graph (`union_default_graph = 1`), and queries whose own
/// dataset clause takes precedence.
const UNION_QUERIES: &[&str] = &[
    "SELECT ?s ?o WHERE { ?s :p ?o }",
    "SELECT * WHERE { ?s ?p ?o }",
    "SELECT ?s WHERE { ?s :p :b }",
    "SELECT ?x ?z WHERE { ?x :p ?y . ?y :p ?z }",
    "SELECT ?s ?v WHERE { ?s :p ?o OPTIONAL { ?o :q ?v } }",
    "SELECT ?s WHERE { ?s :p ?o MINUS { ?s :q 1 } }",
    "SELECT ?s WHERE { ?s :p ?o FILTER NOT EXISTS { ?o :p ?s } }",
    "SELECT (COUNT(*) AS ?n) WHERE { ?s :p ?o }",
    "SELECT (COUNT(DISTINCT ?s) AS ?n) WHERE { ?s ?p ?o }",
    "SELECT ?o WHERE { :a :p/:p ?o }",
    "SELECT ?o WHERE { :a (:p|:q) ?o }",
    "SELECT ?o WHERE { :a :p+ ?o }",
    "SELECT ?s ?o WHERE { ?s :p* ?o }",
    "SELECT ?s WHERE { ?s :p? :a }",
    "SELECT ?s ?o WHERE { ?s !(:q) ?o }",
    "SELECT ?g ?s WHERE { GRAPH ?g { ?s :p :b } }",
    "SELECT ?s ?o FROM :g1 FROM :g2 WHERE { ?s :p ?o }",
    "SELECT ?o FROM :g1 FROM :g3 WHERE { :a :p* ?o }",
    "ASK { :d :p :a }",
];

#[test]
fn union_default_graph_matches_spareval() {
    let engine = Engine::new(&[DataFile::trig(UNION_DATA)]).unwrap();
    let opts = sq_core::plan::QueryOptions { union_default_graph: true, ..Default::default() };
    for q in UNION_QUERIES {
        let text = format!("{PREFIXES}{q}");
        let query = sq_plan::parse_query(&text, None).unwrap();
        let lowered = sq_plan::lower_query(&query, &engine.store, &opts).unwrap();
        let sql = sq_sqlgen::translate(&lowered).unwrap_or_else(|e| panic!("{q}: {e}"));
        let ours = engine.execute(&lowered, &sql).unwrap();
        let reference = engine.reference_with(&query, true).unwrap();
        if let Err(e) = common::compare(&ours, &reference, &common::Expect::of(&query)) {
            panic!("{q}: {e}\nSQL: {}", sql.sql);
        }
    }
}
