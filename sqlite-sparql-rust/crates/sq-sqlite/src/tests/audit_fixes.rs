//! Regression tests for the final audit's sq-sqlite findings: damaged or hostile dictionary
//! data is refused (`Error::Schema`) or reported, never followed, aliased or panicked on, and
//! the dictionary caches never outlive the database they were filled from.
use std::ffi::c_uint;

use oxrdf::{Literal, Term as OxTerm};
use oxrdfio::RdfFormat;
use rusqlite::{Connection, ffi};
use sq_core::store::{Dictionary, QuadStoreWriter};
use sq_core::term::Term;
use sq_core::{Error, TermId};

use super::{Db, iri, store_for};
use crate::{LoadOptions, LoadSource};

fn decode(db: &Db, id: TermId) -> sq_core::Result<Term> {
    let _r = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let mut out = Vec::new();
    db.store.decode(&[id], &mut out)?;
    Ok((*out[0]).clone())
}

fn is_schema(r: &sq_core::Result<impl std::fmt::Debug>) -> bool {
    matches!(r, Err(Error::Schema(m)) if m.starts_with("corrupt"))
}

/// A TYPED row's `dt` must name an IRI row; anything else (a TYPED row, a cycle, a language
/// literal) is refused before it is followed, so decode recursion is at most one level deep.
#[test]
fn typed_datatype_must_be_an_iri_row() {
    let db = Db::mem();
    let typed = |lex: &str, dt: &str| OxTerm::from(Literal::new_typed_literal(lex, iri(dt)));
    let a = db.store.intern(typed("x", "http://ex/dtA").as_ref()).unwrap();
    let b = db.store.intern(typed("y", "http://ex/dtB").as_ref()).unwrap();
    let lang = db.store.intern(Literal::new_language_tagged_literal("z", "en").unwrap().as_ref().into()).unwrap();
    // a -> lang (not an IRI), b -> a (a chain through TYPED rows).
    db.conn.execute("UPDATE rdf_term SET dt=?1 WHERE id=?2", (lang.raw(), a.dict_row())).unwrap();
    db.conn.execute("UPDATE rdf_term SET dt=?1 WHERE id=?2", (a.raw(), b.dict_row())).unwrap();
    for id in [a, b] {
        // A fresh store: cold caches.
        let store = store_for(&db.conn);
        let _r = store.read_scope(0).unwrap();
        store.shared().validate_epoch().unwrap();
        let mut out = Vec::new();
        let r = store.decode(&[id], &mut out);
        assert!(is_schema(&r), "{r:?}");
    }
}

// ---------------------------------------------------------------- deserialize (ABA)

/// `sqlite3_serialize` of `conn`'s main database, as an `sqlite3_malloc` buffer.
fn serialize(conn: &Connection) -> (*mut u8, i64) {
    let mut size: i64 = 0;
    // SAFETY: valid connection; "main" is NUL-terminated; flags 0 asks for a fresh copy.
    let p = unsafe { ffi::sqlite3_serialize(conn.handle(), c"main".as_ptr(), &raw mut size, 0) };
    assert!(!p.is_null());
    (p.cast::<u8>(), size)
}

/// `sqlite3_deserialize` of `image` into `conn`'s main database (which takes ownership).
fn deserialize(conn: &Connection, image: (*mut u8, i64)) {
    let flags = (ffi::SQLITE_DESERIALIZE_FREEONCLOSE | ffi::SQLITE_DESERIALIZE_RESIZEABLE) as c_uint;
    // SAFETY: `image` is an sqlite3_malloc buffer of `size` bytes from `serialize`, handed over.
    let rc = unsafe { ffi::sqlite3_deserialize(conn.handle(), c"main".as_ptr(), image.0, image.1, image.1, flags) };
    assert_eq!(rc, ffi::SQLITE_OK);
}

/// A store image whose first user IRI is `<http://ex/{prefix}0>` (same schema cookie and
/// `dict_gen` as every other image built this way).
fn image(prefix: &str) -> ((*mut u8, i64), TermId) {
    let db = Db::mem();
    let mut first = None;
    for i in 0..5 {
        let id = db.store.intern(iri(&format!("http://ex/{prefix}{i}")).as_ref().into()).unwrap();
        first.get_or_insert(id);
    }
    (serialize(&db.conn), first.unwrap())
}

fn first_iri(db: &Db, id: TermId) -> String {
    match decode(db, id).unwrap() {
        Term::Iri(s) => s.to_string(),
        t => panic!("{t:?}"),
    }
}

/// The main database replaced by `sqlite3_deserialize` (Python `Connection.deserialize`)
/// with images that agree on schema cookie and `dict_gen`: decode and lookup must answer
/// from the new image, not from caches filled from the old one.
#[test]
fn caches_follow_a_deserialized_main_database() {
    let db = Db::file();
    for i in 0..5 {
        db.store.intern(iri(&format!("http://ex/aaa{i}")).as_ref().into()).unwrap();
    }
    let id = db.store.lookup(iri("http://ex/aaa0").as_ref().into()).unwrap().unwrap();
    assert_eq!(first_iri(&db, id), "http://ex/aaa0");
    for prefix in ["bbb", "ccc", "ddd"] {
        let (img, first) = image(prefix);
        assert_eq!(first, id, "same row numbers in every image");
        deserialize(&db.conn, img);
        assert_eq!(first_iri(&db, id), format!("http://ex/{prefix}0"));
        let _r = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        assert_eq!(db.store.lookup(iri("http://ex/aaa0").as_ref().into()).unwrap(), None);
        assert_eq!(db.store.lookup(iri(&format!("http://ex/{prefix}1")).as_ref().into()).unwrap(), Some(TermId(id.raw() + 16)));
    }
}

