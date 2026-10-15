//! Dictionary: lookup / intern / decode semantics, both decode paths, cache admission (§4.4),
//! the rollback scenario, `dict_gen` / schema invalidation and negative lookups.
use std::sync::Arc;

use oxrdf::{BlankNode, Literal, NamedNode, Term as OxTerm};
use sq_core::codec::{self, FIRST_USER_ROW};
use sq_core::store::{Dictionary, QuadStoreWriter};
use sq_core::term::Term;
use sq_core::{Error, Tag, TermId};

use super::{iri, Db};
use crate::{BulkMode, LoadOptions, LoadSource, SchemaState};

fn sample_terms() -> Vec<OxTerm> {
    let custom = iri("http://ex/dt#money");
    vec![
        iri("http://ex/a").into(),
        Literal::new_simple_literal("a long simple literal").into(),
        Literal::new_language_tagged_literal("chat", "FR-be").unwrap().into(),
        Literal::new_typed_literal("1.5e0", iri("http://www.w3.org/2001/XMLSchema#double")).into(),
        Literal::new_typed_literal("12.50", custom.clone()).into(),
        Literal::new_typed_literal("01", iri(codec::XSD_INTEGER_IRI)).into(),
        Literal::new_typed_literal("12.50", iri(codec::XSD_DECIMAL_IRI)).into(),
        custom.into(),
    ]
}

fn decode_all(db: &Db, ids: &[TermId]) -> Vec<Arc<Term>> {
    let mut out = Vec::new();
    db.store.decode(ids, &mut out).unwrap();
    assert_eq!(out.len(), ids.len());
    out
}

#[test]
fn intern_lookup_decode_round_trip() {
    let db = Db::mem();
    let terms = sample_terms();
    let ids: Vec<TermId> = terms.iter().map(|t| db.store.intern(t.as_ref()).unwrap()).collect();
    let tags: Vec<Tag> = ids.iter().map(|i| i.tag()).collect();
    assert_eq!(tags, [Tag::Iri, Tag::Str, Tag::Lang, Tag::Typed, Tag::Typed, Tag::Typed, Tag::Typed, Tag::Iri]);
    assert!(ids.iter().all(|i| i.dict_row() >= FIRST_USER_ROW));
    // The custom datatype IRI was interned before its literal and is the dt of that row.
    assert!(ids[7].dict_row() < ids[4].dict_row());
    assert_eq!(db.query_i64(&format!("SELECT dt FROM rdf_term WHERE id={}", ids[4].dict_row())), ids[7].raw());
    // Language tags are stored lower-case.
    let lang: String = db.conn.query_row(&format!("SELECT lang FROM rdf_term WHERE id={}", ids[2].dict_row()), [], |r| r.get(0)).unwrap();
    assert_eq!(lang, "fr-be");
    // Idempotent intern, lookup finds every term, decode round-trips (twice: cold and cached).
    for (t, id) in terms.iter().zip(&ids) {
        assert_eq!(db.store.intern(t.as_ref()).unwrap(), *id);
        assert_eq!(db.store.lookup(t.as_ref()).unwrap(), Some(*id));
    }
    let _r = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    for _ in 0..2 {
        let back = decode_all(&db, &ids);
        for (t, b) in terms.iter().zip(&back) {
            assert_eq!(&b.to_oxrdf(), t);
        }
    }
    assert_eq!(db.store.decode_one(ids[0]).unwrap().to_oxrdf(), terms[0]);
}

