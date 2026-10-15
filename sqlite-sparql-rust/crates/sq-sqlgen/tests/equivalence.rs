//! Baseline SQL ≡ spareval on hand-written data and queries.
mod common;

use common::{DataFile, Engine, check};

const DATA: &str = r#"
@prefix : <http://ex.org/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
:a :name "Alice" ; :age 30 ; :knows :b , :c .
:b :name "Bob" ; :age 25 ; :knows :c .
:c :name "Carol"@en ; :age "41"^^xsd:integer .
"#;

#[test]
fn smoke() {
    let e = Engine::new(&[DataFile::turtle(DATA)]).unwrap();
    for q in [
        "PREFIX : <http://ex.org/> SELECT ?x ?n WHERE { ?x :name ?n }",
        "PREFIX : <http://ex.org/> SELECT ?x WHERE { ?x :age ?a FILTER(?a > 26) }",
    ] {
        let sql = check(&e, q, None).unwrap_or_else(|err| panic!("{q}\n{err}"));
        assert!(sql.is_some(), "{q} should be supported");
    }
}
