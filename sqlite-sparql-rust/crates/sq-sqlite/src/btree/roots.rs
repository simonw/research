//! Root pages of the nine quad b-trees, read from `sqlite_schema`, with each index's on-disk
//! layout verified before the reader trusts it.
//!
//! The PK b-tree of the WITHOUT ROWID table holds records `(g, s, p, o)`. A secondary index
//! of a WITHOUT ROWID table stores its own columns followed by the primary-key columns it does
//! not already contain. Each of our indexes names all four columns, so every b-tree record is
//! exactly the 4 columns in index order (checked on SQLite 3.45.1; `pragma_index_xinfo` then
//! lists exactly 4 key columns and no auxiliary column). This module checks precisely that, plus
//! the partial-index predicates. A b-tree whose definition differs (another column order, DESC
//! columns, a different or missing WHERE clause) is not read directly: its scans stay on the
//! SQL path.
use sq_core::Result;
use sq_core::store::IndexOrder;

use crate::raw::{RawDb, RawStmt};
use crate::schema::PK_INDEX;
use crate::template::COL_NAMES;

/// Root page of each order (`IndexOrder::ALL` positions) and of `rdf_term`; 0 = not readable
/// directly.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct Roots {
    quads: [u32; 9],
    term: u32,
}

const fn slot(order: IndexOrder) -> usize {
    match order {
        IndexOrder::Gspo => 0,
        IndexOrder::Gpso => 1,
        IndexOrder::Gpos => 2,
        IndexOrder::Gosp => 3,
        IndexOrder::Spog => 4,
        IndexOrder::Psog => 5,
        IndexOrder::Posg => 6,
        IndexOrder::Ospg => 7,
        IndexOrder::Gpox => 8,
    }
}

/// `sqlite_schema` name of an order's b-tree: the table itself for the primary key.
const fn schema_name(order: IndexOrder) -> &'static str {
    match order {
        IndexOrder::Gspo => "rdf_quad",
        IndexOrder::Gpso => "rdf_gpso",
        IndexOrder::Gpos => "rdf_gpos",
        IndexOrder::Gosp => "rdf_gosp",
        IndexOrder::Spog => "rdf_spog",
        IndexOrder::Psog => "rdf_psog",
        IndexOrder::Posg => "rdf_posg",
        IndexOrder::Ospg => "rdf_ospg",
        IndexOrder::Gpox => "rdf_gpox",
    }
}

impl Roots {
    /// Root page of `order`; 0 when it must be read through SQL.
    pub fn get(&self, order: IndexOrder) -> u32 {
        self.quads[slot(order)]
    }

    /// Every order is readable directly.
    pub fn all(&self) -> bool {
        self.quads.iter().all(|r| *r != 0)
    }

    /// Root page of `rdf_term`; 0 when dictionary rows must be read through SQL.
    pub fn term(&self) -> u32 {
        self.term
    }
}

/// One `sqlite_schema` row of `rdf_quad` or one of its indexes.
struct SchemaRow {
    kind: String,
    name: String,
    root: i64,
    sql: Option<String>,
}

/// Reads and verifies the root pages (one-shot statements; runs only when the schema cookie
/// changed). SQL errors are returned as errors; a layout the reader cannot use yields 0 roots.
pub(crate) fn load(db: RawDb) -> Result<Roots> {
    let mut rows = Vec::new();
    let mut st = RawStmt::prepare(
        db,
        "SELECT type, name, rootpage, sql FROM main.sqlite_schema WHERE tbl_name = 'rdf_quad'",
        false,
    )?;
    while st.step()? {
        rows.push(SchemaRow {
            kind: st.col_str(0)?.to_string(),
            name: st.col_str(1)?.to_string(),
            root: st.col_i64(2),
            sql: st.col_opt_str(3)?.map(str::to_string),
        });
    }
    drop(st);
    let mut roots = Roots::default();
    for order in IndexOrder::ALL {
        roots.quads[slot(order)] = verify(db, order, &rows)?.unwrap_or(0);
    }
    roots.term = verify_term(db)?.unwrap_or(0);
    Ok(roots)
}

/// The declared columns of `rdf_term` (§2.1): name, type, primary-key position.
const TERM_COLUMNS: [(&str, &str, i64); 6] =
    [("id", "INTEGER", 1), ("tag", "INTEGER", 0), ("lex", "TEXT", 0), ("dt", "INTEGER", 0), ("lang", "TEXT", 0), ("h", "INTEGER", 0)];

