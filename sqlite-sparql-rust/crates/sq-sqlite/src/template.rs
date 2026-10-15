//! SQL text of every pooled statement: the §5.2 scan / count templates (keyed by `StmtKey`)
//! and the fixed dictionary, writer, epoch and transaction statements.
use std::ffi::c_int;
use std::fmt::Write as _;
use std::ops::Bound;
use std::sync::OnceLock;

use sq_core::store::{Col, IndexOrder, ScanSpec};

/// Operator of the always-present lower bound on the first free column.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum LowerOp {
    /// `c >= ?L` (also used with `?L = i64::MIN` when the spec is unbounded).
    Ge,
    /// `c > ?L`.
    Gt,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum UpperOp {
    None,
    /// `c <= ?U`.
    Le,
    /// `c < ?U`.
    Lt,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum StmtKind {
    /// Rows of the free suffix columns, in index order.
    Scan,
    /// Exact `count(*)`.
    Count,
    /// `count(*)` over a `LIMIT ?cap+1` sub-select.
    CountCapped,
}

/// Shape of a §5.2 template. Every `ScanSpec` maps to exactly one key per kind; seeks with
/// `m >= 2` key columns use the `seek_cols = m` variant of the scan key.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct StmtKey {
    pub order: IndexOrder,
    pub prefix_len: u8,
    pub lower: LowerOp,
    pub upper: UpperOp,
    /// 0, or 2..=width for the row-value resume variant.
    pub seek_cols: u8,
    pub kind: StmtKind,
}

/// Parameter positions (1-based, 0 = absent) of a rendered template.
#[derive(Copy, Clone, Debug, Default)]
pub struct Params {
    pub lower: c_int,
    pub upper: c_int,
    pub seek: c_int,
    pub limit: c_int,
}

pub(crate) const COL_NAMES: [&str; 4] = ["g", "s", "p", "o"];

/// FROM clause pinning the primary key (GSPO) of the WITHOUT ROWID quad table by its
/// automatic index name. `NOT INDEXED` (what `IndexOrder::sql_from` says) does not pin it on
/// SQLite 3.45: without an ORDER BY the planner answers from any covering index, and with
/// `g=? AND s=? AND p=?` bound it even serves `ORDER BY g,s,p,o` from rdf_gpso.
pub(crate) const PK_FROM: &str = "main.rdf_quad INDEXED BY sqlite_autoindex_rdf_quad_1";

/// The FROM clause of an order's templates.
pub(crate) fn from_clause(order: IndexOrder) -> &'static str {
    if order == IndexOrder::Gspo { PK_FROM } else { order.sql_from() }
}

fn col_name(c: Col) -> &'static str {
    COL_NAMES[c as usize]
}

impl StmtKey {
    /// The scan / count key of a spec (bounds are ignored when the prefix is complete).
    pub fn for_spec(spec: &ScanSpec, kind: StmtKind) -> StmtKey {
        let full = spec.prefix_len >= 4;
        StmtKey {
            order: spec.order,
            prefix_len: spec.prefix_len.min(4),
            lower: if !full && matches!(spec.lower, Bound::Excluded(_)) { LowerOp::Gt } else { LowerOp::Ge },
            upper: match spec.upper {
                _ if full => UpperOp::None,
                Bound::Unbounded => UpperOp::None,
                Bound::Included(_) => UpperOp::Le,
                Bound::Excluded(_) => UpperOp::Lt,
            },
            seek_cols: 0,
            kind,
        }
    }

    /// The row-value resume variant (`m >= 2` key columns). The lower bound is folded into
    /// the bound row value, because SQLite 3.45 would otherwise drive the range from the
    /// single-column `c >= ?L` term and scan from the range start (verified with EQP).
    pub fn seek_variant(self, m: u8) -> StmtKey {
        debug_assert!(m >= 2 && (m as usize) <= self.width());
        StmtKey { seek_cols: m, lower: LowerOp::Ge, kind: StmtKind::Scan, ..self }
    }

    pub fn width(&self) -> usize {
        4 - self.prefix_len as usize
    }

