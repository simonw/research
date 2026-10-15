//! The three virtual-table modules (§8.3) and the batch streaming they share.
//!
//! * `sparql_rows` ([`rows`]): eponymous table-valued function over any query;
//! * `sparql_view` ([`view`]): `CREATE VIRTUAL TABLE … USING sparql_view('SELECT …')` with
//!   typed columns and pushed-down `__term` / `__id` equality constraints;
//! * `sparql_status` ([`status`]): eponymous status table and the connection's close hook.
//!
//! Every method body runs inside `guard()` (rule R4). A scan's `xFilter` is one read entry
//! that plans the query and fetches the first batch; later batches read the same snapshot
//! (see `state`). `timeout_ms` counts wall-clock time from `xFilter`, including the time the
//! consumer spends between rows.
use std::sync::Arc;

use rusqlite::Connection;
use rusqlite::vtab::Module;
use sq_core::store::Dictionary;
use sq_core::{BATCH_ROWS, Result, Term, TermId};

use crate::query::Run;
use crate::state::ConnState;

mod rows;
mod status;
mod view;

pub(crate) fn register(conn: &Connection, st: &Arc<ConnState>) -> rusqlite::Result<()> {
    conn.create_module(
        "sparql_status",
        const { &Module::<status::StatusTab>::eponymous_only_module() },
        Some(st.clone()),
    )?;
    conn.create_module("sparql_rows", const { &Module::<rows::RowsTab>::eponymous_only_module() }, Some(st.clone()))?;
    conn.create_module("sparql_view", const { &Module::<view::ViewTab>::read_only_module() }, Some(st.clone()))?;
    Ok(())
}

/// Whether column `col` is in SQLite's `colUsed` mask (bit 63 stands for every column ≥ 63).
pub(crate) fn col_used(mask: u64, col: usize) -> bool {
    mask & (1u64 << col.min(63)) != 0
}

/// Where a scan's rows come from.
pub(crate) enum Source {
    /// SELECT solutions.
    Solutions(Run),
    /// CONSTRUCT / DESCRIBE triples (subject, predicate, object).
    Triples(Run),
    /// ASK: one row holding the boolean (taken when emitted).
    Ask(Option<bool>),
    /// A scan known to be empty without running anything.
    Empty,
}

impl Source {
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        match self {
            Source::Solutions(run) | Source::Triples(run) => run.decode(ids, out),
            // Only the inline boolean of an ASK row.
            Source::Ask(_) | Source::Empty => {
                out.extend(ids.iter().map(|id| sq_core::codec::decode_inline(*id).map(Arc::new)));
                Ok(())
            }
        }
    }
}

/// A scan's position: the current 1024-row batch of ids plus the decoded terms of exactly
/// the columns the statement reads (late materialization, §4.2).
pub(crate) struct Stream {
    source: Source,
    /// Ids per row.
    width: usize,
    /// Columns whose terms are decoded with each batch.
    decode: Vec<usize>,
    ids: Vec<TermId>,
    /// `rows × width` slots when `decode` is not empty (undecoded columns stay None).
    terms: Vec<Option<Arc<Term>>>,
    rows: usize,
    pos: usize,
    /// 1-based ordinal of the current row.
    ordinal: i64,
    eof: bool,
    triples: Vec<[TermId; 3]>,
    want: Vec<TermId>,
    got: Vec<Option<Arc<Term>>>,
}

impl Stream {
    /// Starts the scan and positions it on the first row.
    pub(crate) fn open(st: &ConnState, source: Source, width: usize, decode: Vec<usize>) -> Result<Stream> {
        let mut s = Stream {
            source,
            width,
            decode,
            ids: Vec::new(),
            terms: Vec::new(),
            rows: 0,
            pos: 0,
            ordinal: 1,
            eof: false,
            triples: Vec::new(),
            want: Vec::new(),
            got: Vec::new(),
        };
        s.fetch(st)?;
        Ok(s)
    }

    pub(crate) fn eof(&self) -> bool {
        self.eof
    }

    pub(crate) fn ordinal(&self) -> i64 {
        self.ordinal
    }

    pub(crate) fn advance(&mut self, st: &ConnState) -> Result<()> {
        if self.eof {
            return Ok(());
        }
        self.pos += 1;
        self.ordinal += 1;
        if self.pos >= self.rows {
            self.fetch(st)?;
        }
        Ok(())
    }