#[test]
fn inline_wellknown_and_bnodes_need_no_rows() {
    let db = Db::mem();
    let rows = db.query_i64("SELECT count(*) FROM rdf_term");
    let five = Literal::new_typed_literal("5", iri(codec::XSD_INTEGER_IRI));
    assert_eq!(db.store.intern(five.as_ref().into()).unwrap(), TermId::from_int(5).unwrap());
    let short = Literal::new_simple_literal("abc");
    assert_eq!(db.store.lookup(short.as_ref().into()).unwrap(), codec::encode_short_str("abc"));
    let ty = iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    assert_eq!(db.store.lookup(ty.as_ref().into()).unwrap(), Some(codec::RDF_TYPE));
    assert_eq!(db.store.intern(ty.as_ref().into()).unwrap(), codec::RDF_TYPE);
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_term"), rows, "no rows for inline / well-known terms");
    // Blank nodes: counter ids, labels b<hex>, never interned.
    let b1 = db.store.fresh_bnode().unwrap();
    let b2 = db.store.fresh_bnode().unwrap();
    assert_eq!((b1.tag(), b2.bnode_counter()), (Tag::BNode, b1.bnode_counter() + 1));
    let label = codec::bnode_label(b2);
    assert_eq!(db.store.lookup(BlankNode::new(&label).unwrap().as_ref().into()).unwrap(), Some(b2));
    assert!(matches!(db.store.intern(BlankNode::new("x").unwrap().as_ref().into()), Err(Error::Invalid(_))));
    assert_eq!(db.query_i64("SELECT value FROM rdf_meta WHERE key='next_bnode'"), b2.bnode_counter() + 1);
    let mut out = Vec::new();
    db.store.decode(&[b1, TermId::from_bool(true), TermId::from_int(-3).unwrap()], &mut out).unwrap();
    assert_eq!(*out[0], Term::BNode(codec::bnode_label(b1).into()));
    assert_eq!(out[2].to_oxrdf(), Literal::new_typed_literal("-3", iri(codec::XSD_INTEGER_IRI)).into());
    // Non-storable / unknown ids are errors.
    for bad in [TermId::UNBOUND, TermId::temp(1, Tag::Iri), TermId::ERROR, TermId::from_dict(999_999, Tag::Iri)] {
        let mut out = vec![Arc::new(Term::Iri("kept".into()))];
        assert!(db.store.decode(&[codec::RDF_TYPE, b1, bad], &mut out).is_err(), "{bad:?}");
        assert_eq!(out.len(), 1, "{bad:?}: output untouched on error");
    }
    // Unknown terms: None (and the typed literal of an unknown datatype short-circuits).
    assert_eq!(db.store.lookup(iri("http://ex/zzz").as_ref().into()).unwrap(), None);
    let odd = Literal::new_typed_literal("x", iri("http://ex/unknown-dt"));
    assert_eq!(db.store.lookup(odd.as_ref().into()).unwrap(), None);
}

fn make_terms(db: &Db, n: usize) -> (Vec<OxTerm>, Vec<TermId>) {
    let ws = db.store.write_scope(0).unwrap();
    let mut terms = Vec::new();
    for i in 0..n {
        let t: OxTerm = match i % 4 {
            0 => iri(&format!("http://ex/r{i}")).into(),
            1 => Literal::new_simple_literal(format!("string number {i}")).into(),
            2 => Literal::new_language_tagged_literal(format!("mot {i}"), "fr").unwrap().into(),
            _ => Literal::new_typed_literal(format!("{i}.0e0"), iri("http://www.w3.org/2001/XMLSchema#double")).into(),
        };
        terms.push(t);
    }
    let ids = terms.iter().map(|t| db.store.intern(t.as_ref()).unwrap()).collect();
    ws.commit().unwrap();
    (terms, ids)
}

#[test]
fn decode_batches_point_and_merge_paths() {
    let db = Db::mem();
    let (terms, ids) = make_terms(&db, 3000);
    let _r = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    // Small batch with duplicates: point lookups.
    let small: Vec<TermId> = vec![ids[5], ids[3], ids[5], ids[2999], TermId::from_int(1).unwrap(), ids[3]];
    let out = decode_all(&db, &small);
    assert_eq!(out[0].to_oxrdf(), terms[5]);
    assert_eq!(out[2], out[0]);
    assert_eq!(out[3].to_oxrdf(), terms[2999]);
    assert_eq!(db.shared().dict_counters().decode_merge_scans, 0);
    // Flush and decode a large, sparse, shuffled batch: one merge scan (with re-seeks over gaps).
    db.shared().flush_dictionary_caches().unwrap();
    let mut big: Vec<usize> = (0..3000).filter(|i| i % 3 != 1).collect();
    big.reverse();
    big.extend(2900..2950);
    let want: Vec<TermId> = big.iter().map(|&i| ids[i]).collect();
    let mut distinct = want.clone();
    distinct.sort();
    distinct.dedup();
    let before = db.shared().dict_counters();
    let out = decode_all(&db, &want);
    for (&i, t) in big.iter().zip(&out) {
        assert_eq!(t.to_oxrdf(), terms[i], "term {i}");
    }
    let after = db.shared().dict_counters();
    assert_eq!(after.decode_merge_scans, before.decode_merge_scans + 1);
    // Every distinct miss once, plus the (flushed) xsd:double datatype row once.
    assert_eq!(after.decode_db_rows - before.decode_db_rows, distinct.len() as u64 + 1);
    // Now everything is cached: no further reads.
    let out2 = decode_all(&db, &want);
    assert_eq!(out, out2);
    assert_eq!(db.shared().dict_counters().decode_db_rows, after.decode_db_rows);
}

