//! §2.3 bulk loader: every oxrdfio format round-trips, bulk ≡ incremental ≡ chunked loads,
//! blank nodes are fresh per load, options, atomicity on errors, and the SQLITE_LOCKED
//! fallback of bulk mode.
use std::fmt::Write as _;
use std::sync::Arc;

use oxrdf::dataset::CanonicalizationAlgorithm;
use oxrdf::{BlankNode, Dataset, GraphName, Literal, NamedNode, NamedOrBlankNode, Quad, Term as OxTerm};
use oxrdfio::{JsonLdProfileSet, RdfFormat, RdfParser, RdfSerializer};
use sq_core::store::{Dictionary, IndexOrder, QuadStore, ScanSpec};
use sq_core::term::Term;
use sq_core::{Error, TermId};

use super::{iri, Db, TempPath};
use crate::loader::{load_with, Limits};
use crate::{BulkMode, LoadOptions, LoadReport, LoadSource};

const ALL_FORMATS: [RdfFormat; 7] = [
    RdfFormat::NTriples,
    RdfFormat::NQuads,
    RdfFormat::Turtle,
    RdfFormat::TriG,
    RdfFormat::RdfXml,
    RdfFormat::N3,
    RdfFormat::JsonLd { profile: JsonLdProfileSet::empty() },
];

fn opts(format: RdfFormat, bulk: BulkMode) -> LoadOptions {
    LoadOptions { bulk, ..LoadOptions::new(format) }
}

fn load(db: &Db, data: &[u8], o: &LoadOptions) -> LoadReport {
    db.store.bulk_load(LoadSource::Bytes(data), o).unwrap()
}

/// All stored quads decoded back to an oxrdf dataset.
fn stored_dataset(db: &Db) -> Dataset {
    let mut cur = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    let mut ids = Vec::new();
    while cur.fill(&mut ids, 1024).unwrap() > 0 {}
    let mut terms: Vec<Arc<Term>> = Vec::new();
    let nonzero: Vec<TermId> = ids.iter().map(|t| if t.is_bound() { *t } else { sq_core::codec::RDF_TYPE }).collect();
    db.store.decode(&nonzero, &mut terms).unwrap();
    let mut ds = Dataset::new();
    for (q, t) in ids.chunks_exact(4).zip(terms.chunks_exact(4)) {
        let graph = if q[0].is_bound() {
            match t[0].to_oxrdf() {
                OxTerm::NamedNode(n) => GraphName::NamedNode(n),
                OxTerm::BlankNode(b) => GraphName::BlankNode(b),
                other => panic!("graph name {other}"),
            }
        } else {
            GraphName::DefaultGraph
        };
        let subject: NamedOrBlankNode = match t[1].to_oxrdf() {
            OxTerm::NamedNode(n) => n.into(),
            OxTerm::BlankNode(b) => b.into(),
            other => panic!("subject {other}"),
        };
        let OxTerm::NamedNode(predicate) = t[2].to_oxrdf() else { panic!("predicate") };
        ds.insert(&Quad::new(subject, predicate, t[3].to_oxrdf(), graph));
    }
    ds
}

fn canonical(mut ds: Dataset) -> Dataset {
    ds.canonicalize(CanonicalizationAlgorithm::Unstable);
    ds
}