    pub fn params(&self) -> Params {
        let k = self.prefix_len as c_int;
        let mut next = k + 1;
        let mut p = Params::default();
        if k < 4 {
            if self.seek_cols == 0 {
                p.lower = next;
                next += 1;
            }
            if self.upper != UpperOp::None {
                p.upper = next;
                next += 1;
            }
            if self.seek_cols >= 2 {
                p.seek = next;
                next += self.seek_cols as c_int;
            }
        }
        if self.kind == StmtKind::CountCapped {
            p.limit = next;
        }
        p
    }

    /// Renders the template:
    /// ```text
    /// SELECT <free cols> FROM <from_clause(order)>
    ///  WHERE c0=?1 AND … [AND <partial term>] AND c(k) {>=|>} ?L [AND c(k) {<=|<} ?U]
    ///        [AND (c(k),…) >= (?,…)]           -- seek_cols >= 2 (replaces the ?L term)
    ///  ORDER BY <all 4 cols, index order>      -- satisfied by the index: never a sorter
    /// ```
    pub fn sql(&self) -> String {
        let cols = self.order.cols();
        let k = self.prefix_len as usize;
        let p = self.params();
        let mut terms: Vec<String> = (0..k).map(|i| format!("{}=?{}", col_name(cols[i]), i + 1)).collect();
        let partial = self.order.sql_partial_term();
        if !partial.is_empty() {
            terms.push(partial.to_string());
        }
        if k < 4 {
            let c = col_name(cols[k]);
            if p.lower != 0 {
                let op = if self.lower == LowerOp::Gt { ">" } else { ">=" };
                terms.push(format!("{c}{op}?{}", p.lower));
            }
            if p.upper != 0 {
                let op = if self.upper == UpperOp::Lt { "<" } else { "<=" };
                terms.push(format!("{c}{op}?{}", p.upper));
            }
            if p.seek != 0 {
                let m = self.seek_cols as usize;
                let lhs: Vec<&str> = cols[k..k + m].iter().map(|c| col_name(*c)).collect();
                let rhs: Vec<String> = (0..m).map(|i| format!("?{}", p.seek + i as c_int)).collect();
                terms.push(format!("({})>=({})", lhs.join(","), rhs.join(",")));
            }
        }
        let from = from_clause(self.order);
        let wh = terms.join(" AND ");
        match self.kind {
            StmtKind::Scan => {
                let free: Vec<&str> = cols[k..].iter().map(|c| col_name(*c)).collect();
                let select = if free.is_empty() { "1".to_string() } else { free.join(",") };
                let order: Vec<&str> = cols.iter().map(|c| col_name(*c)).collect();
                let mut s = format!("SELECT {select} FROM {from} WHERE {wh}");
                if k < 4 {
                    let _ = write!(s, " ORDER BY {}", order.join(","));
                }
                s
            }
            StmtKind::Count => format!("SELECT count(*) FROM {from} WHERE {wh}"),
            StmtKind::CountCapped => format!("SELECT count(*) FROM (SELECT 1 FROM {from} WHERE {wh} LIMIT ?{})", p.limit),
        }
    }