/// The root page of `rdf_term` if it is a rowid table with exactly the §2.1 columns, `id`
/// being the rowid alias.
fn verify_term(db: RawDb) -> Result<Option<u32>> {
    let mut st = RawStmt::prepare(db, "SELECT rootpage FROM main.sqlite_schema WHERE type = 'table' AND name = 'rdf_term'", false)?;
    let root = if st.step()? { st.col_i64(0) } else { return Ok(None) };
    drop(st);
    let Some(root) = u32::try_from(root).ok().filter(|r| *r >= 2) else { return Ok(None) };
    // PRAGMA statements here, not the eponymous `pragma_*` tables: see schema::PK_COLUMNS_SQL.
    // Rows in cid order: cid, name, type, notnull, dflt_value, pk, hidden.
    let mut st = RawStmt::prepare(db, "PRAGMA main.table_xinfo('rdf_term')", false)?;
    let mut cols = Vec::with_capacity(6);
    while st.step()? {
        cols.push((st.col_str(1)?.to_string(), st.col_str(2)?.to_ascii_uppercase(), st.col_i64(5), st.col_i64(6)));
    }
    drop(st);
    let columns_ok = cols.len() == TERM_COLUMNS.len()
        && cols.iter().zip(TERM_COLUMNS).all(|((n, t, pk, hidden), (wn, wt, wpk))| n == wn && t == wt && *pk == wpk && *hidden == 0);
    if !columns_ok {
        return Ok(None);
    }
    // A primary-key index means `id` is not the rowid (INTEGER PRIMARY KEY DESC, WITHOUT ROWID).
    // Rows: seq, name, unique, origin, partial.
    let mut st = RawStmt::prepare(db, "PRAGMA main.index_list('rdf_term')", false)?;
    let mut pk_index = false;
    while st.step()? {
        pk_index |= st.col_str(3)? == "pk";
    }
    Ok((!pk_index).then_some(root))
}

/// The root page of `order` if its b-tree has exactly the expected layout.
fn verify(db: RawDb, order: IndexOrder, rows: &[SchemaRow]) -> Result<Option<u32>> {
    let (kind, name) = if order == IndexOrder::Gspo { ("table", "rdf_quad") } else { ("index", schema_name(order)) };
    let Some(row) = rows.iter().find(|r| r.kind == kind && r.name == name) else { return Ok(None) };
    let Some(root) = u32::try_from(row.root).ok().filter(|r| *r >= 2) else { return Ok(None) };
    // Exactly the 4 order columns, ascending, all of them key columns (no auxiliary columns:
    // a rowid table's PK index would add the rowid, a table with more columns would add them).
    let index = if order == IndexOrder::Gspo { PK_INDEX } else { name };
    // A PRAGMA takes no bound parameters; `index` is one of our own (quote-free) names.
    // Rows in seqno order: seqno, cid, name, desc, coll, key.
    let mut st = RawStmt::prepare(db, &format!("PRAGMA main.index_xinfo('{index}')"), false)?;
    let mut cols = Vec::with_capacity(4);
    while st.step()? {
        cols.push((st.col_opt_str(2)?.unwrap_or("").to_string(), st.col_i64(3), st.col_i64(5)));
    }
    drop(st);
    let expected = order.cols().map(|c| COL_NAMES[c as usize]);
    let layout_ok = cols.len() == expected.len()
        && cols.iter().zip(expected).all(|((n, desc, key), want)| n == want && *desc == 0 && *key == 1);
    if !layout_ok {
        return Ok(None);
    }
    // Partial indexes must have exactly our predicate (as stored by CREATE INDEX), full ones none.
    // Rows: seq, name, unique, origin, partial.
    let mut st = RawStmt::prepare(db, "PRAGMA main.index_list('rdf_quad')", false)?;
    let mut partial = None;
    while st.step()? {
        if st.col_str(1)? == index {
            partial = Some(st.col_i64(4) != 0);
        }
    }
    drop(st);
    let Some(partial) = partial else { return Ok(None) };
    let term = order.sql_partial_term();
    let predicate_ok = if term.is_empty() {
        !partial
    } else {
        partial && row.sql.as_deref().is_some_and(|sql| sql.trim_end().ends_with(&format!(") WHERE {term}")))
    };
    Ok(predicate_ok.then_some(root))
}
