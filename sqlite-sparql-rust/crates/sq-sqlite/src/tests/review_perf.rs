//! Adversarial review of the storage performance pass (B-tree range counts, direct
//! `rdf_term` reads, `decode_one`): differential checks against the VDBE / SQL paths on
//! shapes the conformance suite does not generate (extreme and non-integer key values written
//! by plain SQL, own commits, rollbacks with spilled pages, a zero page-cache budget, lexical
//! forms at the overflow threshold, sparse and very large rowids, odd `rdf_term` records).
use std::ops::Bound;

use rusqlite::Connection;
use sq_core::store::{Dictionary, IndexOrder, QuadStore, QuadStoreWriter, ScanSpec};
use sq_core::testing::{Rng, random_quads};
use sq_core::{Tag, TermId};

use super::{Db, TempPath, iri};
use crate::ReaderMode;

fn last_fallback(db: &Db) -> String {
    db.shared().status().into_iter().find(|(k, _)| k == "btree_last_fallback").map(|(_, v)| v).unwrap_or_default()
}

/// A file database with `page_size` (store schema only).
fn empty_file_db(page_size: u32) -> Db {
    let path = TempPath::new("review-perf");
    let conn = Connection::open(&path.0).unwrap();
    conn.execute_batch(&format!("PRAGMA page_size={page_size};")).unwrap();
    Db::with(conn, Some(path))
}

/// `count(spec, cap)` through the VDBE (kill switch on) and through the store's default path.
fn both_counts(db: &Db, spec: &ScanSpec, cap: Option<u64>) -> (u64, u64) {
    db.shared().set_reader(ReaderMode::Sql);
    let vdbe = db.store.count(spec, cap).unwrap();
    db.shared().set_reader(ReaderMode::Auto);
    let auto = db.store.count(spec, cap).unwrap();
    (vdbe, auto)
}

/// Integer values of every serial-type width, the i64 extremes and their neighbours.
const EXTREMES: [i64; 22] = [
    i64::MIN,
    i64::MIN + 1,
    -(1 << 47) - 1,
    -(1 << 31),
    -8_388_609,
    -32769,
    -129,
    -1,
    0,
    1,
    2,
    127,
    128,
    32768,
    8_388_608,
    1 << 31,
    1 << 40,
    (1 << 47) - 1,
    1 << 47,
    1 << 59,
    i64::MAX - 1,
    i64::MAX,
];

/// Random specs over `vals` (values present in the data plus extremes), every order and prefix
/// length, every bound shape on the first free column, several caps: the B-tree count must
/// equal the VDBE count.
fn differential_counts(db: &Db, vals: &[i64], seed: u64, iterations: usize) -> (u64, u64) {
    use Bound::{Excluded as Ex, Included as In, Unbounded as Un};
    let mut r = Rng(seed | 1);
    let pick = |r: &mut Rng| vals[r.below(vals.len() as u64) as usize];
    let before = db.shared().btree_counters();
    let mut calls = 0u64;
    for order in IndexOrder::ALL {
        for prefix_len in 0..=4usize {
            for _ in 0..iterations {
                let prefix: Vec<TermId> = (0..prefix_len).map(|_| TermId(pick(&mut r))).collect();
                let (a, b) = (pick(&mut r), pick(&mut r));
                let (lo, hi) = (a.min(b), a.max(b));
                let shapes = [
                    (Un, Un),
                    (In(lo), Un),
                    (Ex(lo), Un),
                    (Un, In(hi)),
                    (Un, Ex(hi)),
                    (In(lo), In(hi)),
                    (Ex(lo), Ex(hi)),
                    (In(i64::MIN), In(i64::MAX)),
                    (Ex(i64::MIN), Ex(i64::MAX)),
                    (In(i64::MAX), Un),
                    (Un, In(i64::MIN)),
                ];
                for (l, u) in shapes {
                    let spec = ScanSpec::new(order, &prefix).with_range(l, u);
                    for cap in [None, Some(0), Some(1), Some(7), Some(u64::MAX), Some(i64::MAX as u64)] {
                        let (vdbe, auto) = both_counts(db, &spec, cap);
                        assert_eq!(auto, vdbe, "count({spec:?}, {cap:?}): B-tree path vs VDBE ({})", last_fallback(db));
                        calls += 1;
                    }
                }
            }
        }
    }
    let after = db.shared().btree_counters();
    (calls, after.counts - before.counts)
}