    /// Every key the store can reach (the EXPLAIN QUERY PLAN test iterates over these).
    #[cfg(test)]
    pub fn all_reachable() -> Vec<StmtKey> {
        let mut out = Vec::new();
        for order in IndexOrder::ALL {
            for prefix_len in 0..=4u8 {
                let bounds: &[(LowerOp, UpperOp)] = if prefix_len == 4 {
                    &[(LowerOp::Ge, UpperOp::None)]
                } else {
                    &[
                        (LowerOp::Ge, UpperOp::None),
                        (LowerOp::Ge, UpperOp::Le),
                        (LowerOp::Ge, UpperOp::Lt),
                        (LowerOp::Gt, UpperOp::None),
                        (LowerOp::Gt, UpperOp::Le),
                        (LowerOp::Gt, UpperOp::Lt),
                    ]
                };
                for &(lower, upper) in bounds {
                    for kind in [StmtKind::Scan, StmtKind::Count, StmtKind::CountCapped] {
                        let key = StmtKey { order, prefix_len, lower, upper, seek_cols: 0, kind };
                        out.push(key);
                        if kind == StmtKind::Scan && lower == LowerOp::Ge {
                            for m in 2..=key.width() as u8 {
                                out.push(key.seek_variant(m));
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

/// Savepoint statements (pooled per nesting depth, §8.6 rule R3).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TxnOp {
    SavepointRead,
    ReleaseRead,
    RollbackRead,
    SavepointWrite,
    ReleaseWrite,
    RollbackWrite,
}

impl TxnOp {
    pub fn sql(self, depth: u32) -> String {
        match self {
            TxnOp::SavepointRead => format!("SAVEPOINT sq_r{depth}"),
            TxnOp::ReleaseRead => format!("RELEASE sq_r{depth}"),
            TxnOp::RollbackRead => format!("ROLLBACK TO sq_r{depth}"),
            TxnOp::SavepointWrite => format!("SAVEPOINT sq_w{depth}"),
            TxnOp::ReleaseWrite => format!("RELEASE sq_w{depth}"),
            TxnOp::RollbackWrite => format!("ROLLBACK TO sq_w{depth}"),
        }
    }
}

/// Fixed statements used by the dictionary, writer, epoch check and maintenance code.
/// Internal SQL touches only `rdf_*` tables, `sqlite_schema` and `pragma_*` (rule R2).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Fixed {
    SchemaCookie,
    Epoch,
    Select1,
    TermByHash,
    TermById,
    TermScanFrom,
    TermMaxRow,
    TermInsert,
    QuadInsert,
    QuadDelete,
    GraphRegister,
    GraphUnregister,
    GraphRegistered,
    GraphHasQuads,
    NextGraph,
    Registry,
    MetaGet,
    MetaSet,
    MetaBump,
    ClearDefault,
    ClearGraph,
    ClearNamed,
    ClearAll,
    QuadCountCapped,
    MetaAdd,
    /// Plain `ROLLBACK`: ends a transaction that a scope's savepoint started.
    Rollback,
    /// A statement SQLite treats as a possible writer that changes nothing: `DROP TABLE IF
    /// EXISTS` of a name nobody can create (`sqlite_` is reserved) compiles to
    /// `OP_JournalMode` (query; clears `readOnly`) plus a read `OP_Transaction`, against any
    /// schema, and unlike a PRAGMA it does not expire itself. Stepped while an interrupt is
    /// pending, it makes SQLite roll back the whole transaction (see `scope`).
    Abort,
    /// `QuadInsert` for `QUAD_INSERT_BATCH` rows at once (bulk paths).
    QuadInsertMany,
    /// `TermInsert` for `TERM_INSERT_BATCH` rows at once (loader).
    TermInsertMany,
}

/// Rows per multi-row INSERT (4 parameters each).
pub(crate) const QUAD_INSERT_BATCH: usize = 64;
/// Rows per multi-row dictionary INSERT (6 parameters each).
pub(crate) const TERM_INSERT_BATCH: usize = 32;

/// `INSERT … VALUES (?,…),(?,…),…` with numbered parameters, built once.
fn multi_row_insert(head: &str, cols: usize, rows: usize) -> String {
    let mut s = String::from(head);
    for r in 0..rows {
        s.push_str(if r == 0 { " VALUES (" } else { ",(" });
        for c in 0..cols {
            if c > 0 {
                s.push(',');
            }
            let _ = write!(s, "?{}", r * cols + c + 1);
        }
        s.push(')');
    }
    s
}

impl Fixed {
    pub fn sql(self) -> &'static str {
        static QUAD_MANY: OnceLock<String> = OnceLock::new();
        static TERM_MANY: OnceLock<String> = OnceLock::new();
        match self {
            Fixed::QuadInsertMany => QUAD_MANY
                .get_or_init(|| multi_row_insert("INSERT OR IGNORE INTO main.rdf_quad(g,s,p,o)", 4, QUAD_INSERT_BATCH)),
            Fixed::TermInsertMany => TERM_MANY
                .get_or_init(|| multi_row_insert("INSERT INTO main.rdf_term(id,tag,lex,dt,lang,h)", 6, TERM_INSERT_BATCH)),
            Fixed::SchemaCookie => "SELECT schema_version FROM pragma_schema_version",
            // §4.4 epoch. The SQLite schema cookie is read by SchemaCookie first, so an absent
            // schema never makes this statement fail to prepare; the version rows are
            // re-checked because UPDATEs of rdf_meta do not change the cookie.
            Fixed::Epoch => {
                "SELECT (SELECT value FROM main.rdf_meta WHERE key='dict_gen'), \
                 (SELECT value FROM main.rdf_meta WHERE key='stats_gen'), \
                 (SELECT max(id) FROM main.rdf_term), \
                 (SELECT value FROM main.rdf_meta WHERE key='schema_version'), \
                 (SELECT value FROM main.rdf_meta WHERE key='encoding_version')"
            }
            Fixed::Select1 => "SELECT 1",
            Fixed::TermByHash => "SELECT id, tag, lex, dt, lang FROM main.rdf_term INDEXED BY rdf_term_h WHERE h=?1",
            Fixed::TermById => "SELECT tag, lex, dt, lang FROM main.rdf_term WHERE id=?1",
            Fixed::TermScanFrom => "SELECT id, tag, lex, dt, lang FROM main.rdf_term WHERE id>=?1 ORDER BY id",
            Fixed::TermMaxRow => "SELECT max(id) FROM main.rdf_term",
            Fixed::TermInsert => "INSERT INTO main.rdf_term(id,tag,lex,dt,lang,h) VALUES (?1,?2,?3,?4,?5,?6)",
            Fixed::QuadInsert => "INSERT OR IGNORE INTO main.rdf_quad(g,s,p,o) VALUES (?1,?2,?3,?4)",
            Fixed::QuadDelete => "DELETE FROM main.rdf_quad WHERE g=?1 AND s=?2 AND p=?3 AND o=?4",
            Fixed::GraphRegister => "INSERT OR IGNORE INTO main.rdf_graph(g) VALUES (?1)",
            Fixed::GraphUnregister => "DELETE FROM main.rdf_graph WHERE g=?1",
            Fixed::GraphRegistered => "SELECT 1 FROM main.rdf_graph WHERE g=?1",
            Fixed::GraphHasQuads => "SELECT 1 FROM main.rdf_quad INDEXED BY sqlite_autoindex_rdf_quad_1 WHERE g=?1 LIMIT 1",
            // Loose index scan over the g column of the primary key.
            Fixed::NextGraph => {
                "SELECT g FROM main.rdf_quad INDEXED BY sqlite_autoindex_rdf_quad_1 WHERE g>=?1 ORDER BY g LIMIT 1"
            }
            Fixed::Registry => "SELECT g FROM main.rdf_graph ORDER BY g",
            Fixed::MetaGet => "SELECT value FROM main.rdf_meta WHERE key=?1",
            Fixed::MetaSet => "UPDATE main.rdf_meta SET value=?2 WHERE key=?1",
            Fixed::MetaBump => "UPDATE main.rdf_meta SET value=value+1 WHERE key=?1",
            Fixed::ClearDefault => "DELETE FROM main.rdf_quad WHERE g=0",
            Fixed::ClearGraph => "DELETE FROM main.rdf_quad WHERE g=?1",
            Fixed::ClearNamed => "DELETE FROM main.rdf_quad WHERE g<>0",
            Fixed::ClearAll => "DELETE FROM main.rdf_quad",
            Fixed::QuadCountCapped => "SELECT count(*) FROM (SELECT 1 FROM main.rdf_quad LIMIT ?1)",
            // Upsert: counters that §2.1's defaults do not create (`loaded_since_analyze`).
            Fixed::MetaAdd => {
                "INSERT INTO main.rdf_meta(key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value=value+excluded.value"
            }
            Fixed::Rollback => "ROLLBACK",
            Fixed::Abort => "DROP TABLE IF EXISTS main.sqlite_sparqlite_abort",
        }
    }
}
