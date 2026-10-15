//! `vacuum_terms`, `check` (each invariant is broken once on purpose) and
//! `explain_query_plan`.
use oxrdf::Literal;
use oxrdfio::RdfFormat;
use sq_core::codec;
use sq_core::store::{Dictionary, GraphTarget, QuadStoreWriter};
use sq_core::{Tag, TermId};

use super::{iri, Db};
use crate::{BulkMode, LoadOptions, LoadSource};

const DATA: &str = r#"
<http://ex/a> <http://ex/p> "a long literal that will go away" .
<http://ex/a> <http://ex/p> "1.5"^^<http://ex/only-datatype-of-a-doomed-literal> .
<http://ex/a> <http://ex/p> "2.5"^^<http://ex/shared-datatype> .
<http://ex/b> <http://ex/p> "3.5"^^<http://ex/shared-datatype> .
<http://ex/b> <http://ex/q> <http://ex/c> <http://ex/g> .
<http://ex/b> <http://ex/q> "short" .
"#;

fn loaded() -> Db {
    let db = Db::mem();
    let o = LoadOptions { bulk: BulkMode::Never, ..LoadOptions::new(RdfFormat::NQuads) };
    db.store.bulk_load(LoadSource::Bytes(DATA.as_bytes()), &o).unwrap();
    db
}

#[test]
fn vacuum_removes_exactly_the_unreferenced_rows() {
    let db = loaded();
    assert!(db.store.check().unwrap().starts_with("ok"));
    let a = db.store.lookup(iri("http://ex/a").as_ref().into()).unwrap().unwrap();
    let keep = db.store.lookup(iri("http://ex/c").as_ref().into()).unwrap().unwrap();
    assert_eq!(db.store.vacuum_terms().unwrap(), 0, "everything is referenced");
    // Remove every quad of subject a: a, its long literal, the doomed typed literal and that
    // literal's datatype become unreferenced; the shared datatype and <p> stay.
    let ws = db.store.write_scope(0).unwrap();
    db.conn.execute("DELETE FROM rdf_quad WHERE s=?1", [a.0]).unwrap();
    ws.commit().unwrap();
    let dict_gen = db.query_i64("SELECT value FROM rdf_meta WHERE key='dict_gen'");
    db.shared().validate_epoch().unwrap();
    db.store.decode(&[keep], &mut Vec::new()).unwrap();
    assert!(db.shared().decode_cache.borrow().len() > 0);
    assert_eq!(db.store.vacuum_terms().unwrap(), 5);
    assert_eq!(db.query_i64("SELECT value FROM rdf_meta WHERE key='dict_gen'"), dict_gen + 1);
    assert_eq!(db.shared().decode_cache.borrow().len(), 0, "own vacuum flushes");
    for gone in [iri("http://ex/a").into(), iri("http://ex/only-datatype-of-a-doomed-literal").into()] {
        let gone: oxrdf::Term = gone;
        assert_eq!(db.store.lookup(gone.as_ref()).unwrap(), None);
    }
    let lit = Literal::new_simple_literal("a long literal that will go away");
    assert_eq!(db.store.lookup(lit.as_ref().into()).unwrap(), None);
    assert!(db.store.lookup(iri("http://ex/shared-datatype").as_ref().into()).unwrap().is_some());
    assert!(db.store.lookup(iri("http://ex/g").as_ref().into()).unwrap().is_some(), "graph names are references");
    assert!(db.store.check().unwrap().starts_with("ok"), "{}", db.store.check().unwrap());
    // Registered-but-empty graphs keep their name.
    let g = db.store.lookup(iri("http://ex/g").as_ref().into()).unwrap().unwrap();
    db.store.clear(GraphTarget::Named(g)).unwrap();
    db.store.vacuum_terms().unwrap();
    assert!(db.store.lookup(iri("http://ex/g").as_ref().into()).unwrap().is_some());
    // Rows are reused after a vacuum, and caches never serve the old terms.
    db.shared().validate_epoch().unwrap();
    let fresh = db.store.intern(iri("http://ex/brand-new").as_ref().into()).unwrap();
    db.shared().validate_epoch().unwrap();
    let mut out = Vec::new();
    db.store.decode(&[fresh], &mut out).unwrap();
    assert_eq!(out[0].to_oxrdf(), iri("http://ex/brand-new").into());
}

#[test]
fn vacuum_rolled_back_keeps_caches_correct() {
    let db = loaded();
    let a = db.store.lookup(iri("http://ex/a").as_ref().into()).unwrap().unwrap();
    db.shared().validate_epoch().unwrap();
    db.conn.execute_batch("BEGIN").unwrap();
    db.conn.execute("DELETE FROM rdf_quad WHERE s=?1", [a.0]).unwrap();
    assert_eq!(db.store.vacuum_terms().unwrap(), 5);
    let reused = db.store.intern(iri("http://ex/reuses-a-row").as_ref().into()).unwrap();
    db.shared().validate_epoch().unwrap();
    db.store.decode(&[reused], &mut Vec::new()).unwrap();
    db.conn.execute_batch("ROLLBACK").unwrap();
    db.shared().validate_epoch().unwrap();
    let mut out = Vec::new();
    db.store.decode(&[a], &mut out).unwrap();
    assert_eq!(out[0].to_oxrdf(), iri("http://ex/a").into());
    assert_eq!(db.store.lookup(iri("http://ex/reuses-a-row").as_ref().into()).unwrap(), None);
}