/// Keys of every integer width, the i64 extremes included, written by plain SQL (the
/// conformance data only uses small ids): every count is answered by the reader and equals
/// the VDBE count, at 512-byte pages (deep trees, many interior entries) and 4K pages.
#[test]
fn review_perf_counts_with_extreme_integer_keys_match_the_vdbe() {
    for (page_size, n) in [(512u32, 6000usize), (4096, 20_000)] {
        let db = empty_file_db(page_size);
        let mut r = Rng(u64::from(page_size) * 7 + 1);
        let mut vals: Vec<i64> = EXTREMES.to_vec();
        {
            let tx = db.conn.unchecked_transaction().unwrap();
            let mut st = tx.prepare("INSERT OR IGNORE INTO rdf_quad(g, s, p, o) VALUES (?1, ?2, ?3, ?4)").unwrap();
            for _ in 0..n {
                let mut v = [0i64; 4];
                for x in &mut v {
                    *x = match r.below(4) {
                        0 => EXTREMES[r.below(EXTREMES.len() as u64) as usize],
                        1 => r.below(1 << 62) as i64 * if r.below(2) == 0 { 1 } else { -1 },
                        // A few repeated values so that prefixes select several rows; some
                        // match the rdf_gpox predicate ((o & 15) IN (5, 8)).
                        _ => [0i64, 5, 8, 21, 24, -11, 1 << 40 | 5][r.below(7) as usize],
                    };
                }
                st.execute(rusqlite::params![v[0], v[1], v[2], v[3]]).unwrap();
                if r.below(8) == 0 {
                    vals.extend_from_slice(&v);
                }
            }
            drop(st);
            tx.commit().unwrap();
        }
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        assert!(db.shared().btree_active(), "{}", last_fallback(&db));
        let fallbacks = db.shared().btree_counters().fallbacks;
        let (calls, served) = differential_counts(&db, &vals, u64::from(page_size), 3);
        assert_eq!(served, calls, "every count is answered by the reader: {}", last_fallback(&db));
        assert_eq!(db.shared().btree_counters().fallbacks, fallbacks, "{}", last_fallback(&db));
    }
}

/// Non-integer key values (REAL inside and outside the i64 range, TEXT, BLOB) written by plain
/// SQL next to integer rows: whether the walk meets them (and falls back) or not, the count
/// equals the VDBE count, whose WHERE clause compares them numerically / by storage class.
#[test]
fn review_perf_counts_with_non_integer_keys_match_the_vdbe() {
    let db = empty_file_db(512);
    let mut r = Rng(4242);
    let odd: [&str; 9] = ["5.5", "-0.5", "9.3e18", "-9.3e18", "1e300", "-1e300", "'abc'", "'5x'", "x'00ff'"];
    {
        let tx = db.conn.unchecked_transaction().unwrap();
        for _ in 0..4000 {
            let v: Vec<String> = (0..4).map(|_| [0i64, 1, 5, 8, 21, -3][r.below(6) as usize].to_string()).collect();
            tx.execute(&format!("INSERT OR IGNORE INTO rdf_quad(g, s, p, o) VALUES ({})", v.join(",")), []).unwrap();
        }
        for _ in 0..60 {
            let mut v: Vec<String> = (0..4).map(|_| [0i64, 1, 5, 8, 21, -3][r.below(6) as usize].to_string()).collect();
            let c = r.below(4) as usize;
            v[c] = odd[r.below(odd.len() as u64) as usize].to_string();
            tx.execute(&format!("INSERT OR IGNORE INTO rdf_quad(g, s, p, o) VALUES ({})", v.join(",")), []).unwrap();
        }
        tx.commit().unwrap();
    }
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let vals = [0i64, 1, 5, 8, 21, -3, 6, i64::MIN, i64::MAX];
    let (calls, served) = differential_counts(&db, &vals, 99, 4);
    // Some counts must still be answered by the reader (the odd rows are few).
    assert!(served > 0 && served < calls, "{served} of {calls} counts answered by the reader");
}

