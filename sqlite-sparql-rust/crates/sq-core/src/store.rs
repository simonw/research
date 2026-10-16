//! Storage-access contract used by the planner and executor. Implemented by
//! `sq-memstore::MemStore` (oracle/test double), `sq-sqlite::SqliteStore` (SQL cursors) and,
//! in phase 2, `sq-btree` (direct page reader) behind the same `QuadCursor`.
use std::ops::Bound;
use std::sync::Arc;

use crate::error::Result;
use crate::id::TermId;
use crate::stats::{CostConstants, Stats};
use crate::term::Term;

/// Quad column. Quads are always written `[g, s, p, o]` (the primary-key order).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Col {
    G = 0,
    S = 1,
    P = 2,
    O = 3,
}

/// The nine physical orders. Every one is a COVERING B-tree of `rdf_quad` (WITHOUT ROWID).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum IndexOrder {
    /// Primary key (g,s,p,o).
    Gspo,
    /// rdf_gpso (g,p,s,o): s-sorted stream for every `?s :p ?o`.
    Gpso,
    /// rdf_gpos (g,p,o,s): o-sorted stream for `?s :p ?o`, s-sorted for `?s :p :o`.
    Gpos,
    /// rdf_gosp (g,o,s,p).
    Gosp,
    /// Partial `WHERE g<>0` (named graphs only), g last: GRAPH ?g / union-default merges.
    Spog,
    Psog,
    Posg,
    Ospg,
    /// rdf_gpox (g,p,o,s) partial `WHERE (o & 15) IN (5,8)`: the objects of p that are
    /// dictionary-typed literals or inline decimals ("numeric exceptions" of int range scans).
    Gpox,
}

impl IndexOrder {
    pub const ALL: [IndexOrder; 9] = [
        IndexOrder::Gspo,
        IndexOrder::Gpso,
        IndexOrder::Gpos,
        IndexOrder::Gosp,
        IndexOrder::Spog,
        IndexOrder::Psog,
        IndexOrder::Posg,
        IndexOrder::Ospg,
        IndexOrder::Gpox,
    ];
    pub const fn cols(self) -> [Col; 4] {
        use Col::*;
        match self {
            IndexOrder::Gspo => [G, S, P, O],
            IndexOrder::Gpso => [G, P, S, O],
            IndexOrder::Gpos | IndexOrder::Gpox => [G, P, O, S],
            IndexOrder::Gosp => [G, O, S, P],
            IndexOrder::Spog => [S, P, O, G],
            IndexOrder::Psog => [P, S, O, G],
            IndexOrder::Posg => [P, O, S, G],
            IndexOrder::Ospg => [O, S, P, G],
        }
    }
    /// Rows only from named graphs (g <> 0); g is the LAST column.
    pub const fn named_only(self) -> bool {
        matches!(self, IndexOrder::Spog | IndexOrder::Psog | IndexOrder::Posg | IndexOrder::Ospg)
    }
    /// Rows only where tag(o) is 5 (Typed) or 8 (Decimal).
    pub const fn numeric_exceptions(self) -> bool {
        matches!(self, IndexOrder::Gpox)
    }
    /// FROM clause used by the SQL cursor (all SQL is schema-qualified `main.`).
    pub const fn sql_from(self) -> &'static str {
        match self {
            // NOT INDEXED does not pin the PK of a WITHOUT ROWID table on SQLite 3.45 (the planner
            // still picks a secondary index); naming the PK's automatic index does.
            IndexOrder::Gspo => "main.rdf_quad INDEXED BY sqlite_autoindex_rdf_quad_1",
            IndexOrder::Gpso => "main.rdf_quad INDEXED BY rdf_gpso",
            IndexOrder::Gpos => "main.rdf_quad INDEXED BY rdf_gpos",
            IndexOrder::Gosp => "main.rdf_quad INDEXED BY rdf_gosp",
            IndexOrder::Spog => "main.rdf_quad INDEXED BY rdf_spog",
            IndexOrder::Psog => "main.rdf_quad INDEXED BY rdf_psog",
            IndexOrder::Posg => "main.rdf_quad INDEXED BY rdf_posg",
            IndexOrder::Ospg => "main.rdf_quad INDEXED BY rdf_ospg",
            IndexOrder::Gpox => "main.rdf_quad INDEXED BY rdf_gpox",
        }
    }
    /// Extra WHERE term that makes a partial index usable (must appear verbatim).
    pub const fn sql_partial_term(self) -> &'static str {
        if self.named_only() {
            "g<>0"
        } else if self.numeric_exceptions() {
            "(o & 15) IN (5,8)"
        } else {
            ""
        }
    }
    /// Orders (excluding Gpox) whose first `bound.len()` columns are exactly the bound
    /// columns. `bound[c]` = column c (G,S,P,O) is bound. If G is unbound, only named-only
    /// orders qualify (GRAPH ?g); if G is bound, only graph-first orders qualify.
    pub fn candidates(bound: [bool; 4]) -> Vec<(IndexOrder, u8)> {
        let k = bound.iter().filter(|b| **b).count();
        IndexOrder::ALL
            .iter()
            .copied()
            .filter(|o| !o.numeric_exceptions() && o.named_only() != bound[0])
            .filter(|o| o.cols()[..k].iter().all(|c| bound[*c as usize]))
            .map(|o| (o, k as u8))
            .collect()
    }
}