#[test]
fn uncommitted_rows_are_never_cached() {
    let db = Db::mem();
    db.shared().validate_epoch().unwrap();
    let entries = || db.shared().decode_cache.borrow().len();
    let t = iri("http://ex/new");
    let ws = db.store.write_scope(0).unwrap();
    let id = db.store.intern(t.as_ref().into()).unwrap();
    db.shared().validate_epoch().unwrap(); // inside the write transaction: no refresh
    assert!(id.dict_row() > db.shared().committed_max_row());
    assert_eq!(decode_all(&db, &[id])[0].to_oxrdf(), t.clone().into());
    assert_eq!(db.store.lookup(t.as_ref().into()).unwrap(), Some(id));
    assert_eq!(entries(), 0, "row of an open transaction admitted");
    assert_eq!(db.shared().lookup_cache.borrow().len(), 0);
    ws.commit().unwrap();
    db.shared().validate_epoch().unwrap();
    assert_eq!(db.shared().committed_max_row(), id.dict_row());
    decode_all(&db, &[id]);
    assert_eq!(entries(), 1);
    db.store.lookup(t.as_ref().into()).unwrap();
    assert_eq!(db.shared().lookup_cache.borrow().len(), 1);
}

/// §12.1 rollback / cache-admission scenario: BEGIN; load; query; ROLLBACK; load other data;
/// query -> no stale terms (the rolled-back rows reuse the same row numbers).
#[test]
fn rollback_then_reload_sees_no_stale_terms() {
    let db = Db::file();
    let opts = LoadOptions { bulk: BulkMode::Never, ..LoadOptions::new(oxrdfio::RdfFormat::NTriples) };
    // Warm the caches with committed data first.
    db.store.bulk_load(LoadSource::Bytes(b"<http://ex/s0> <http://ex/p> \"committed literal\" .\n"), &opts).unwrap();
    db.shared().validate_epoch().unwrap();
    let committed = db.store.lookup(Literal::new_simple_literal("committed literal").as_ref().into()).unwrap().unwrap();
    decode_all(&db, &[committed]);

    db.conn.execute_batch("BEGIN").unwrap();
    let a = b"<http://ex/a1> <http://ex/p> \"rolled back literal\" .\n";
    db.store.bulk_load(LoadSource::Bytes(a), &opts).unwrap();
    db.shared().validate_epoch().unwrap();
    let a_lit = Literal::new_simple_literal("rolled back literal");
    let a_id = db.store.lookup(a_lit.as_ref().into()).unwrap().unwrap();
    let a_iri = db.store.lookup(iri("http://ex/a1").as_ref().into()).unwrap().unwrap();
    assert_eq!(decode_all(&db, &[a_id])[0].to_oxrdf(), a_lit.clone().into());
    db.conn.execute_batch("ROLLBACK").unwrap();

    let b = b"<http://ex/b1> <http://ex/p> \"replacement literal\" .\n";
    db.store.bulk_load(LoadSource::Bytes(b), &opts).unwrap();
    let _r = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let b_lit = Literal::new_simple_literal("replacement literal");
    let b_id = db.store.lookup(b_lit.as_ref().into()).unwrap().unwrap();
    let b_iri = db.store.lookup(iri("http://ex/b1").as_ref().into()).unwrap().unwrap();
    assert_eq!((b_iri, b_id), (a_iri, a_id), "rows are reused after the rollback");
    let back = decode_all(&db, &[a_iri, a_id, committed]);
    assert_eq!(back[0].to_oxrdf(), iri("http://ex/b1").into());
    assert_eq!(back[1].to_oxrdf(), b_lit.into());
    assert_eq!(back[2].to_oxrdf(), Literal::new_simple_literal("committed literal").into());
    assert_eq!(db.store.lookup(a_lit.as_ref().into()).unwrap(), None);
    assert_eq!(db.store.lookup(iri("http://ex/a1").as_ref().into()).unwrap(), None);
    assert_eq!(db.store.check().unwrap().split(':').next(), Some("ok"));
}

