//! Row helpers shared by the operators: a flat row arena for materialized inputs (hash
//! tables, sorts, buffered join runs), in-place batch filtering, and the SPARQL
//! compatibility test / merge on full-width rows.
use sq_core::{Batch, Result, Slot, TermId};

/// Row-major arena of full-width rows (no per-row allocation).
#[derive(Clone, Debug, Default)]
pub(crate) struct Rows {
    width: usize,
    len: usize,
    data: Vec<TermId>,
}

impl Rows {
    pub fn new(width: usize) -> Self {
        Rows { width, len: 0, data: Vec::new() }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn row(&self, i: usize) -> &[TermId] {
        &self.data[i * self.width..(i + 1) * self.width]
    }

    #[inline]
    pub fn row_mut(&mut self, i: usize) -> &mut [TermId] {
        &mut self.data[i * self.width..(i + 1) * self.width]
    }

    #[inline]
    pub fn push(&mut self, r: &[TermId]) {
        debug_assert_eq!(r.len(), self.width);
        self.data.extend_from_slice(r);
        self.len += 1;
    }

    /// Appends every row of `b`.
    pub fn extend_from_batch(&mut self, b: &Batch) {
        for i in 0..b.len() {
            self.push(b.row(i));
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.len = 0;
    }

    /// Releases the storage (after the memory charge was released).
    pub fn reset(&mut self) {
        self.data = Vec::new();
        self.len = 0;
    }

    /// Bytes of one row.
    #[inline]
    pub fn row_bytes(&self) -> usize {
        self.width.max(1) * std::mem::size_of::<TermId>()
    }
}

/// Keeps the rows of `out` from index `start` on for which `keep` returns true (earlier rows
/// are untouched). The first error stops the evaluation and is returned.
pub(crate) fn retain_from(
    out: &mut Batch,
    start: usize,
    mut keep: impl FnMut(&[TermId]) -> Result<bool>,
) -> Result<()> {
    if start >= out.len() {
        return Ok(());
    }
    let mut i = 0usize;
    let mut err = None;
    out.retain(|row| {
        let idx = i;
        i += 1;
        if idx < start {
            return true;
        }
        if err.is_some() {
            return false;
        }
        match keep(row) {
            Ok(k) => k,
            Err(e) => {
                err = Some(e);
                false
            }
        }
    });
    err.map_or(Ok(()), Err)
}

/// SPARQL compatibility on `check`: every slot bound on both sides holds the same id.
#[inline]
pub(crate) fn compatible(a: &[TermId], b: &[TermId], check: &[Slot]) -> bool {
    check.iter().all(|&s| {
        let (x, y) = (a[s as usize], b[s as usize]);
        !x.is_bound() || !y.is_bound() || x == y
    })
}

/// Copies every bound slot of `src` into `dst` (the rows are known to be compatible).
#[inline]
pub(crate) fn merge_into(dst: &mut [TermId], src: &[TermId]) {
    for (d, s) in dst.iter_mut().zip(src) {
        if s.is_bound() {
            *d = *s;
        }
    }
}

/// Appends `left` merged with `right` to `out` and returns the new row's index.
#[inline]
pub(crate) fn push_merged(out: &mut Batch, left: &[TermId], right: &[TermId]) -> usize {
    out.push_row(left);
    let i = out.len() - 1;
    merge_into(out.row_mut(i), right);
    i
}

/// Pulls `op` into `b` (cleared first) until `b` is full or `op` is exhausted.
/// Returns false once `op` is exhausted (the batch may still hold its last rows).
pub(crate) fn refill(
    op: &mut dyn crate::ops::Operator,
    cx: &mut crate::ctx::ExecCtx,
    b: &mut Batch,
) -> Result<bool> {
    b.clear();
    op.next(cx, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(v: i64) -> TermId {
        TermId::from_int(v).unwrap()
    }

    #[test]
    fn retain_from_keeps_prefix_and_reports_errors() {
        let mut b = Batch::new(1);
        for v in 0..6 {
            b.push_row(&[t(v)]);
        }
        retain_from(&mut b, 2, |r| Ok(r[0].as_int().unwrap() % 2 == 1)).unwrap();
        let got: Vec<i64> = (0..b.len()).map(|i| b.get(i, 0).as_int().unwrap()).collect();
        assert_eq!(got, [0, 1, 3, 5]);
        let e = retain_from(&mut b, 0, |_| Err(sq_core::Error::Timeout));
        assert!(matches!(e, Err(sq_core::Error::Timeout)));
    }

    #[test]
    fn compat_and_merge() {
        let a = [t(1), TermId::UNBOUND, t(3)];
        let b = [t(1), t(2), TermId::UNBOUND];
        assert!(compatible(&a, &b, &[0, 1, 2]));
        assert!(!compatible(&a, &[t(9), t(2), t(3)], &[0]));
        let mut out = Batch::new(3);
        let i = push_merged(&mut out, &a, &b);
        assert_eq!(out.row(i), &[t(1), t(2), t(3)]);
    }

    #[test]
    fn zero_width_rows() {
        let mut r = Rows::new(0);
        r.push(&[]);
        r.push(&[]);
        assert_eq!(r.len(), 2);
        assert!(r.row(1).is_empty());
    }
}