    /// Id of column `col` of the current row (UNBOUND past the row's width).
    pub(crate) fn id(&self, col: usize) -> TermId {
        if col >= self.width || self.eof {
            return TermId::UNBOUND;
        }
        self.ids[self.pos * self.width + col]
    }

    /// The `id*` / `__id` value of column `col` of the current row: the term's id in the
    /// store, NULL when unbound or absent from the store. A term the execution computed has
    /// an execution-local TEMP id; when the dictionary holds the same term, its stored id is
    /// reported, so the column depends only on the term — as `__id` pushdown (a `sameTerm`
    /// filter) does.
    pub(crate) fn public_id(&mut self, st: &ConnState, col: usize) -> Result<Option<i64>> {
        let id = self.id(col);
        if !id.is_temp() {
            return Ok(crate::value::public_id(id));
        }
        let Some(term) = self.term(st, col)? else { return Ok(None) };
        Ok(st.store().lookup(crate::helpers::term_ref(&term))?.and_then(crate::value::public_id))
    }

    /// Term of column `col` of the current row; decoded on demand (and kept for the row)
    /// when the column was not announced in `colUsed`.
    pub(crate) fn term(&mut self, st: &ConnState, col: usize) -> Result<Option<Arc<Term>>> {
        let id = self.id(col);
        if !id.is_bound() || id == TermId::ERROR {
            return Ok(None);
        }
        let at = self.pos * self.width + col;
        if self.terms.len() < self.rows * self.width {
            self.terms.resize(self.rows * self.width, None);
        }
        if self.terms[at].is_none() {
            self.got.clear();
            self.source.decode(&[id], &mut self.got)?;
            st.bump(|c| c.terms_decoded += 1);
            self.terms[at] = self.got.pop().flatten();
        }
        Ok(self.terms[at].clone())
    }

    /// Every term of the current row (the `json` column).
    pub(crate) fn row_terms(&mut self, st: &ConnState) -> Result<&[Option<Arc<Term>>]> {
        if self.eof {
            return Ok(&[]);
        }
        if self.terms.len() < self.rows * self.width {
            self.terms.resize(self.rows * self.width, None);
        }
        for c in 0..self.width {
            self.term(st, c)?;
        }
        let at = self.pos * self.width;
        Ok(&self.terms[at..at + self.width])
    }

    fn fetch(&mut self, st: &ConnState) -> Result<()> {
        self.ids.clear();
        self.terms.clear();
        self.pos = 0;
        self.rows = 0;
        let n = match &mut self.source {
            Source::Solutions(run) => run.solutions().next_rows(&mut self.ids, BATCH_ROWS)?,
            Source::Triples(run) => {
                self.triples.clear();
                let n = run.triples()?.next_triples(&mut self.triples, BATCH_ROWS)?;
                self.ids.extend(self.triples.iter().flatten());
                n
            }
            Source::Ask(answer) => match answer.take() {
                Some(b) => {
                    self.ids.push(TermId::from_bool(b));
                    1
                }
                None => 0,
            },
            Source::Empty => 0,
        };
        if n == 0 {
            self.eof = true;
            return Ok(());
        }
        self.rows = n;
        st.bump(|c| c.rows_emitted += n as u64);
        if self.decode.is_empty() {
            return Ok(());
        }
        // One batched decode for every needed (row, column) of the batch.
        self.want.clear();
        for r in 0..n {
            for &c in &self.decode {
                let id = self.ids[r * self.width + c];
                if id.is_bound() && id != TermId::ERROR {
                    self.want.push(id);
                }
            }
        }
        self.got.clear();
        self.source.decode(&self.want, &mut self.got)?;
        st.bump(|c| c.terms_decoded += self.want.len() as u64);
        self.terms.resize(n * self.width, None);
        let mut got = self.got.drain(..);
        for r in 0..n {
            for &c in &self.decode {
                let at = r * self.width + c;
                let id = self.ids[at];
                if id.is_bound() && id != TermId::ERROR {
                    self.terms[at] = got.next().flatten();
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn col_used_bits() {
        assert!(col_used(1 << 5, 5));
        assert!(!col_used(1 << 5, 6));
        assert!(col_used(1 << 63, 63));
        assert!(col_used(1 << 63, 200), "bit 63 covers every later column");
        assert!(!col_used(u64::MAX >> 1, 70));
    }
}
