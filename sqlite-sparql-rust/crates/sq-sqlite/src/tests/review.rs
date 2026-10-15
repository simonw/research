//! Adversarial review tests (WP2 + WP3). Each `review_*` test pins one claim of the package
//! description: the cursor contract under interleaved operations, error-path robustness,
//! the well-known rows of an uninitialized store, statement plans with `sqlite_stat1`
//! present, and schema repair by `ensure_schema`.
use std::ops::Bound;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::time::Duration;

use sq_core::codec;
use sq_core::store::{Dictionary, IndexOrder, QuadStore, ScanSpec};
use sq_core::testing::{expected_rows, random_id, random_quads, Rng};
use sq_core::{Tag, TermId};

use super::{iri, store_for, Db};
use crate::template::{StmtKey, StmtKind};
use crate::RawDb;

fn random_bound(r: &mut Rng, rows: &[Vec<TermId>]) -> Bound<i64> {
    let v = if rows.is_empty() || r.below(4) == 0 { random_id(r, 60).0 } else { rows[r.below(rows.len() as u64) as usize][0].0 };
    match r.below(3) {
        0 => Bound::Unbounded,
        1 => Bound::Included(v),
        _ => Bound::Excluded(v),
    }
}

/// Random interleavings of short fills, forward / backward seeks of 1..=width columns (many
/// of them within the galloping distance, some from a pending row) and rebinds (also to
/// other bound operators), checked after every operation against a brute-force model. The
/// shared conformance suite only seeks after full drains, so it never reaches the
/// pending-row / gallop paths of `SqlCursor::seek`.
#[test]
fn review_random_interleaved_cursor_ops_match_model() {
    for (seed, named) in [(101u64, false), (102, true), (103, true)] {
        let quads = random_quads(seed, 1500, named);
        let db = Db::mem();
        db.insert(&quads);
        let _r = db.store.read_scope(0).unwrap();
        let mut r = Rng(seed * 7 + 1);
        for order in IndexOrder::ALL {
            for prefix_len in 0..=4u8 {
                for _ in 0..5 {
                    let pick_prefix = |r: &mut Rng| -> Vec<TermId> {
                        let q = quads[r.below(quads.len() as u64) as usize];
                        order.cols()[..prefix_len as usize].iter().map(|c| q[*c as usize]).collect()
                    };
                    let prefix = pick_prefix(&mut r);
                    let mut spec = ScanSpec::new(order, &prefix);
                    if prefix_len < 4 && r.below(2) == 0 {
                        let all = expected_rows(&quads, &spec);
                        spec.lower = random_bound(&mut r, &all);
                        spec.upper = random_bound(&mut r, &all);
                    }
                    let mut rows = expected_rows(&quads, &spec);
                    let mut pos = 0usize;
                    let mut cur = db.store.cursor(&spec).unwrap();
                    let w = cur.width();
                    let mut log: Vec<String> = Vec::new();
                    for _ in 0..80 {
                        match r.below(6) {
                            0..=2 => {
                                let k = 1 + r.below(4) as usize;
                                let mut buf = Vec::new();
                                let n = cur.fill(&mut buf, k).unwrap();
                                let end = (pos + k).min(rows.len());
                                let want: Vec<TermId> = rows[pos..end].concat();
                                log.push(format!("fill({k}) -> {n}"));
                                assert_eq!(buf, want, "{spec:?} after {log:?}");
                                assert_eq!(n, end - pos, "{spec:?} after {log:?}");
                                pos = end;
                            }
                            3 | 4 if w > 0 => {
                                let klen = 1 + r.below(w as u64) as usize;
                                let key: Vec<TermId> = if rows.is_empty() || r.below(5) == 0 {
                                    (0..klen).map(|_| random_id(&mut r, 60)).collect()
                                } else {
                                    // mostly short forward jumps (gallop range), some backward
                                    let delta = r.below(12) as i64 - 2;
                                    let j = (pos as i64 + delta).clamp(0, rows.len() as i64 - 1) as usize;
                                    let mut key = rows[j][..klen].to_vec();
                                    if r.below(3) == 0 {
                                        let last = key.len() - 1;
                                        key[last] = TermId(key[last].0 + if r.below(2) == 0 { 1 } else { -1 });
                                    }
                                    key
                                };
                                cur.seek(&key).unwrap();
                                log.push(format!("seek({key:?})"));
                                pos = rows.partition_point(|row| row[..klen] < key[..]);
                            }
                            _ => {
                                let p2 = pick_prefix(&mut r);
                                let (lo, hi) = if prefix_len < 4 && r.below(2) == 0 {
                                    let base = expected_rows(&quads, &ScanSpec::new(order, &p2));
                                    (random_bound(&mut r, &base), random_bound(&mut r, &base))
                                } else {
                                    (Bound::Unbounded, Bound::Unbounded)
                                };
                                cur.rebind(&p2, lo, hi).unwrap();
                                spec = ScanSpec::new(order, &p2).with_range(lo, hi);
                                rows = expected_rows(&quads, &spec);
                                pos = 0;
                                log.push(format!("rebind({spec:?})"));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// An error while positioning (here: `sqlite3_prepare_v3` returns SQLITE_INTERRUPT for the
/// not-yet-pooled row-value variant) must leave the cursor usable or report errors; it must
/// not leave `active` pointing at an un-leased slot so that the next `fill` panics
/// ("active statement is leased"). The same happens when the variant's index was dropped
/// (bulk-load window, user DDL) or for a rebind to other bound operators.
#[test]
fn review_fill_after_failed_seek_does_not_panic() {
    let db = Db::mem();
    let quads = random_quads(5, 300, false);
    db.insert(&quads);
    let spec = ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH]);
    let mut cur = db.store.cursor(&spec).unwrap();
    // Keep one statement of the connection running, so the interrupt flag stays set.
    let mut running = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    assert_eq!(running.fill(&mut Vec::new(), 1).unwrap(), 1);
    db.shared().db().interrupt();
    let key = [quads[10][2], quads[10][1]];
    assert!(cur.seek(&key).is_err(), "preparing the seek_cols=2 variant is interrupted");
    let outcome = catch_unwind(AssertUnwindSafe(|| cur.fill(&mut Vec::new(), 10)));
    drop(running);
    assert!(outcome.is_ok(), "SqlCursor::fill panicked after a failed seek");
}

/// Same defect through `rebind`: new bound operators re-lease the base statement; if that
/// prepare fails, `fill` must not panic.
#[test]
fn review_fill_after_failed_rebind_does_not_panic() {
    let db = Db::mem();
    let quads = random_quads(6, 300, false);
    db.insert(&quads);
    let p = quads[0][2];
    let mut cur = db.store.cursor(&ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH, p])).unwrap();
    let mut running = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    assert_eq!(running.fill(&mut Vec::new(), 1).unwrap(), 1);
    db.shared().db().interrupt();
    // Excluded/Included bounds -> StmtKey with Gt/Le: a statement that was never prepared.
    assert!(cur.rebind(&[TermId::DEFAULT_GRAPH, p], Bound::Excluded(0), Bound::Included(1 << 40)).is_err());
    let outcome = catch_unwind(AssertUnwindSafe(|| cur.fill(&mut Vec::new(), 10)));
    drop(running);
    assert!(outcome.is_ok(), "SqlCursor::fill panicked after a failed rebind");
}

/// `codec::inline_datatype` hands out the WELL_KNOWN ids (e.g. DATATYPE(1) = xsd:integer =
/// row 2) without any dictionary access, and §8.2 says queries on an absent schema see an
/// empty dataset. So on an uninitialized store the dictionary must still map the well-known
/// IRIs to their fixed ids and decode them (MemStore does). Otherwise
/// `ASK { FILTER(datatype(1) = xsd:integer) }` compares row 2 with a TEMP id -> false, and
/// projecting DATATYPE(1) fails with "no such table: main.rdf_term".
#[test]
fn review_absent_schema_still_knows_the_well_known_rows() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let store = store_for(&conn);
    assert_eq!(store.shared().validate_epoch().unwrap(), crate::SchemaState::Absent);
    let xsd_integer = iri(codec::XSD_INTEGER_IRI);
    let looked_up = store.lookup(xsd_integer.as_ref().into());
    let mut out = Vec::new();
    let decoded = store.decode(&[codec::XSD_INTEGER], &mut out).map(|()| out[0].to_oxrdf());
    assert!(
        matches!(looked_up, Ok(Some(id)) if id == codec::XSD_INTEGER)
            && matches!(&decoded, Ok(t) if *t == oxrdf::Term::from(xsd_integer.clone())),
        "absent schema: lookup(xsd:integer) = {looked_up:?} (want Some(XSD_INTEGER)), decode(XSD_INTEGER) = {decoded:?}"
    );
    drop(store);
}

/// `RawDb::exec` (public §11 API) loops forever when the text contains a NUL byte: SQLite
/// stops at the NUL, returns no statement and a tail that did not advance, and
/// `rest.trim()` does not trim '\0'.
#[test]
fn review_exec_with_nul_byte_terminates() {
    let (tx, rx) = mpsc::channel();
    // The worker owns its connection; if exec never returns, the thread is leaked (spinning)
    // until the test binary exits, and the test fails on the timeout.
    std::thread::spawn(move || {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // SAFETY: `conn` outlives `db` (both live in this closure).
        let db = unsafe { RawDb::from_ptr(conn.handle()) };
        let r = db.exec("SELECT 1;\0");
        let _ = tx.send(r.is_ok());
        drop(conn);
    });
    assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok(), "RawDb::exec(\"SELECT 1;\\0\") did not return within 5 s");
}

/// The EXPLAIN QUERY PLAN guarantee must also hold once `ANALYZE main` has written
/// `sqlite_stat1` (the benchmark runs it on every SQLite database, §12.4).
#[test]
fn review_every_statement_key_keeps_its_plan_after_analyze() {
    let db = Db::mem();
    let mut q = random_quads(77, 4000, true);
    // a few heavily skewed predicates / graphs so stat1 is far from uniform
    for i in 0..2000i64 {
        q.push([TermId::DEFAULT_GRAPH, TermId::from_dict(5000 + i, Tag::Iri), codec::RDF_TYPE, TermId::from_dict(4000, Tag::Iri)]);
    }
    db.insert(&q);
    db.conn.execute_batch("ANALYZE main").unwrap();
    assert!(db.query_i64("SELECT count(*) FROM sqlite_stat1") > 0);
    for key in StmtKey::all_reachable() {
        let sql = key.sql();
        let plan = db.store.explain_query_plan(&sql).unwrap();
        assert!(!plan.contains("TEMP B-TREE"), "{key:?}: sorter after ANALYZE\n{sql}\n{plan}");
        let line = plan.lines().find(|l| l.contains("rdf_quad") && (l.contains("SEARCH") || l.contains("SCAN"))).unwrap();
        let want_index = match key.order {
            IndexOrder::Gspo => "PRIMARY KEY".to_string(),
            o => format!("COVERING INDEX {}", o.sql_from().rsplit(' ').next().unwrap()),
        };
        assert!(line.contains(&want_index), "{key:?}: index after ANALYZE\n{sql}\n{plan}");
        if key.prefix_len > 0 && key.kind != StmtKind::Count {
            assert!(line.contains("SEARCH"), "{key:?}: prefix not used after ANALYZE\n{sql}\n{plan}");
        }
    }
}

/// `sparql_init()` is idempotent DDL (`CREATE TABLE IF NOT EXISTS …`, §2.1): with the
/// version rows intact, a missing statistics table is re-created like a missing index,
/// instead of declaring the whole store incompatible.
#[test]
fn review_ensure_schema_recreates_a_missing_table() {
    let db = Db::mem();
    db.conn.execute_batch("DROP TABLE rdf_stat_cs").unwrap();
    let r = db.store.ensure_schema();
    assert!(r.is_ok(), "ensure_schema after DROP TABLE rdf_stat_cs: {r:?}");
    assert_eq!(db.query_i64("SELECT count(*) FROM sqlite_schema WHERE name='rdf_stat_cs'"), 1);
}

/// A cursor whose positioning failed must not silently report "exhausted" either: the
/// seek's rows exist. (Companion to the panic test: after the interrupt is gone, a retried
/// seek works.)
#[test]
fn review_seek_retry_after_interrupt_returns_rows() {
    let db = Db::mem();
    let quads = random_quads(8, 300, false);
    db.insert(&quads);
    let spec = ScanSpec::new(IndexOrder::Gspo, &[TermId::DEFAULT_GRAPH]);
    let mut cur = db.store.cursor(&spec).unwrap();
    let mut running = db.store.cursor(&ScanSpec::new(IndexOrder::Gpos, &[])).unwrap();
    assert_eq!(running.fill(&mut Vec::new(), 1).unwrap(), 1);
    db.shared().db().interrupt();
    let key = [quads[50][1], quads[50][2]];
    let _ = cur.seek(&key);
    drop(running);
    cur.seek(&key).unwrap();
    let mut buf = Vec::new();
    while cur.fill(&mut buf, 64).unwrap() > 0 {}
    let want: Vec<TermId> =
        expected_rows(&quads, &spec).into_iter().filter(|r| r[..2] >= key[..]).flatten().collect();
    assert_eq!(buf, want);
}

/// The host statement that is running while our code executes inside a scalar function
/// (`SELECT sparql_update('…')`): a CTE that reads no database, so `txn_state(main)` stays
/// NONE and the scopes own the transaction, exactly as for a bare `SELECT f()`.
const HOST_STATEMENT: &str = "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM c LIMIT 10) SELECT x FROM c";

/// §8.6 WriteScope: "on error ROLLBACK TO sq_w{depth} + RELEASE". After `sqlite3_interrupt`
/// (user Ctrl-C / `conn.interrupt()` while `SELECT sparql_update(…)` runs) every statement of
/// the connection fails with SQLITE_INTERRUPT until the host statement ends — including the
/// pooled `ROLLBACK TO` / `RELEASE` run by `WriteScope::drop`, whose errors are ignored.
/// SQLite only auto-rolls back interrupted *write* statements; the interrupted read (the
/// update's WHERE probe) and the interrupted savepoint statements are read-only, so nothing
/// rolls back: the half-applied update stays in an open transaction that a later COMMIT (or
/// Python's `conn.commit()`) makes durable.
#[test]
fn review_interrupted_write_scope_still_rolls_back() {
    let db = Db::mem();
    let quads = random_quads(9, 50, false);
    let mut host = db.conn.prepare(HOST_STATEMENT).unwrap();
    {
        let mut rows = host.query([]).unwrap();
        assert!(rows.next().unwrap().is_some());
        let ws = db.store.write_scope(0).unwrap();
        assert_eq!(sq_core::store::QuadStoreWriter::insert(&db.store, &mut quads.clone()).unwrap(), quads.len() as u64);
        db.shared().db().interrupt();
        // The update's next read (WHERE evaluation) fails ...
        assert!(matches!(db.store.check_interrupt(), Err(sq_core::Error::Interrupted)));
        // ... and the error path drops the scope without commit.
        drop(ws);
    }
    drop(host);
    let open = !db.shared().db().autocommit();
    let visible = db.query_i64("SELECT count(*) FROM rdf_quad");
    if open {
        // What a caller's later COMMIT would make durable.
        db.conn.execute_batch("COMMIT").unwrap();
    }
    assert!(
        !open && visible == 0,
        "dropped WriteScope after an interrupt: transaction still open = {open}, rolled-back quads still visible = {visible}"
    );
}

/// Same for the ReadScope of a bare `SELECT f()`: the interrupted `RELEASE sq_r{d}` is
/// ignored, the connection stays inside our savepoint transaction (autocommit off) after the
/// host statement ended, and on a rollback-journal file database its SHARED lock blocks
/// every writer on other connections until the user happens to end the transaction.
#[test]
fn review_interrupted_read_scope_does_not_leak_a_transaction() {
    let db = Db::file();
    db.insert(&random_quads(10, 50, false));
    let other = db.reopen();
    other.conn.busy_timeout(Duration::ZERO).unwrap();
    let mut host = db.conn.prepare(HOST_STATEMENT).unwrap();
    {
        let mut rows = host.query([]).unwrap();
        assert!(rows.next().unwrap().is_some());
        let scope = db.store.read_scope(0).unwrap();
        assert!(scope.is_active());
        db.shared().validate_epoch().unwrap();
        db.shared().db().interrupt();
        assert!(matches!(db.store.check_interrupt(), Err(sq_core::Error::Interrupted)));
        drop(scope);
    }
    drop(host);
    let leaked = !db.shared().db().autocommit();
    let writer = other.conn.execute("INSERT INTO rdf_graph(g) VALUES (12345)", []);
    assert!(
        !leaked && writer.is_ok(),
        "after an interrupted bare call: savepoint leaked = {leaked}, other connection's write = {writer:?}"
    );
}

/// Width-0 cursors (complete prefix) through every order, including the partial orders whose
/// partial term must exclude the row (named-only order with g = 0, Gpox with a non-numeric
/// object), plus capped counts of those specs.
#[test]
fn review_width0_specs_respect_partial_indexes() {
    let db = Db::mem();
    let s = TermId::from_dict(2000, Tag::Iri);
    let p = TermId::from_dict(2001, Tag::Iri);
    let g = TermId::from_dict(2002, Tag::Iri);
    let o_iri = TermId::from_dict(2003, Tag::Iri);
    let o_typed = TermId::from_dict(2004, Tag::Typed);
    let quads = vec![[TermId::DEFAULT_GRAPH, s, p, o_iri], [TermId::DEFAULT_GRAPH, s, p, o_typed], [g, s, p, o_iri]];
    db.insert(&quads);
    for order in IndexOrder::ALL {
        for q in &quads {
            let prefix: Vec<TermId> = order.cols().iter().map(|c| q[*c as usize]).collect();
            let spec = ScanSpec::new(order, &prefix);
            let want = expected_rows(&quads, &spec).len();
            let mut cur = db.store.cursor(&spec).unwrap();
            let mut buf = Vec::new();
            let mut n = 0;
            loop {
                let k = cur.fill(&mut buf, 5).unwrap();
                if k == 0 {
                    break;
                }
                n += k;
            }
            assert_eq!(n, want, "{spec:?}");
            assert_eq!(db.store.count(&spec, None).unwrap() as usize, want, "{spec:?}");
            assert_eq!(db.store.count(&spec, Some(0)).unwrap() as usize, want.min(1), "{spec:?}");
            // restart via an empty seek
            cur.seek(&[]).unwrap();
            assert_eq!(cur.fill(&mut buf, 5).unwrap(), want, "{spec:?} after seek(&[])");
        }
    }
}

/// §2.3 step 6: "If this is the first load or the store grew by more than 25% since
/// `analyzed_quads`, run analyze()". Loads smaller than 1/16 of `analyzed_quads` never
/// measure growth, so a store that grows through many small loads keeps its first
/// statistics forever (here: +60% in ten loads of 6% each, no re-analysis). A running
/// counter in `rdf_meta` (quads added since the last analyze) would make the check O(1).
#[test]
fn review_many_small_loads_trigger_reanalyze() {
    use crate::{BulkMode, LoadOptions, LoadSource};
    let db = Db::mem();
    let nt = |from: usize, to: usize| -> String {
        (from..to).map(|i| format!("<http://ex/s{i}> <http://ex/p{}> \"literal number {i}\" .\n", i % 7)).collect()
    };
    let opts = LoadOptions { bulk: BulkMode::Never, ..LoadOptions::new(oxrdfio::RdfFormat::NTriples) };
    db.store.bulk_load(LoadSource::Bytes(nt(0, 2000).as_bytes()), &opts).unwrap();
    let analyzed = || db.query_i64("SELECT value FROM rdf_meta WHERE key='analyzed_quads'");
    assert_eq!(analyzed(), 2000, "first load analyzes");
    for k in 0..10 {
        let from = 2000 + k * 120;
        db.store.bulk_load(LoadSource::Bytes(nt(from, from + 120).as_bytes()), &opts).unwrap();
    }
    let total = db.query_i64("SELECT count(*) FROM rdf_quad");
    assert_eq!(total, 3200);
    assert!(
        analyzed() as f64 * 1.25 >= total as f64,
        "store grew from {} to {total} quads (+{:.0}%) through loads without re-analysis",
        analyzed(),
        (total as f64 / analyzed() as f64 - 1.0) * 100.0
    );
}

/// Lexical forms with an embedded NUL (legal in RDF, `\u0000` in N-Triples) and non-ASCII
/// text survive load, hash probe, lookup and decode (TEXT is bound with explicit lengths).
#[test]
fn review_nul_and_unicode_lexical_forms_round_trip() {
    use crate::{BulkMode, LoadOptions, LoadSource};
    let db = Db::mem();
    let opts = LoadOptions { bulk: BulkMode::Never, ..LoadOptions::new(oxrdfio::RdfFormat::NTriples) };
    let data = "<http://ex/s> <http://ex/p> \"a\\u0000b and more text\" .\n<http://ex/s> <http://ex/p> \"a\\u0000c and more text\" .\n<http://ex/s> <http://ex/p> \"\u{1F600} emoji literal\"@EN-gb .\n";
    let r = db.store.bulk_load(LoadSource::Bytes(data.as_bytes()), &opts).unwrap();
    assert_eq!(r.quads_new, 3);
    // reload: every term found by hash probe, no new rows
    assert_eq!(db.store.bulk_load(LoadSource::Bytes(data.as_bytes()), &opts).unwrap().terms_new, 0);
    db.shared().validate_epoch().unwrap();
    for lit in [
        oxrdf::Literal::new_simple_literal("a\0b and more text"),
        oxrdf::Literal::new_simple_literal("a\0c and more text"),
        oxrdf::Literal::new_language_tagged_literal("\u{1F600} emoji literal", "en-GB").unwrap(),
    ] {
        let id = db.store.lookup(lit.as_ref().into()).unwrap().unwrap_or_else(|| panic!("{lit} not found"));
        let mut out = Vec::new();
        db.store.decode(&[id], &mut out).unwrap();
        assert_eq!(out[0].to_oxrdf(), lit.clone().into());
    }
    assert!(db.store.check().unwrap().starts_with("ok"));
}

/// `WriteScope::commit` when the COMMIT (RELEASE of the outermost savepoint) is refused with
/// SQLITE_BUSY (rollback-journal file database; another connection holds a read
/// transaction, busy_timeout 0). `commit` then rolls back and must leave the connection
/// outside any transaction with nothing written; a failing second RELEASE is ignored.
#[test]
fn review_busy_commit_does_not_leak_the_write_transaction() {
    let db = Db::file();
    db.insert(&random_quads(12, 20, false));
    let before = db.query_i64("SELECT count(*) FROM rdf_quad");
    db.conn.busy_timeout(Duration::ZERO).unwrap();
    let reader = db.reopen();
    reader.conn.execute_batch("BEGIN; SELECT count(*) FROM rdf_quad;").unwrap();
    let ws = db.store.write_scope(0).unwrap();
    let mut more = random_quads(13, 30, false);
    sq_core::store::QuadStoreWriter::insert(&db.store, &mut more).unwrap();
    let committed = ws.commit();
    let leaked = !db.shared().db().autocommit();
    reader.conn.execute_batch("COMMIT").unwrap();
    let after = db.query_i64("SELECT count(*) FROM rdf_quad");
    if leaked {
        let _ = db.conn.execute_batch("COMMIT");
    }
    let durable = db.query_i64("SELECT count(*) FROM rdf_quad");
    assert!(committed.is_err(), "commit must report SQLITE_BUSY");
    assert!(
        !leaked && after == before && durable == before,
        "failed commit: transaction left open = {leaked}, quads visible {after} / after a later COMMIT {durable} (before: {before})"
    );
}