/// The connection's own commits, a rolled-back transaction whose dirty pages spilled to the
/// database file (tiny `cache_size`), deleting every row: counts in the next read scope see
/// exactly the committed data.
#[test]
fn review_perf_counts_follow_own_commits_and_spilled_rollbacks() {
    let q = random_quads(17, 6000, true);
    let db = empty_file_db(1024);
    db.insert(&q);
    let check = |db: &Db, want: &[[TermId; 4]], what: &str| {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        let before = db.shared().btree_counters().counts;
        for order in IndexOrder::ALL {
            let spec = ScanSpec::new(order, &[]);
            let n = want.iter().filter(|x| spec.matches(x)).count() as u64;
            assert_eq!(db.store.count(&spec, None).unwrap(), n, "{what} {order:?}");
            assert_eq!(db.store.count(&spec, Some(10)).unwrap(), n.min(11), "{what} {order:?}");
            if let Some(x) = want.first() {
                let spec = ScanSpec::new(order, &[x[order.cols()[0] as usize]]);
                let n = want.iter().filter(|y| spec.matches(y)).count() as u64;
                assert_eq!(db.store.count(&spec, None).unwrap(), n, "{what} {spec:?}");
            }
        }
        assert!(db.shared().btree_counters().counts > before, "{what}: {}", last_fallback(db));
    };
    check(&db, &q, "initial");
    // Own commit.
    let extra = random_quads(18, 3000, true);
    db.insert(&extra);
    let mut all: Vec<_> = q.iter().chain(&extra).copied().collect();
    all.sort();
    all.dedup();
    check(&db, &all, "after own commit");
    // A large transaction that spills dirty pages to the file, then ROLLBACK.
    db.conn.execute_batch("PRAGMA cache_size=8; BEGIN").unwrap();
    {
        let ws = db.store.write_scope(0).unwrap();
        db.store.insert(&mut random_quads(19, 20_000, true)).unwrap();
        db.store.remove(&mut all.iter().copied().step_by(2).collect::<Vec<_>>()).unwrap();
        ws.commit().unwrap();
    }
    db.conn.execute_batch("ROLLBACK; PRAGMA cache_size=-2000").unwrap();
    db.shared().after_rollback();
    check(&db, &all, "after a spilled rollback");
    // Delete everything: every root page is an empty leaf (or an interior page left over).
    db.remove(&all);
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    for order in IndexOrder::ALL {
        let spec = ScanSpec::new(order, &[]);
        assert_eq!(db.store.count(&spec, None).unwrap(), 0, "{order:?}");
        assert_eq!(db.store.count(&spec, Some(0)).unwrap(), 0, "{order:?}");
    }
}