/// A range scan: fixed key prefix (in index order) + optional bounds on the next column.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ScanSpec {
    pub order: IndexOrder,
    /// Values of the first `prefix_len` columns of `order.cols()`.
    pub prefix: [TermId; 4],
    pub prefix_len: u8,
    /// Bounds on column `order.cols()[prefix_len]` (ignored when prefix_len == 4).
    pub lower: Bound<i64>,
    pub upper: Bound<i64>,
}

impl ScanSpec {
    pub fn new(order: IndexOrder, prefix: &[TermId]) -> Self {
        let mut p = [TermId::UNBOUND; 4];
        p[..prefix.len()].copy_from_slice(prefix);
        ScanSpec { order, prefix: p, prefix_len: prefix.len() as u8, lower: Bound::Unbounded, upper: Bound::Unbounded }
    }
    pub fn with_range(mut self, lower: Bound<i64>, upper: Bound<i64>) -> Self {
        self.lower = lower;
        self.upper = upper;
        self
    }
    /// Free (output) columns, in index order.
    pub fn free_cols(&self) -> &'static [Col] {
        let cols: &'static [Col; 4] = match self.order {
            IndexOrder::Gspo => &[Col::G, Col::S, Col::P, Col::O],
            IndexOrder::Gpso => &[Col::G, Col::P, Col::S, Col::O],
            IndexOrder::Gpos | IndexOrder::Gpox => &[Col::G, Col::P, Col::O, Col::S],
            IndexOrder::Gosp => &[Col::G, Col::O, Col::S, Col::P],
            IndexOrder::Spog => &[Col::S, Col::P, Col::O, Col::G],
            IndexOrder::Psog => &[Col::P, Col::S, Col::O, Col::G],
            IndexOrder::Posg => &[Col::P, Col::O, Col::S, Col::G],
            IndexOrder::Ospg => &[Col::O, Col::S, Col::P, Col::G],
        };
        &cols[self.prefix_len as usize..]
    }
    /// Does a full quad [g,s,p,o] match this spec (used by MemStore and the conformance suite)?
    pub fn matches(&self, q: &[TermId; 4]) -> bool {
        let cols = self.order.cols();
        for i in 0..self.prefix_len as usize {
            if q[cols[i] as usize] != self.prefix[i] {
                return false;
            }
        }
        if self.order.named_only() && q[0] == TermId::DEFAULT_GRAPH {
            return false;
        }
        if self.order.numeric_exceptions() && !matches!(q[3].0 & 15, 5 | 8) {
            return false;
        }
        if (self.prefix_len as usize) < 4 {
            let v = q[cols[self.prefix_len as usize] as usize].0;
            let lo_ok = match self.lower {
                Bound::Included(l) => v >= l,
                Bound::Excluded(l) => v > l,
                Bound::Unbounded => true,
            };
            let hi_ok = match self.upper {
                Bound::Included(h) => v <= h,
                Bound::Excluded(h) => v < h,
                Bound::Unbounded => true,
            };
            return lo_ok && hi_ok;
        }
        true
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ReaderKind {
    #[default]
    Memory,
    SqlCursor,
    Btree,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct CursorCounters {
    pub rows: u64,
    pub seeks: u64,
    pub fills: u64,
    pub reader: ReaderKind,
}

/// A positioned range scan over one index.
///
/// Rows are the FREE SUFFIX columns of the spec (`width() == 4 - prefix_len`), appended
/// row-major, in ascending lexicographic SIGNED-i64 order, each row exactly once.
pub trait QuadCursor {
    fn width(&self) -> usize;
    /// Append up to `max_rows` rows to `out`. Returns the number appended; 0 = exhausted
    /// (a later `seek`/`rebind` restarts it).
    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize>;
    /// Reposition so that the next `fill` returns exactly the rows r with
    /// `suffix(r) >= key` (lexicographic over the first key.len() free columns,
    /// 1 <= key.len() <= width) that also satisfy the spec's bounds. Forward seeks are the
    /// fast path (finger/galloping); backward seeks are legal but may restart from the root.
    fn seek(&mut self, key: &[TermId]) -> Result<()>;
    /// Cheap retarget for index-nested-loop probes: same order and prefix_len, new prefix
    /// values and bounds; positions at the start. Must not allocate per call.
    fn rebind(&mut self, prefix: &[TermId], lower: Bound<i64>, upper: Bound<i64>) -> Result<()>;
    fn counters(&self) -> CursorCounters;
}

/// Term dictionary access (read side).
pub trait Dictionary {
    /// Canonical id of an EXISTING term: inline-encodable terms are always Some (no I/O);
    /// store blank-node labels "b<hex>" map to their id; never inserts. None = not in store.
    fn lookup(&self, t: oxrdf::TermRef<'_>) -> Result<Option<TermId>>;
    /// Decode storable ids (dictionary, bnode, inline); appends one entry per input id, in
    /// order. TEMP/UNBOUND/ERROR ids are `Error::Internal`. Implementations sort+dedupe misses
    /// and use the connection cache.
    fn decode(&self, ids: &[TermId], out: &mut Vec<Arc<Term>>) -> Result<()>;
    fn decode_one(&self, id: TermId) -> Result<Arc<Term>> {
        let mut v = Vec::with_capacity(1);
        self.decode(&[id], &mut v)?;
        v.pop().ok_or_else(|| crate::Error::internal("decode returned nothing"))
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct StoreCaps {
    /// `sql_rows` is implemented (SqlJoin hybrid / CountScan in VDBE).
    pub sql: bool,
    pub reader: ReaderKind,
}

/// Object-safe. Executions hold `Rc<dyn QuadStore>`; cursors are `'static` (they own their
/// leased statement / snapshot), so a streaming execution can own its operator tree.
pub trait QuadStore {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>>;
    /// Number of rows of `spec`: exact when `cap` is None; otherwise `min(count, cap + 1)`.
    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64>;
    fn dict(&self) -> &dyn Dictionary;
    /// All named graphs: registry (`rdf_graph`) UNION distinct g <> 0 in rdf_quad; sorted.
    fn named_graphs(&self) -> Result<Vec<TermId>>;
    /// None = never analyzed. Statistics are HINTS: correctness never depends on them.
    fn stats(&self) -> Option<Arc<Stats>>;
    fn costs(&self) -> &CostConstants;
    fn caps(&self) -> StoreCaps;
    /// Run a generated read-only SQL statement whose result columns are `width` INTEGER
    /// TermIds (SqlJoin hybrid). Only when `caps().sql`. seek/rebind are unsupported on it.
    fn sql_rows(&self, _sql: &str, _params: &[i64], _width: usize) -> Result<Box<dyn QuadCursor>> {
        Err(crate::Error::Unsupported("sql_rows".into()))
    }
    /// Polled by CPU-bound loops every 4096 rows; Err(Interrupted) after sqlite3_interrupt.
    fn check_interrupt(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GraphTarget {
    Default,
    Named(TermId),
    AllNamed,
    All,
}

/// Write side. Atomicity is provided by the CALLER (sq-api wraps every request in a
/// SAVEPOINT write scope); implementations never commit.
pub trait QuadStoreWriter: QuadStore {
    /// Canonical id, inserting a dictionary row if needed. Blank nodes are `Error::Invalid`
    /// (callers map labels to `fresh_bnode()` per load / per update operation).
    fn intern(&self, t: oxrdf::TermRef<'_>) -> Result<TermId>;
    fn fresh_bnode(&self) -> Result<TermId>;
    /// Quads are [g,s,p,o]; sorts + dedups in place; rejects non-storable ids
    /// (`Error::Internal`); registers new named graphs in `rdf_graph`. Returns #new quads.
    fn insert(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64>;
    /// Returns #removed quads.
    fn remove(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64>;
    fn clear(&self, target: GraphTarget) -> Result<u64>;
    /// Registers an (empty) named graph; Ok(false) if it already existed.
    fn create_graph(&self, g: TermId) -> Result<bool>;
    /// Removes a graph from the registry (quads must already be cleared); Ok(false) if absent.
    fn drop_graph(&self, g: TermId) -> Result<bool>;
}
