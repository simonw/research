//! Maintenance: `sparql_vacuum_terms()`, `sparql_check()` and `EXPLAIN QUERY PLAN`.
use std::fmt::Write as _;

use rustc_hash::FxHashMap;
use sq_core::codec::{self, FIRST_USER_ROW, WELL_KNOWN};
use sq_core::store::{IndexOrder, QuadCursor, ScanSpec};
use sq_core::{Error, Result, Tag, TermId};

use crate::cursor::{bind_count, SqlCursor};
use crate::pool::PoolKey;
use crate::raw::{RawDb, RawStmt};
use crate::shared::StorageShared;
use crate::template::{StmtKey, StmtKind};

/// Distinct-value streams covering every column of every quad, each read in index order by
/// the VDBE (no temp B-tree): (s, p, o) of the default graph via g-first orders and of the
/// named graphs via the partial g-last orders. Graph names come from `named_graph_ids`.
const DISTINCT_COLUMNS: [&str; 6] = [
    "SELECT DISTINCT s FROM main.rdf_quad INDEXED BY sqlite_autoindex_rdf_quad_1 WHERE g=0",
    "SELECT DISTINCT p FROM main.rdf_quad INDEXED BY rdf_gpso WHERE g=0",
    "SELECT DISTINCT o FROM main.rdf_quad INDEXED BY rdf_gosp WHERE g=0",
    "SELECT DISTINCT s FROM main.rdf_quad INDEXED BY rdf_spog WHERE g<>0",
    "SELECT DISTINCT p FROM main.rdf_quad INDEXED BY rdf_psog WHERE g<>0",
    "SELECT DISTINCT o FROM main.rdf_quad INDEXED BY rdf_ospg WHERE g<>0",
];

