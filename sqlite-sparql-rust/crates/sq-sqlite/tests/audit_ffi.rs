//! Audit evidence (sq-sqlite FFI / lifetimes audit). Not part of the regular suite's design:
//! each test documents one finding and fails while the finding stands.
use oxrdf::{Literal, NamedNode, Term as OxTerm};
use rusqlite::Connection;
use sq_core::store::{Dictionary, QuadStoreWriter};
use sq_sqlite::{RawDb, SqliteStore, StorageConfig, StorageShared};

fn store_for(conn: &Connection) -> SqliteStore {
    // SAFETY: `conn` outlives the store in this test.
    let db = unsafe { RawDb::from_ptr(conn.handle()) };
    let shared = StorageShared::new(db, StorageConfig::default());
    shared.arm_pool();
    SqliteStore::new(shared)
}

/// Two TYPED dictionary rows whose `dt` columns point at each other (a damaged or hostile
/// database file, or plain `UPDATE rdf_term ...`). `fetch_terms` recurses into the missing
/// datatype ids before checking that a datatype is an IRI, so the recursion never ends and
/// the host process dies of a stack overflow instead of getting an `Error::Internal`.
fn dt_cycle_child() {
    let conn = Connection::open_in_memory().unwrap();
    let (a, b) = {
        let store = store_for(&conn);
        store.ensure_schema().unwrap();
        let lit = |lex: &str, dt: &str| OxTerm::from(Literal::new_typed_literal(lex, NamedNode::new(dt).unwrap()));
        let a = store.intern(lit("x", "http://ex/dtA").as_ref()).unwrap();
        let b = store.intern(lit("y", "http://ex/dtB").as_ref()).unwrap();
        (a, b)
    };
    conn.execute("UPDATE rdf_term SET dt=?1 WHERE id=?2", (b.raw(), a.dict_row())).unwrap();
    conn.execute("UPDATE rdf_term SET dt=?1 WHERE id=?2", (a.raw(), b.dict_row())).unwrap();
    // Fresh store: cold caches.
    let store = store_for(&conn);
    let _r = store.read_scope(0).unwrap();
    store.shared().validate_epoch().unwrap();
    let mut out = Vec::new();
    let r = store.decode(&[a], &mut out);
    println!("AUDIT decode returned {r:?}");
    assert!(r.is_err());
}

#[test]
fn audit_typed_dt_cycle_decode_does_not_crash() {
    if std::env::var_os("AUDIT_DT_CYCLE_CHILD").is_some() {
        dt_cycle_child();
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["audit_typed_dt_cycle_decode_does_not_crash", "--exact", "--nocapture", "--test-threads=1"])
        .env("AUDIT_DT_CYCLE_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "child process died: {:?}\nstdout: {}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