/// A dataset exercising every term category (and a named graph for quad formats).
fn sample_dataset(with_graphs: bool) -> Dataset {
    let ex = |s: &str| NamedNode::new(format!("http://example.org/{s}")).unwrap();
    let xsd = |s: &str| NamedNode::new(format!("http://www.w3.org/2001/XMLSchema#{s}")).unwrap();
    let objects: Vec<OxTerm> = vec![
        ex("o").into(),
        Literal::new_simple_literal("short").into(),
        Literal::new_simple_literal("a much longer simple literal with \"quotes\"\nand a newline, é€").into(),
        Literal::new_language_tagged_literal("bonjour", "fr-BE").unwrap().into(),
        Literal::new_typed_literal("42", xsd("integer")).into(),
        Literal::new_typed_literal("007", xsd("integer")).into(),
        Literal::new_typed_literal("-1.50", xsd("decimal")).into(),
        Literal::new_typed_literal("2.5", xsd("decimal")).into(),
        Literal::new_typed_literal("1.0E3", xsd("double")).into(),
        Literal::new_typed_literal("true", xsd("boolean")).into(),
        Literal::new_typed_literal("2024-02-29", xsd("date")).into(),
        Literal::new_typed_literal("2024-02-29T12:30:00.5Z", xsd("dateTime")).into(),
        Literal::new_typed_literal("whatever", ex("custom-datatype")).into(),
        BlankNode::new("obj").unwrap().into(),
    ];
    let mut ds = Dataset::new();
    let b = BlankNode::new("subj").unwrap();
    for (i, o) in objects.iter().enumerate() {
        let p = ex(&format!("p{}", i % 4));
        ds.insert(&Quad::new(ex("s"), p.clone(), o.clone(), GraphName::DefaultGraph));
        ds.insert(&Quad::new(b.clone(), p.clone(), o.clone(), GraphName::DefaultGraph));
        if with_graphs {
            ds.insert(&Quad::new(ex("s2"), p.clone(), o.clone(), GraphName::NamedNode(ex("g1"))));
            ds.insert(&Quad::new(ex("s"), p, o.clone(), GraphName::NamedNode(ex("g2"))));
        }
    }
    ds.insert(&Quad::new(BlankNode::new("obj").unwrap(), ex("p0"), ex("o"), GraphName::DefaultGraph));
    ds
}

fn serialize(ds: &Dataset, format: RdfFormat) -> Vec<u8> {
    let mut w = RdfSerializer::from_format(format).for_writer(Vec::new());
    for q in ds.iter() {
        if format.supports_datasets() {
            w.serialize_quad(q).unwrap();
        } else if q.graph_name.is_default_graph() {
            w.serialize_triple(q).unwrap();
        }
    }
    w.finish().unwrap()
}

#[test]
fn every_format_round_trips() {
    for format in ALL_FORMATS {
        for bulk in [BulkMode::Never, BulkMode::Always] {
            let db = Db::mem();
            let data = serialize(&sample_dataset(format.supports_datasets()), format);
            // Expected = what the oxrdfio parser itself yields (serializers may normalize).
            let want: Dataset = RdfParser::from_format(format).for_slice(&data).map(Result::unwrap).collect();
            let report = load(&db, &data, &opts(format, bulk));
            assert_eq!(report.quads_read, want.len() as u64, "{format:?}");
            assert_eq!(report.quads_new, want.len() as u64, "{format:?}");
            assert_eq!(report.bulk, bulk == BulkMode::Always, "{format:?}");
            assert_eq!(canonical(stored_dataset(&db)), canonical(want), "{format:?} {bulk:?}");
            assert!(db.store.check().unwrap().starts_with("ok"), "{format:?}: {}", db.store.check().unwrap());
            // Loading the same document again adds nothing but fresh blank nodes.
            let again = load(&db, &data, &opts(format, bulk));
            assert_eq!(again.terms_new, 0, "{format:?}: no new dictionary rows on reload");
        }
    }
}

#[test]
fn blank_nodes_are_fresh_per_load_and_shared_within_one() {
    let db = Db::mem();
    let data = b"_:a <http://ex/p> _:b .\n_:b <http://ex/p> _:a .\n";
    let o = opts(RdfFormat::NTriples, BulkMode::Never);
    assert_eq!(load(&db, data, &o).quads_new, 2);
    assert_eq!(load(&db, data, &o).quads_new, 2, "new blank nodes, new quads");
    let bnodes = db.query_i64("SELECT count(DISTINCT s) FROM rdf_quad");
    assert_eq!(bnodes, 4);
    assert_eq!(db.query_i64("SELECT value FROM rdf_meta WHERE key='next_bnode'"), 5);
    use sq_core::store::QuadStoreWriter;
    assert_eq!(db.store.fresh_bnode().unwrap(), TermId::bnode(5));
}