/// A zero (and tiny) page-cache budget: every page is read through the VFS; counts stay exact.
#[test]
fn review_perf_counts_with_a_zero_page_cache_budget() {
    let q = random_quads(23, 15_000, true);
    let db = empty_file_db(1024);
    db.insert(&q);
    for mb in [0u64, 1] {
        db.shared().set_page_cache_mb(mb).unwrap();
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        let before = db.shared().btree_counters();
        for order in IndexOrder::ALL {
            let spec = ScanSpec::new(order, &[]);
            let n = q.iter().filter(|x| spec.matches(x)).count() as u64;
            assert_eq!(db.store.count(&spec, None).unwrap(), n, "{mb} MB {order:?}");
            let spec = ScanSpec::new(order, &[q[q.len() / 2][order.cols()[0] as usize]]);
            let n = q.iter().filter(|x| spec.matches(x)).count() as u64;
            assert_eq!(db.store.count(&spec, Some(n / 2)).unwrap(), n.min(n / 2 + 1), "{mb} MB {spec:?}");
        }
        let after = db.shared().btree_counters();
        assert_eq!(after.counts - before.counts, 18, "{}", last_fallback(&db));
        assert_eq!(after.fallbacks, before.fallbacks, "{}", last_fallback(&db));
    }
}

// ---------------------------------------------------------------------- dictionary rows

/// Decodes `ids` cold, one by one and in one batch, through the reader and through SQL: every
/// `decode_one` result (term or error text) must agree, and the batch must agree in its terms
/// or fail on both paths.
fn decode_both_ways(db: &Db, ids: &[TermId]) -> u64 {
    let run = |mode: ReaderMode| -> (Vec<Result<String, String>>, Result<Vec<String>, String>) {
        db.shared().set_reader(mode);
        db.shared().flush_dictionary_caches().unwrap();
        let one: Vec<Result<String, String>> =
            ids.iter().map(|id| db.store.decode_one(*id).map(|t| t.to_oxrdf().to_string()).map_err(|e| e.to_string())).collect();
        db.shared().flush_dictionary_caches().unwrap();
        let mut out = Vec::new();
        let batch = db.store.decode(ids, &mut out).map(|()| out.iter().map(|t| t.to_oxrdf().to_string()).collect()).map_err(|e| e.to_string());
        (one, batch)
    };
    let sql = run(ReaderMode::Sql);
    let before = db.shared().btree_counters().term_rows;
    let direct = run(ReaderMode::Auto);
    let served = db.shared().btree_counters().term_rows - before;
    db.shared().set_reader(ReaderMode::Auto);
    for (i, (a, b)) in direct.0.iter().zip(&sql.0).enumerate() {
        assert_eq!(a, b, "decode_one({:?})", ids[i]);
    }
    // A batch fails as a whole. When it holds several bad ids, the two paths may report
    // different ones: the direct path checks the rows it reads first (e.g. a wrong tag) and
    // leaves the others (e.g. invalid UTF-8) to SQL, which reports the first in id order.
    match (&direct.1, &sql.1) {
        (Ok(a), Ok(b)) => assert_eq!(a, b, "decode batch"),
        (Err(_), Err(_)) => {}
        (a, b) => panic!("decode batch: direct {a:?} vs SQL {b:?}"),
    }
    served
}

/// Lexical forms whose record size straddles the table-leaf local limit (`usable - 35`) at
/// 512, 1024 (with 32 reserved bytes) and 4096-byte pages, for every dictionary tag.
#[test]
fn review_perf_decode_lexical_forms_at_the_overflow_threshold() {
    for (page_size, reserved) in [(512u32, 0i32), (1024, 32), (4096, 0)] {
        let path = TempPath::new("review-perf-terms");
        let conn = Connection::open(&path.0).unwrap();
        conn.execute_batch(&format!("PRAGMA page_size={page_size};")).unwrap();
        if reserved > 0 {
            let mut n = reserved;
            // SAFETY: valid connection, NUL-terminated name, one int in/out; empty database.
            let rc = unsafe {
                rusqlite::ffi::sqlite3_file_control(conn.handle(), c"main".as_ptr(), rusqlite::ffi::SQLITE_FCNTL_RESERVE_BYTES, (&raw mut n).cast())
            };
            assert_eq!(rc, rusqlite::ffi::SQLITE_OK);
        }
        let db = Db::with(conn, Some(path));
        let usable = page_size as usize - reserved as usize;
        let mut terms: Vec<oxrdf::Term> = Vec::new();
        for len in usable.saturating_sub(70)..usable + 6 {
            let lex: String = "x".repeat(len);
            terms.push(iri(&format!("http://e/{}", "i".repeat(len.saturating_sub(9)))).into());
            terms.push(oxrdf::Literal::new_simple_literal(lex.clone()).into());
            terms.push(oxrdf::Literal::new_language_tagged_literal(lex.clone(), "en-gb").unwrap().into());
            terms.push(oxrdf::Literal::new_typed_literal(lex.clone(), iri("http://e/dt")).into());
            // Two-byte characters: the byte length, not the char count, decides.
            terms.push(oxrdf::Literal::new_simple_literal("é".repeat(len / 2)).into());
        }
        let ids: Vec<TermId> = {
            let ws = db.store.write_scope(0).unwrap();
            let ids = terms.iter().map(|t| db.store.intern(t.as_ref()).unwrap()).collect();
            ws.commit().unwrap();
            ids
        };
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        let served = decode_both_ways(&db, &ids);
        assert!(served > 0, "{page_size}: nothing read directly ({})", last_fallback(&db));
        assert_eq!(db.shared().btree_counters().fallbacks, 0, "{}", last_fallback(&db));
    }
}