#[test]
fn caches_survive_writes_but_not_dict_gen_or_schema_changes() {
    let db = Db::file();
    let other = db.reopen();
    let (terms, ids) = make_terms(&db, 40);
    db.shared().validate_epoch().unwrap();
    decode_all(&db, &ids);
    // 40 terms plus the xsd:double datatype row of the TYPED ones.
    let cached = db.shared().decode_cache.borrow().len();
    assert_eq!(cached, 41);
    // Writes by another connection (dictionary and unrelated tables) keep the cache.
    other.store.intern(iri("http://ex/from-other").as_ref().into()).unwrap();
    other.conn.execute_batch("CREATE TABLE IF NOT EXISTS unrelated(x); INSERT INTO unrelated VALUES (1);").unwrap();
    db.conn.execute_batch("INSERT INTO unrelated VALUES (2)").unwrap();
    // (CREATE TABLE changed the schema cookie: that alone flushes, as specified.)
    db.shared().validate_epoch().unwrap();
    decode_all(&db, &ids);
    other.conn.execute_batch("INSERT INTO unrelated VALUES (3)").unwrap();
    db.shared().validate_epoch().unwrap();
    assert_eq!(db.shared().decode_cache.borrow().len(), 41);
    // A dict_gen bump (vacuum by another connection) flushes.
    other.conn.execute_batch("UPDATE rdf_meta SET value=value+1 WHERE key='dict_gen'").unwrap();
    db.shared().validate_epoch().unwrap();
    assert_eq!(db.shared().decode_cache.borrow().len(), 0);
    decode_all(&db, &ids);
    // Dropping and re-creating the store (dict_gen back to 0!) is caught by the schema cookie.
    other
        .conn
        .execute_batch(
            "DROP TABLE rdf_meta; DROP TABLE rdf_term; DROP TABLE rdf_quad; DROP TABLE rdf_graph; DROP TABLE rdf_stat_global;
             DROP TABLE rdf_stat_pred; DROP TABLE rdf_stat_po; DROP TABLE rdf_stat_cs;",
        )
        .unwrap();
    assert_eq!(db.shared().validate_epoch().unwrap(), SchemaState::Absent);
    other.store.ensure_schema().unwrap();
    let new_id = other.store.intern(iri("http://ex/totally-different").as_ref().into()).unwrap();
    assert_eq!(new_id, ids[0], "row numbers restart");
    db.shared().validate_epoch().unwrap();
    assert_eq!(decode_all(&db, &[new_id])[0].to_oxrdf(), iri("http://ex/totally-different").into());
    assert_ne!(decode_all(&db, &[new_id])[0].to_oxrdf(), terms[0]);
}

#[test]
fn negative_lookups_are_not_cached() {
    let db = Db::file();
    let other = db.reopen();
    db.shared().validate_epoch().unwrap();
    let t = iri("http://ex/later");
    assert_eq!(db.store.lookup(t.as_ref().into()).unwrap(), None);
    let id = other.store.intern(t.as_ref().into()).unwrap();
    db.shared().validate_epoch().unwrap();
    assert_eq!(db.store.lookup(t.as_ref().into()).unwrap(), Some(id));
}

#[test]
fn hash_collisions_are_resolved_by_exact_compare() {
    let db = Db::mem();
    let t = Literal::new_simple_literal("the real term, long enough");
    // Plant a different term under the real term's hash at a lower row (as if xxh3 collided),
    // so every probe meets the impostor first.
    let h = codec::term_hash(Tag::Str, "the real term, long enough", "");
    db.conn
        .execute(
            "INSERT INTO rdf_term(id,tag,lex,dt,lang,h) VALUES (?1,3,'an impostor sharing the hash',NULL,NULL,?2)",
            rusqlite::params![FIRST_USER_ROW, h],
        )
        .unwrap();
    assert_eq!(db.store.lookup(t.as_ref().into()).unwrap(), None);
    let id = db.store.intern(t.as_ref().into()).unwrap();
    assert_eq!(id, TermId::from_dict(FIRST_USER_ROW + 1, Tag::Str), "intern allocated its own row");
    db.shared().flush_dictionary_caches().unwrap();
    assert_eq!(db.store.lookup(t.as_ref().into()).unwrap(), Some(id));
    assert_eq!(decode_all(&db, &[id])[0].to_oxrdf(), t.into());
}