#[test]
fn base_iri_and_target_graph() {
    let db = Db::mem();
    let o = LoadOptions {
        base_iri: Some("http://base.example/dir/".into()),
        graph: Some("http://ex/target".into()),
        ..LoadOptions::new(RdfFormat::Turtle)
    };
    load(&db, b"<s> <p> <../o> .", &o);
    let ds = stored_dataset(&db);
    let q = ds.iter().next().unwrap();
    assert_eq!(q.subject.to_string(), "<http://base.example/dir/s>");
    assert_eq!(q.object.to_string(), "<http://base.example/o>");
    assert_eq!(q.graph_name.to_string(), "<http://ex/target>");
    let g = db.store.lookup(iri("http://ex/target").as_ref().into()).unwrap().unwrap();
    assert_eq!(db.store.named_graphs().unwrap(), vec![g]);
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_graph"), 1, "registered");
    // Quads with explicit graphs keep them; default-graph triples go to the target.
    let o = LoadOptions { graph: Some("http://ex/target2".into()), ..LoadOptions::new(RdfFormat::NQuads) };
    load(&db, b"<http://ex/a> <http://ex/b> <http://ex/c> <http://ex/g> .\n<http://ex/a> <http://ex/b> <http://ex/d> .\n", &o);
    assert_eq!(db.store.named_graphs().unwrap().len(), 3);
    // Invalid options.
    let bad = LoadOptions { base_iri: Some("not an iri".into()), ..LoadOptions::new(RdfFormat::Turtle) };
    assert!(matches!(db.store.bulk_load(LoadSource::Bytes(b""), &bad), Err(Error::Invalid(_))));
    let bad = LoadOptions { graph: Some("also not an iri".into()), ..LoadOptions::new(RdfFormat::Turtle) };
    assert!(matches!(db.store.bulk_load(LoadSource::Bytes(b""), &bad), Err(Error::Invalid(_))));
}

#[test]
fn parse_errors_leave_the_store_untouched() {
    let db = Db::mem();
    load(&db, b"<http://ex/a> <http://ex/b> \"kept\" .\n", &opts(RdfFormat::NTriples, BulkMode::Never));
    let before = (db.query_i64("SELECT count(*) FROM rdf_term"), db.query_i64("SELECT count(*) FROM rdf_quad"));
    let bnode = db.query_i64("SELECT value FROM rdf_meta WHERE key='next_bnode'");
    for bulk in [BulkMode::Never, BulkMode::Always] {
        let data = b"<http://ex/new> <http://ex/b> _:x .\n<http://ex/new2> <http://ex/b> \"x\" .\nthis is not n-triples\n";
        let err = db.store.bulk_load(LoadSource::Bytes(data), &opts(RdfFormat::NTriples, bulk)).unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
        let after = (db.query_i64("SELECT count(*) FROM rdf_term"), db.query_i64("SELECT count(*) FROM rdf_quad"));
        assert_eq!(after, before);
        assert_eq!(db.query_i64("SELECT value FROM rdf_meta WHERE key='next_bnode'"), bnode);
        assert_eq!(db.query_i64("SELECT count(*) FROM sqlite_schema WHERE type='index' AND name GLOB 'rdf_*'"), 9);
    }
    // The store still works after the rollbacks.
    db.shared().validate_epoch().unwrap();
    assert!(db.store.check().unwrap().starts_with("ok"));
}

/// Social-network-like N-Triples (deterministic): `persons` × ~7 triples.
pub(crate) fn social_ntriples(persons: usize) -> String {
    let mut s = String::with_capacity(persons * 400);
    for i in 0..persons {
        let p = format!("<http://ex.org/person/{i}>");
        let _ = writeln!(s, "{p} <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://ex.org/Class{}> .", i % 10);
        let _ = writeln!(s, "{p} <http://ex.org/name> \"Person number {i}\" .");
        let _ = writeln!(s, "{p} <http://ex.org/age> \"{}\"^^<http://www.w3.org/2001/XMLSchema#integer> .", i % 90);
        for k in 1..=3 {
            let _ = writeln!(s, "{p} <http://ex.org/knows> <http://ex.org/person/{}> .", (i * 7 + k * 13) % persons);
        }
        let _ = writeln!(s, "{p} <http://ex.org/city> <http://ex.org/city/{}> .", i % 1000);
        if i % 50 == 0 {
            let _ = writeln!(s, "{p} <http://ex.org/score> \"{}.25e1\"^^<http://www.w3.org/2001/XMLSchema#double> .", i);
            let _ = writeln!(s, "{p} <http://ex.org/nick> \"nick{i}\"@en .");
        }
    }
    s
}