/// `rdf_term` rows written by plain SQL: very large and sparse rowids (multi-byte varint keys
/// in interior and leaf cells, interpolation over huge gaps), and records of odd shapes (BLOB
/// or invalid UTF-8 lexical forms, an embedded NUL, a TEXT datatype, an INTEGER language, an
/// empty language). Both paths return the same terms / errors.
#[test]
fn review_perf_decode_sparse_huge_rowids_and_odd_records() {
    let db = empty_file_db(1024);
    let mut rows: Vec<(i64, i64)> = Vec::new();
    {
        let tx = db.conn.unchecked_transaction().unwrap();
        // A dense run first (so the table has interior pages), then sparse huge rowids.
        let mut st = tx.prepare("INSERT INTO rdf_term(id, tag, lex, dt, lang, h) VALUES (?1, ?2, ?3, ?4, ?5, ?6)").unwrap();
        let dt_row = 1000i64;
        st.execute(rusqlite::params![dt_row, 1, "http://e/dt", None::<i64>, None::<String>, 1]).unwrap();
        rows.push((dt_row, 1));
        fn add(st: &mut rusqlite::Statement<'_>, rows: &mut Vec<(i64, i64)>, dt_row: i64, row: i64, tag: i64, lex: &str) {
            let (dt, lang): (Option<i64>, Option<&str>) = match tag {
                4 => (None, Some("de")),
                5 => (Some(TermId::from_dict(dt_row, Tag::Iri).0), None),
                _ => (None, None),
            };
            st.execute(rusqlite::params![row, tag, lex, dt, lang, row]).unwrap();
            rows.push((row, tag));
        }
        // 14000..19000 crosses 2^14: 2-byte and 3-byte rowid varints.
        for i in 0..5000i64 {
            add(&mut st, &mut rows, dt_row, 14_000 + i, [1, 3, 4, 5][(i % 4) as usize], &format!("term number {i} {}", "z".repeat((i % 50) as usize)));
        }
        let mut row = 2_097_151i64; // 2^21 - 1: the last 3-byte varint
        for k in 0..400i64 {
            add(&mut st, &mut rows, dt_row, row, [1, 3, 4, 5][(k % 4) as usize], &format!("sparse {row}"));
            row = match k % 5 {
                0 => row + 1,
                1 => row + 977,
                2 => row * 3 + 1,
                3 => row + (1 << 20),
                _ => row + 7,
            };
            if row >= (1i64 << 58) {
                row = (1i64 << 21) + 31 * k;
            }
        }
        for row in [(1i64 << 28) - 1, 1 << 28, (1 << 35) - 1, 1 << 35, 1 << 42, 1 << 49, 1 << 56, (1 << 59) - 1] {
            if !rows.iter().any(|r| r.0 == row) {
                add(&mut st, &mut rows, dt_row, row, 3, &format!("huge {row}"));
            }
        }
        drop(st);
        // Odd records (rows 900..): the SQL path decides what they decode to.
        for (row, tag, lex, dt, lang) in [
            (900i64, 3i64, "CAST(x'616263' AS BLOB)", "NULL", "NULL"),
            (901, 3, "CAST(x'ff61' AS TEXT)", "NULL", "NULL"),
            (902, 3, "'a' || char(0) || 'b'", "NULL", "NULL"),
            (903, 5, "'v'", "'not an id'", "NULL"),
            (904, 4, "'w'", "NULL", "42"),
            (905, 4, "'w'", "NULL", "''"),
            (906, 1, "''", "NULL", "NULL"),
            (907, 3, "'x'", "7", "'en'"),
            (908, 5, "'y'", "NULL", "NULL"),
        ] {
            tx.execute(&format!("INSERT INTO rdf_term(id, tag, lex, dt, lang, h) VALUES ({row}, {tag}, {lex}, {dt}, {lang}, 0)"), [])
                .unwrap();
            rows.push((row, tag));
        }
        tx.commit().unwrap();
    }
    rows.sort();
    let tag = |t: i64| match t {
        1 => Tag::Iri,
        3 => Tag::Str,
        4 => Tag::Lang,
        _ => Tag::Typed,
    };
    let mut ids: Vec<TermId> = rows.iter().map(|(row, t)| TermId::from_dict(*row, tag(*t))).collect();
    // Absent rows and a wrong tag: the same errors.
    ids.extend([TermId::from_dict(13_999, Tag::Iri), TermId::from_dict(1 << 57, Tag::Str), TermId::from_dict(14_001, Tag::Iri)]);
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    // Every id on its own (errors are per id), then batches of mixed good rows only.
    let served = decode_both_ways(&db, &ids);
    assert!(served > 5000, "{served} rows read directly ({})", last_fallback(&db));
    let good: Vec<TermId> = ids.iter().copied().filter(|id| id.dict_row() >= 1000 && rows.iter().any(|r| r.0 == id.dict_row() && tag(r.1) == id.tag())).collect();
    // Every well-formed row is read directly: a lookup that misses a row the b-tree holds would
    // still decode correctly (through SQL), so only the counter shows it.
    for one in [false, true] {
        db.shared().flush_dictionary_caches().unwrap();
        let before = db.shared().btree_counters().term_rows;
        if one {
            for id in &good {
                db.store.decode_one(*id).unwrap();
            }
        } else {
            db.store.decode(&good, &mut Vec::new()).unwrap();
        }
        let direct = db.shared().btree_counters().term_rows - before;
        assert_eq!(direct, good.len() as u64, "decode_one {one}: rows read directly ({})", last_fallback(&db));
    }
    let mut r = Rng(5);
    for _ in 0..20 {
        let mut chunk: Vec<TermId> = (0..1 + r.below(300)).map(|_| good[r.below(good.len() as u64) as usize]).collect();
        if r.below(2) == 0 {
            chunk.sort();
        }
        decode_both_ways(&db, &chunk);
    }
}