/// Stores created before `rdf_meta.db_id` existed (no row) still flush on a new main file.
#[test]
fn caches_follow_a_deserialized_store_without_db_id() {
    let db = Db::mem();
    let a = db.store.intern(iri("http://ex/aaa0").as_ref().into()).unwrap();
    db.conn.execute_batch("DELETE FROM rdf_meta WHERE key='db_id'").unwrap();
    assert_eq!(first_iri(&db, a), "http://ex/aaa0");
    let (img, first) = image("bbb");
    assert_eq!(first, a);
    let other = Connection::open_in_memory().unwrap();
    deserialize(&other, img);
    other.execute_batch("DELETE FROM rdf_meta WHERE key='db_id'").unwrap();
    deserialize(&db.conn, serialize(&other));
    assert_eq!(first_iri(&db, a), "http://ex/bbb0");
}

/// Every store gets its own random `db_id`; `sparql_init` on an existing store keeps it.
#[test]
fn db_id_is_random_and_stable() {
    let (x, y) = (Db::mem(), Db::mem());
    let id = |db: &Db| db.query_i64("SELECT value FROM rdf_meta WHERE key='db_id'");
    assert_ne!(id(&x), id(&y));
    let before = id(&x);
    x.store.ensure_schema().unwrap();
    assert_eq!(id(&x), before);
}

// ---------------------------------------------------------------- id ranges

/// Row numbers that do not fit a `TermId` payload are refused wherever they become ids.
#[test]
fn out_of_range_dictionary_rows_are_refused() {
    let db = Db::mem();
    let a = db.store.intern(iri("http://ex/a").as_ref().into()).unwrap();
    // 2^59 + row would alias row `row` after `row << 4`.
    db.conn.execute("UPDATE rdf_term SET id=(1<<59)+id WHERE id=?1", [a.dict_row()]).unwrap();
    let fresh = |conn: &Connection| store_for(conn);
    let store = fresh(&db.conn);
    let r = store.lookup(iri("http://ex/a").as_ref().into());
    assert!(is_schema(&r), "{r:?}");
    // The next row would be 2^59 + row + 1: refused before anything is written.
    let r = store.intern(iri("http://ex/new").as_ref().into());
    assert!(is_schema(&r), "{r:?}");
    let opts = LoadOptions::new(RdfFormat::NTriples);
    let r = store.bulk_load(LoadSource::Bytes(b"<http://ex/s> <http://ex/p> <http://ex/o> .\n"), &opts);
    assert!(is_schema(&r), "{r:?}");
    // max(id) = i64::MAX: no overflow in next_dict_row.
    db.conn.execute_batch("UPDATE rdf_term SET id=9223372036854775807 WHERE id>(1<<59)").unwrap();
    let r = fresh(&db.conn).intern(iri("http://ex/new").as_ref().into());
    assert!(is_schema(&r), "{r:?}");
}

/// `next_bnode` outside `1..2^59` is refused by the writer and the loader.
#[test]
fn out_of_range_next_bnode_is_refused() {
    let db = Db::mem();
    for bad in ["(1<<59)", "9223372036854775807", "0", "-5"] {
        db.conn.execute_batch(&format!("UPDATE rdf_meta SET value={bad} WHERE key='next_bnode'")).unwrap();
        let r = db.store.fresh_bnode();
        assert!(is_schema(&r), "{bad}: {r:?}");
        let opts = LoadOptions::new(RdfFormat::NTriples);
        let r = db.store.bulk_load(LoadSource::Bytes(b"_:x <http://ex/p> <http://ex/o> .\n"), &opts);
        assert!(is_schema(&r), "{bad}: {r:?}");
    }
    // The largest counter is still allocated; the one after it is refused.
    db.conn.execute_batch("UPDATE rdf_meta SET value=(1<<59)-1 WHERE key='next_bnode'").unwrap();
    let last = db.store.fresh_bnode().unwrap();
    assert_eq!(last.bnode_counter(), (1 << 59) - 1);
    assert!(is_schema(&db.store.fresh_bnode()));
}

// ---------------------------------------------------------------- sparql_check

/// Invalid UTF-8 in `lex` or `lang` is reported per row, not an internal error.
#[test]
fn check_reports_invalid_utf8_rows() {
    let db = Db::mem();
    let a = db.store.intern(iri("http://ex/a").as_ref().into()).unwrap();
    let l = db.store.intern(Literal::new_language_tagged_literal("chat", "fr").unwrap().as_ref().into()).unwrap();
    db.conn.execute("UPDATE rdf_term SET lex=X'FFFE' WHERE id=?1", [a.dict_row()]).unwrap();
    db.conn.execute("UPDATE rdf_term SET lang=X'C328' WHERE id=?1", [l.dict_row()]).unwrap();
    let report = db.store.check().unwrap();
    assert!(report.starts_with("errors:"), "{report}");
    assert!(report.contains(&format!("row {}: lex is not valid UTF-8", a.dict_row())), "{report}");
    assert!(report.contains(&format!("row {}: lang is not valid UTF-8", l.dict_row())), "{report}");
}