fn problems(db: &Db) -> String {
    let report = db.store.check().unwrap();
    assert!(report.starts_with("errors:"), "expected problems, got {report}");
    report
}

#[test]
fn check_detects_each_broken_invariant() {
    fn fresh_id(db: &Db) -> i64 {
        db.query_i64("SELECT max(id) FROM rdf_term") + 1
    }
    type Damage = Box<dyn Fn(&Db)>;
    let cases: Vec<(&str, Damage)> = vec![
        ("inline-eligible", Box::new(|db: &Db| {
            let (lex, dt) = ("5", codec::XSD_INTEGER);
            let h = codec::term_hash(Tag::Typed, lex, codec::XSD_INTEGER_IRI);
            db.conn.execute("INSERT INTO rdf_term VALUES (?1,5,?2,?3,NULL,?4)", rusqlite::params![fresh_id(db), lex, dt.0, h]).unwrap();
        })),
        ("same term", Box::new(|db: &Db| {
            let h = codec::term_hash(Tag::Iri, "http://ex/a", "");
            db.conn.execute("INSERT INTO rdf_term VALUES (?1,1,'http://ex/a',NULL,NULL,?2)", rusqlite::params![fresh_id(db), h]).unwrap();
        })),
        ("hash", Box::new(|db: &Db| {
            db.conn.execute("UPDATE rdf_term SET h=h+1 WHERE lex='http://ex/b'", []).unwrap();
        })),
        ("missing dictionary row", Box::new(|db: &Db| {
            let dangling = TermId::from_dict(fresh_id(db) + 10, Tag::Iri);
            db.conn.execute("INSERT INTO rdf_quad VALUES (0,?1,?1,?1)", [dangling.0]).unwrap();
        })),
        ("different tag", Box::new(|db: &Db| {
            let row = db.query_i64("SELECT id FROM rdf_term WHERE lex='http://ex/b'");
            let wrong = TermId::from_dict(row, Tag::Str);
            db.conn.execute("INSERT INTO rdf_quad VALUES (0,?1,?2,?1)", [wrong.0, codec::RDF_TYPE.0]).unwrap();
        })),
        ("not in rdf_graph", Box::new(|db: &Db| {
            db.conn.execute("DELETE FROM rdf_graph", []).unwrap();
        })),
        ("not storable", Box::new(|db: &Db| {
            db.conn.execute("INSERT INTO rdf_quad VALUES (0,?1,?1,?2)", [codec::RDF_TYPE.0, TermId::temp(3, Tag::Iri).0]).unwrap();
        })),
        ("never allocated", Box::new(|db: &Db| {
            db.conn.execute("INSERT INTO rdf_quad VALUES (0,?1,?2,?2)", [TermId::bnode(999).0, codec::RDF_TYPE.0]).unwrap();
        })),
        ("language tag", Box::new(|db: &Db| {
            let h = codec::term_hash(Tag::Lang, "x", "EN");
            db.conn.execute("INSERT INTO rdf_term VALUES (?1,4,'x',NULL,'EN',?2)", rusqlite::params![fresh_id(db), h]).unwrap();
        })),
        ("dangling datatype", Box::new(|db: &Db| {
            db.conn.execute("INSERT INTO rdf_term VALUES (?1,5,'x',NULL,NULL,0)", [fresh_id(db)]).unwrap();
        })),
    ];
    for (needle, damage) in cases {
        let db = loaded();
        damage(&db);
        let report = problems(&db);
        assert!(report.contains(needle), "{needle}: {report}");
    }
}

#[test]
fn explain_query_plan_renders_the_tree() {
    let db = Db::mem();
    let plan = db
        .store
        .explain_query_plan("SELECT count(*) FROM (SELECT 1 FROM main.rdf_quad INDEXED BY rdf_gpos WHERE g=?1 AND p=?2 LIMIT ?3)")
        .unwrap();
    let lines: Vec<&str> = plan.lines().collect();
    assert_eq!(lines.len(), 3, "{plan}");
    assert!(lines[0].starts_with("CO-ROUTINE"));
    assert!(lines[1].starts_with("  SEARCH main.rdf_quad USING COVERING INDEX rdf_gpos (g=? AND p=?)"));
    assert!(db.store.explain_query_plan("SELECT * FROM nowhere").is_err());
}

/// `RawDb::exec` runs multi-statement text with comments, and rejects text SQLite cannot
/// parse completely (an embedded NUL ends SQLite's parse) instead of looping on it.
#[test]
fn raw_exec_runs_scripts_and_rejects_nul_bytes() {
    let db = Db::mem();
    let raw = db.shared().db();
    raw.exec("CREATE TABLE x(a); -- comment\nINSERT INTO x VALUES (1); /* trailing */ ;\n").unwrap();
    assert_eq!(db.query_i64("SELECT count(*) FROM x"), 1);
    for bad in ["SELECT 1;\0", "\0", "INSERT INTO x VALUES (2);\0INSERT INTO x VALUES (3)"] {
        assert!(matches!(raw.exec(bad), Err(sq_core::Error::Invalid(_))), "{bad:?}");
    }
    assert_eq!(db.query_i64("SELECT count(*) FROM x"), 1, "nothing of a rejected script ran");
}