/// `decode_one` agrees with `decode` for every kind of id: inline values, blank nodes,
/// well-known IRIs, dictionary hits and misses, and ids that cannot be decoded.
#[test]
fn review_perf_decode_one_agrees_with_decode_for_every_kind_of_id() {
    let db = empty_file_db(4096);
    let terms: Vec<oxrdf::Term> = vec![
        iri("http://e/a").into(),
        oxrdf::Literal::new_simple_literal("s").into(),
        oxrdf::Literal::new_language_tagged_literal("l", "fr").unwrap().into(),
        oxrdf::Literal::new_typed_literal("t", iri("http://e/dt")).into(),
        iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").into(),
        oxrdf::Literal::new_typed_literal("42", iri("http://www.w3.org/2001/XMLSchema#integer")).into(),
    ];
    let mut ids: Vec<TermId> = {
        let ws = db.store.write_scope(0).unwrap();
        let ids = terms.iter().map(|t| db.store.intern(t.as_ref()).unwrap()).collect();
        ws.commit().unwrap();
        ids
    };
    ids.extend([
        TermId::bnode(3),
        TermId::from_int(-7).unwrap(),
        TermId::from_bool(true),
        TermId::UNBOUND,
        TermId::ERROR,
        TermId::temp(1, Tag::Iri),
        TermId::from_dict(1, Tag::Iri),
        TermId::from_dict(63, Tag::Iri),
        TermId::from_dict(1 << 40, Tag::Iri),
        TermId(-15),
    ]);
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    for pass in 0..2 {
        if pass == 0 {
            db.shared().flush_dictionary_caches().unwrap();
        }
        for id in &ids {
            let one = db.store.decode_one(*id).map(|t| t.to_oxrdf().to_string()).map_err(|e| e.to_string());
            let mut out = Vec::new();
            let many = db.store.decode(&[*id], &mut out).map(|()| out[0].to_oxrdf().to_string()).map_err(|e| e.to_string());
            assert_eq!(one, many, "pass {pass} {id:?}");
        }
    }
}