/// Every table and every index order of two stores, compared row by row.
fn assert_same_tables(a: &Db, b: &Db) {
    for sql in [
        "SELECT id, tag, lex, dt, lang, h FROM rdf_term ORDER BY id",
        "SELECT g FROM rdf_graph ORDER BY g",
        // write_gen / stats_gen / growth_since_analyze count write scopes, analyze runs and
        // writes, not content.
        "SELECT key, value FROM rdf_meta WHERE key NOT IN ('write_gen', 'stats_gen', 'growth_since_analyze', 'db_id') ORDER BY key",
        "SELECT * FROM rdf_stat_global ORDER BY scope",
        "SELECT * FROM rdf_stat_pred ORDER BY scope, p",
        "SELECT * FROM rdf_stat_po ORDER BY scope, p, o",
        "SELECT * FROM rdf_stat_cs ORDER BY scope, cs",
    ] {
        let dump = |db: &Db| -> Vec<String> {
            let mut st = db.conn.prepare(sql).unwrap();
            let n = st.column_count();
            st.query_map([], |r| {
                Ok((0..n).map(|i| format!("{:?}", r.get_ref(i).unwrap())).collect::<Vec<_>>().join("|"))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
        };
        assert_eq!(dump(a), dump(b), "{sql}");
    }
    for order in IndexOrder::ALL {
        let scan = |db: &Db| {
            let mut cur = db.store.cursor(&ScanSpec::new(order, &[])).unwrap();
            let mut v = Vec::new();
            while cur.fill(&mut v, 4096).unwrap() > 0 {}
            v
        };
        assert_eq!(scan(a), scan(b), "{order:?}");
    }
}

#[test]
fn bulk_incremental_and_chunked_loads_build_identical_tables() {
    let data = social_ntriples(4000);
    let nt = opts(RdfFormat::NTriples, BulkMode::Always);
    let bulk = Db::mem();
    let r1 = load(&bulk, data.as_bytes(), &nt);
    assert!(r1.bulk);
    let incremental = Db::mem();
    let r2 = load(&incremental, data.as_bytes(), &opts(RdfFormat::NTriples, BulkMode::Never));
    assert!(!r2.bulk);
    assert_eq!((r1.quads_read, r1.quads_new, r1.terms_new), (r2.quads_read, r2.quads_new, r2.terms_new));
    assert_same_tables(&bulk, &incremental);
    // Tiny chunks and a tiny load-local term map: every probe / clear path, same tables.
    let chunked = Db::mem();
    let limits = Limits { chunk_quads: 97, term_map_cap: 50 };
    let r3 = load_with(&chunked.store, LoadSource::Bytes(data.as_bytes()), &nt, limits).unwrap();
    assert_eq!(r3.terms_new, r1.terms_new);
    assert!(chunked.shared().dict_counters().probes > 0);
    assert_same_tables(&bulk, &chunked);
    // Two incremental halves (no shared blank nodes) give the same tables as one load,
    // except for statistics (analyzed after the first half and again for the growth).
    let halves = Db::mem();
    let (first, second) = data.split_at(data.len() / 2 + data[data.len() / 2..].find('\n').unwrap() + 1);
    load(&halves, first.as_bytes(), &nt);
    load(&halves, second.as_bytes(), &opts(RdfFormat::NTriples, BulkMode::Never));
    halves.store.analyze().unwrap();
    bulk.store.analyze().unwrap();
    assert_same_tables(&bulk, &halves);
    for db in [&bulk, &incremental, &chunked, &halves] {
        assert!(db.store.check().unwrap().starts_with("ok"), "{}", db.store.check().unwrap());
    }
}

#[test]
fn file_source_and_reports() {
    let db = Db::mem();
    let path = TempPath::new("load");
    let data = "<http://ex/a> <http://ex/p> \"one\" .\n<http://ex/a> <http://ex/p> \"one\" .\n<http://ex/a> <http://ex/p> \"two words long\" .\n";
    std::fs::write(&path.0, data).unwrap();
    let r = db.store.bulk_load(LoadSource::File(&path.0), &opts(RdfFormat::NTriples, BulkMode::Auto)).unwrap();
    assert_eq!((r.quads_read, r.quads_new, r.terms_new, r.bulk), (3, 2, 3, false));
    assert!(db.store.stats().is_some(), "first load analyzes");
    let missing = std::path::Path::new("/nonexistent/file.nt");
    assert!(db.store.bulk_load(LoadSource::File(missing), &opts(RdfFormat::NTriples, BulkMode::Auto)).is_err());
}

#[test]
fn auto_bulk_mode_threshold() {
    use crate::loader::use_bulk;
    let db = Db::mem();
    assert!(!use_bulk(db.shared(), BulkMode::Auto, 99_999).unwrap());
    assert!(use_bulk(db.shared(), BulkMode::Auto, 100_000).unwrap(), "empty store");
    assert!(use_bulk(db.shared(), BulkMode::Always, 1).unwrap());
    assert!(!use_bulk(db.shared(), BulkMode::Never, 10_000_000).unwrap());
    // 400K existing quads: 100K is not > 0.3x, 130K is.
    db.conn
        .execute_batch("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i < 400000) INSERT INTO rdf_quad SELECT 0, i*16+1, 17, 6 FROM n;")
        .unwrap();
    assert!(!use_bulk(db.shared(), BulkMode::Auto, 100_000).unwrap());
    assert!(use_bulk(db.shared(), BulkMode::Auto, 130_000).unwrap());
}

/// DROP INDEX is refused (SQLITE_LOCKED) while another statement of the connection reads;
/// bulk mode then falls back to incremental inserts.
#[test]
fn bulk_mode_falls_back_while_another_statement_reads() {
    let db = Db::mem();
    db.conn.execute_batch("CREATE TABLE other(x); INSERT INTO other VALUES (1), (2);").unwrap();
    let data = social_ntriples(50);
    let mut st = db.conn.prepare("SELECT x FROM other").unwrap();
    let mut rows = st.query([]).unwrap();
    assert!(rows.next().unwrap().is_some());
    let r = load(&db, data.as_bytes(), &opts(RdfFormat::NTriples, BulkMode::Always));
    assert!(!r.bulk, "fell back to incremental inserts");
    drop(rows);
    drop(st);
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad") as u64, r.quads_new);
    assert!(db.store.check().unwrap().starts_with("ok"));
}

/// Inputs of 1 MB or more are parsed on a background thread: same results, errors at the
/// end of the stream still roll everything back, and file sources work the same way.
#[test]
fn pipelined_parsing_matches_and_rolls_back() {
    let data = social_ntriples(3000);
    assert!(data.len() > 1 << 20);
    // Reference: the same triples in pieces below 1 MB (parsed on the calling thread).
    let small_chunks = Db::mem();
    let lines: Vec<&str> = data.lines().collect();
    let o = opts(RdfFormat::NTriples, BulkMode::Never);
    for part in lines.chunks(lines.len() / 4 + 1) {
        let text = part.join("\n");
        assert!(text.len() < 1 << 20);
        load(&small_chunks, text.as_bytes(), &o);
    }
    let piped = Db::mem();
    let path = TempPath::new("piped");
    std::fs::write(&path.0, &data).unwrap();
    let r = piped.store.bulk_load(LoadSource::File(&path.0), &o).unwrap();
    assert_eq!(r.quads_new as i64, small_chunks.query_i64("SELECT count(*) FROM rdf_quad"));
    assert_eq!(canonical(stored_dataset(&piped)), canonical(stored_dataset(&small_chunks)));
    // A syntax error after more than 1 MB of valid data.
    let before = piped.query_i64("SELECT count(*) FROM rdf_term");
    let mut broken = social_ntriples(3500).replace("person/", "other/");
    broken.push_str("<http://ex/x> <http://ex/y> oops .\n");
    let err = piped.store.bulk_load(LoadSource::Bytes(broken.as_bytes()), &o).unwrap_err();
    assert!(matches!(err, Error::Parse(_)), "{err}");
    assert_eq!(piped.query_i64("SELECT count(*) FROM rdf_term"), before);
    assert!(piped.store.check().unwrap().starts_with("ok"));
}