#[test]
fn absent_and_incompatible_schemas() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let store = super::store_for(&conn);
    assert_eq!(store.shared().validate_epoch().unwrap(), SchemaState::Absent);
    // Absent: an empty dataset for readers, Error::Schema for writers.
    use sq_core::store::{IndexOrder, QuadStore, ScanSpec};
    let mut cur = store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    assert_eq!(cur.fill(&mut Vec::new(), 10).unwrap(), 0);
    assert_eq!(store.count(&ScanSpec::new(IndexOrder::Gpos, &[]), None).unwrap(), 0);
    assert!(store.named_graphs().unwrap().is_empty());
    assert!(store.stats().is_none());
    assert_eq!(store.lookup(iri("http://ex/a").as_ref().into()).unwrap(), None);
    assert!(matches!(store.intern(iri("http://ex/a").as_ref().into()), Err(Error::Schema(_))));
    assert!(matches!(store.fresh_bnode(), Err(Error::Schema(_))));
    // Init, then make the encoding incompatible.
    assert_eq!(store.ensure_schema().unwrap(), 1);
    assert_eq!(store.ensure_schema().unwrap(), 1, "idempotent");
    assert!(matches!(store.shared().validate_epoch().unwrap(), SchemaState::Ready { schema_version: 1 }));
    conn.execute_batch("UPDATE rdf_meta SET value=2 WHERE key='encoding_version'").unwrap();
    assert!(matches!(store.shared().validate_epoch().unwrap(), SchemaState::Incompatible(_)));
    assert!(matches!(store.ensure_schema(), Err(Error::Schema(_))));
    assert!(matches!(store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])), Err(Error::Schema(_))));
    assert!(matches!(store.lookup(iri("http://ex/a").as_ref().into()), Err(Error::Schema(_))));
    drop(cur);
    drop(store);
}

#[test]
fn ensure_schema_creates_the_exact_layout() {
    let db = Db::mem();
    let names: Vec<String> = db
        .conn
        .prepare("SELECT name FROM sqlite_schema WHERE name GLOB 'rdf_*' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut want = vec![
        "rdf_graph", "rdf_gosp", "rdf_gpos", "rdf_gpox", "rdf_gpso", "rdf_meta", "rdf_ospg", "rdf_posg", "rdf_psog",
        "rdf_quad", "rdf_spog", "rdf_stat_cs", "rdf_stat_global", "rdf_stat_po", "rdf_stat_pred", "rdf_term", "rdf_term_h",
    ];
    want.sort();
    assert_eq!(names, want);
    for (row, iri) in codec::WELL_KNOWN {
        let (lex, h): (String, i64) =
            db.conn.query_row("SELECT lex, h FROM rdf_term WHERE id=?1 AND tag=1", [row], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(lex, *iri);
        assert_eq!(h, codec::term_hash(Tag::Iri, iri, ""));
    }
    for (k, v) in [("schema_version", 1), ("encoding_version", 1), ("next_bnode", 1), ("dict_gen", 0), ("stats_gen", 0)] {
        assert_eq!(db.query_i64(&format!("SELECT value FROM rdf_meta WHERE key='{k}'")), v, "{k}");
    }
    // Re-running the DDL after an index was dropped restores it.
    db.conn.execute_batch("DROP INDEX rdf_gpox").unwrap();
    db.store.ensure_schema().unwrap();
    assert_eq!(db.query_i64("SELECT count(*) FROM sqlite_schema WHERE name='rdf_gpox'"), 1);
}

#[test]
fn quad_values_interop_with_memstore_ids() {
    // Ids are plain i64 in SQLite: negative inline ints sort before positive ids (signed order).
    let db = Db::mem();
    let neg = NamedNode::new("http://ex/s").unwrap();
    let s = db.store.intern(neg.as_ref().into()).unwrap();
    let p = codec::RDF_TYPE;
    let quads: Vec<[TermId; 4]> =
        [-5i64, 3, -1].iter().map(|v| [TermId::DEFAULT_GRAPH, s, p, TermId::from_int(*v).unwrap()]).collect();
    db.insert(&quads);
    let mut cur = sq_core::store::QuadStore::cursor(
        &db.store,
        &sq_core::store::ScanSpec::new(sq_core::store::IndexOrder::Gspo, &[TermId::DEFAULT_GRAPH, s, p]),
    )
    .unwrap();
    let mut out = Vec::new();
    cur.fill(&mut out, 10).unwrap();
    let vals: Vec<i64> = out.iter().map(|t| t.as_int().unwrap()).collect();
    assert_eq!(vals, [-5, -1, 3]);
}