/// The kept `rdf_term` path and the connection's OWN commit: `vacuum_terms` on the same
/// connection deletes the last row, a new term reuses its row number; the next read scope
/// decodes the new term.
#[test]
fn review_perf_kept_term_path_after_own_commit_reusing_a_row() {
    let db = empty_file_db(1024);
    let terms: Vec<oxrdf::Term> = (0..3000).map(|i| oxrdf::Literal::new_simple_literal(format!("term {i}")).into()).collect();
    let ids: Vec<TermId> = {
        let ws = db.store.write_scope(0).unwrap();
        let p = db.store.intern(iri("http://e/p").as_ref().into()).unwrap();
        let ids: Vec<TermId> = terms.iter().map(|t| db.store.intern(t.as_ref()).unwrap()).collect();
        let mut quads: Vec<[TermId; 4]> =
            ids.iter().enumerate().map(|(i, o)| [TermId::DEFAULT_GRAPH, TermId::bnode(1 + i as i64), p, *o]).collect();
        db.store.insert(&mut quads).unwrap();
        ws.commit().unwrap();
        ids
    };
    let old_term: oxrdf::Term = oxrdf::Literal::new_simple_literal("old unreferenced last row").into();
    let old = {
        let ws = db.store.write_scope(0).unwrap();
        let id = db.store.intern(old_term.as_ref()).unwrap();
        ws.commit().unwrap();
        id
    };
    {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        db.shared().flush_dictionary_caches().unwrap();
        assert_eq!(db.store.decode_one(ids[7]).unwrap().to_oxrdf(), terms[7]);
        assert_eq!(db.store.decode_one(old).unwrap().to_oxrdf(), old_term);
    }
    assert!(db.store.vacuum_terms().unwrap() >= 1);
    let new_term: oxrdf::Term = oxrdf::Literal::new_simple_literal("new term reusing the row").into();
    let new = {
        let ws = db.store.write_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        let id = db.store.intern(new_term.as_ref()).unwrap();
        ws.commit().unwrap();
        id
    };
    assert_eq!(new.dict_row(), old.dict_row(), "the row number is reused");
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    assert_eq!(db.store.decode_one(new).unwrap().to_oxrdf(), new_term);
    db.shared().flush_dictionary_caches().unwrap();
    let before = db.shared().btree_counters().term_rows;
    assert_eq!(db.store.decode_one(new).unwrap().to_oxrdf(), new_term);
    assert_eq!(db.shared().btree_counters().term_rows, before + 1, "read directly: {}", last_fallback(&db));
}