/// Every id used in a quad position or as a graph name (registry included), sorted, distinct.
fn used_ids(shared: &StorageShared) -> Result<Vec<i64>> {
    let mut ids: Vec<i64> = shared.named_graph_ids()?.into_iter().map(TermId::raw).collect();
    for sql in DISTINCT_COLUMNS {
        let mut st = RawStmt::prepare(shared.db(), sql, false)?;
        while st.step()? {
            ids.push(st.col_i64(0));
        }
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

/// Deletes dictionary rows (>= FIRST_USER_ROW) that no quad, graph name or kept TYPED
/// literal (datatype) references; bumps `dict_gen`. The caller holds a write scope.
pub(crate) fn vacuum_terms(shared: &StorageShared) -> Result<u64> {
    let db = shared.db();
    let mut used: Vec<i64> = used_ids(shared)?.into_iter().filter(|id| TermId(*id).is_dict()).collect();
    {
        let mut st = RawStmt::prepare(db, "SELECT id, dt FROM main.rdf_term WHERE tag=5", false)?;
        let mut dts = Vec::new();
        while st.step()? {
            let id = TermId::from_dict(st.col_i64(0), Tag::Typed);
            if used.binary_search(&id.raw()).is_ok()
                && let Some(dt) = st.col_opt_i64(1)
            {
                dts.push(dt);
            }
        }
        used.extend(dts);
        used.sort_unstable();
        used.dedup();
    }
    let mut victims = Vec::new();
    {
        let mut st = RawStmt::prepare(db, "SELECT id, tag FROM main.rdf_term WHERE id>=?1 ORDER BY id", false)?;
        st.bind_i64(1, FIRST_USER_ROW)?;
        while st.step()? {
            let row = st.col_i64(0);
            let id = (row << 4) | (st.col_i64(1) & 15);
            if used.binary_search(&id).is_err() {
                victims.push(row);
            }
        }
    }
    if victims.is_empty() {
        return Ok(0);
    }
    let mut del = RawStmt::prepare(db, "DELETE FROM main.rdf_term WHERE id=?1", false)?;
    for row in &victims {
        del.reset();
        del.bind_i64(1, *row)?;
        del.run()?;
    }
    shared.meta_bump("dict_gen")?;
    shared.note_dictionary_rows_deleted()?;
    Ok(victims.len() as u64)
}

const MAX_REPORTED: usize = 100;

/// Exact identity of a dictionary row.
#[derive(PartialEq, Eq)]
struct RowKey {
    tag: i64,
    lex: String,
    dt: Option<i64>,
    lang: Option<String>,
}

struct Problems {
    list: Vec<String>,
    total: usize,
}

impl Problems {
    fn add(&mut self, msg: String) {
        self.total += 1;
        if self.list.len() < MAX_REPORTED {
            self.list.push(msg);
        }
    }
}

/// `sparql_check()` invariants: no inline-eligible dictionary rows, consistent tags / hashes /
/// language tags / datatypes, no duplicate dictionary keys, no dangling ids, index/table
/// count parity, registry ⊇ used graphs. Returns "ok: …" or one line per problem.
pub(crate) fn check(shared: &StorageShared) -> Result<String> {
    let db = shared.db();
    let mut p = Problems { list: Vec::new(), total: 0 };

    // --- dictionary rows
    let mut iris: FxHashMap<i64, String> = FxHashMap::default();
    {
        let mut st = RawStmt::prepare(db, "SELECT id, lex FROM main.rdf_term WHERE tag=1", false)?;
        while st.step()? {
            iris.insert(st.col_i64(0), st.col_str(1)?.to_string());
        }
    }
    for (row, iri) in WELL_KNOWN {
        if iris.get(row).map(String::as_str) != Some(*iri) {
            p.add(format!("well-known row {row} is not <{iri}>"));
        }
    }
    let mut terms = 0u64;
    let mut rows: Vec<(i64, i64)> = Vec::new();
    {
        let mut st = RawStmt::prepare(db, "SELECT id, tag, lex, dt, lang, h FROM main.rdf_term ORDER BY id", false)?;
        while st.step()? {
            terms += 1;
            let (row, tag_raw) = (st.col_i64(0), st.col_i64(1));
            rows.push((row, tag_raw));
            let lex = st.col_str(2)?;
            let (dt, lang, h) = (st.col_opt_i64(3), st.col_opt_str(4)?, st.col_i64(5));
            if row > WELL_KNOWN.len() as i64 && row < FIRST_USER_ROW {
                p.add(format!("row {row} is in the reserved range"));
            }
            let (tag, extra, datatype) = match tag_raw {
                1 => (Tag::Iri, String::new(), None),
                3 => (Tag::Str, String::new(), Some(codec::XSD_STRING_IRI.to_string())),
                4 => {
                    let lang = lang.unwrap_or_default();
                    if lang.is_empty() || lang.bytes().any(|b| b.is_ascii_uppercase()) {
                        p.add(format!("row {row}: invalid language tag {lang:?}"));
                    }
                    (Tag::Lang, lang.to_string(), None)
                }
                5 => {
                    let dt_iri = dt.filter(|d| TermId(*d).tag() == Tag::Iri).and_then(|d| iris.get(&(d >> 4)).cloned());
                    let Some(dt_iri) = dt_iri else {
                        p.add(format!("row {row}: TYPED literal with dangling datatype {dt:?}"));
                        continue;
                    };
                    if dt_iri == codec::XSD_STRING_IRI || dt_iri == codec::RDF_LANG_STRING_IRI {
                        p.add(format!("row {row}: TYPED literal with datatype <{dt_iri}>"));
                    }
                    (Tag::Typed, dt_iri.clone(), Some(dt_iri))
                }
                t => {
                    p.add(format!("row {row}: invalid tag {t}"));
                    continue;
                }
            };
            if let Some(datatype) = datatype
                && codec::encode_inline_literal(lex, &datatype, None).is_some()
            {
                p.add(format!("row {row}: {lex:?}^^<{datatype}> is inline-eligible"));
            }
            if h != codec::term_hash(tag, lex, &extra) {
                p.add(format!("row {row}: stored hash does not match the term"));
            }
        }
    }
    // duplicate keys (same hash and same exact key)
    {
        let mut dup = RawStmt::prepare(db, "SELECT h FROM main.rdf_term GROUP BY h HAVING count(*) > 1", false)?;
        let mut by_h = RawStmt::prepare(db, "SELECT id, tag, lex, dt, lang FROM main.rdf_term WHERE h=?1", false)?;
        while dup.step()? {
            by_h.reset();
            by_h.bind_i64(1, dup.col_i64(0))?;
            // (row, exact key) of every row sharing this hash.
            let mut seen: Vec<(i64, RowKey)> = Vec::new();
            while by_h.step()? {
                let key = RowKey {
                    tag: by_h.col_i64(1),
                    lex: by_h.col_str(2)?.to_string(),
                    dt: by_h.col_opt_i64(3),
                    lang: by_h.col_opt_str(4)?.map(str::to_string),
                };
                if let Some((other, _)) = seen.iter().find(|(_, k)| *k == key) {
                    p.add(format!("rows {other} and {} hold the same term", by_h.col_i64(0)));
                }
                seen.push((by_h.col_i64(0), key));
            }
        }
    }

    // --- quads: one primary-key scan for the expected per-index counts and graph names
    let mut total = 0u64;
    let mut named = 0u64;
    let mut numeric = 0u64;
    let mut graphs: Vec<i64> = Vec::new();
    {
        let mut cur = SqlCursor::open(shared.pool.clone(), &ScanSpec::new(IndexOrder::Gspo, &[]))?;
        let mut buf = Vec::with_capacity(4096);
        loop {
            buf.clear();
            if cur.fill(&mut buf, 1024)? == 0 {
                break;
            }
            for q in buf.chunks_exact(4) {
                total += 1;
                if q[0] != TermId::DEFAULT_GRAPH {
                    named += 1;
                    if graphs.last() != Some(&q[0].0) {
                        graphs.push(q[0].0);
                    }
                }
                if matches!(q[3].0 & 15, 5 | 8) {
                    numeric += 1;
                }
            }
        }
    }
    for order in IndexOrder::ALL.into_iter().filter(|o| *o != IndexOrder::Gspo) {
        let expected = if order.named_only() {
            named
        } else if order.numeric_exceptions() {
            numeric
        } else {
            total
        };
        let spec = ScanSpec::new(order, &[]);
        let key = StmtKey::for_spec(&spec, StmtKind::Count);
        let mut st = shared.pool.lease(PoolKey::Scan(key))?;
        bind_count(&mut st, &key, &spec, None)?;
        st.step()?;
        let n = st.col_i64(0) as u64;
        if n != expected {
            p.add(format!("index {} holds {n} rows, the table implies {expected}", order.sql_from()));
        }
    }

    // --- registry ⊇ used graphs
    let registry: Vec<i64> = {
        let mut st = RawStmt::prepare(db, "SELECT g FROM main.rdf_graph ORDER BY g", false)?;
        let mut v = Vec::new();
        while st.step()? {
            v.push(st.col_i64(0));
        }
        v
    };
    for g in &graphs {
        if registry.binary_search(g).is_err() {
            p.add(format!("graph {g} has quads but is not in rdf_graph"));
        }
    }

    // --- dangling / non-storable ids
    let next_bnode = shared.meta_get("next_bnode")?.unwrap_or(1);
    for id in used_ids(shared)?.into_iter().map(TermId) {
        if id.is_dict() {
            match rows.binary_search_by_key(&id.dict_row(), |r| r.0) {
                Ok(k) if rows[k].1 == id.tag() as i64 => {}
                Ok(_) => p.add(format!("id {} has a different tag than dictionary row {}", id.0, id.dict_row())),
                Err(_) => p.add(format!("id {} references missing dictionary row {}", id.0, id.dict_row())),
            }
        } else if id.tag() == Tag::BNode && id.is_storable() {
            if id.bnode_counter() >= next_bnode {
                p.add(format!("blank node {} was never allocated (next_bnode {next_bnode})", codec::bnode_label(id)));
            }
        } else if !id.is_inline() && id != TermId::DEFAULT_GRAPH {
            p.add(format!("id {} is not storable", id.0));
        }
    }

    if p.total == 0 {
        return Ok(format!("ok: {terms} terms, {total} quads, {} named graphs", graphs.len().max(registry.len())));
    }
    let mut out = format!("errors: {}", p.total);
    for m in &p.list {
        let _ = write!(out, "\n{m}");
    }
    if p.total > p.list.len() {
        let _ = write!(out, "\n… {} more", p.total - p.list.len());
    }
    Ok(out)
}

/// `EXPLAIN QUERY PLAN <sql>` rendered as one line per node, indented two spaces per level.
pub(crate) fn explain_query_plan(db: RawDb, sql: &str) -> Result<String> {
    let mut st = RawStmt::prepare(db, &format!("EXPLAIN QUERY PLAN {sql}"), false)?;
    let mut depth: FxHashMap<i64, usize> = FxHashMap::default();
    let mut out = String::new();
    while st.step()? {
        let (id, parent) = (st.col_i64(0), st.col_i64(1));
        let d = if parent == 0 { 0 } else { depth.get(&parent).map_or(0, |d| d + 1) };
        depth.insert(id, d);
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&"  ".repeat(d));
        out.push_str(st.col_str(3).map_err(|_| Error::internal("non-UTF-8 query plan"))?);
    }
    Ok(out)
}
