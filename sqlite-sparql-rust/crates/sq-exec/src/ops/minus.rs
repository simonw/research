//! SPARQL MINUS (§5.3): the right side is evaluated independently (no Join-mode seed) and
//! hashed on `keys`; a left row is removed iff some right row is compatible with it **and**
//! shares at least one bound slot with it. Domains are the actually bound slots of both rows;
//! inside an EXISTS evaluation the substituted slots are constants and are excluded from the
//! domain intersection.
use sq_core::{Batch, Result, Slot};

use super::hash_join::HashTable;
use super::{BoxOp, Operator};
use crate::ctx::ExecCtx;
use crate::rows::retain_from;
use crate::seed::{Seed, SeedBuf};

pub(crate) struct Minus {
    left: BoxOp,
    right: BoxOp,
    table: HashTable,
    /// keys ∪ check: the slots both sides may bind.
    shared: Vec<Slot>,
    /// `shared` minus the substituted slots of the current open.
    domain: Vec<Slot>,
    seed: SeedBuf,
    built: bool,
    buf: Batch,
}

impl Minus {
    pub fn new(left: BoxOp, right: BoxOp, keys: Vec<Slot>, check: Vec<Slot>, width: usize) -> Self {
        let mut shared: Vec<Slot> = keys.iter().chain(&check).copied().collect();
        shared.sort_unstable();
        shared.dedup();
        Minus {
            left,
            right,
            table: HashTable::new(width, keys, check),
            shared,
            domain: Vec::new(),
            seed: SeedBuf::new(width),
            built: false,
            buf: Batch::new(width),
        }
    }
}

impl Operator for Minus {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.domain.clear();
        self.domain.extend(self.shared.iter().copied().filter(|s| !self.seed.is_subst(*s)));
        self.built = false;
        self.left.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if !self.built {
            self.table.clear(cx);
            self.right.open(cx, self.seed.subst_only())?;
            self.table.build_from(cx, self.right.as_mut(), &mut self.buf)?;
            self.right.close(cx);
            self.built = true;
        }
        loop {
            if out.is_full() {
                return Ok(true);
            }
            let start = out.len();
            let more = self.left.next(cx, out)?;
            if self.table.len() > 0 && !self.domain.is_empty() {
                let (table, domain) = (&self.table, &self.domain);
                retain_from(out, start, |row| {
                    let mut cur = table.start(row);
                    while let Some(b) = table.next_match(row, &mut cur) {
                        let r = table.rows.row(b as usize);
                        if domain.iter().any(|s| row[*s as usize].is_bound() && r[*s as usize].is_bound()) {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                })?;
            }
            if !more {
                return Ok(false);
            }
        }
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.table.reset(cx);
        self.built = false;
        self.left.close(cx);
        self.right.close(cx);
    }
}
