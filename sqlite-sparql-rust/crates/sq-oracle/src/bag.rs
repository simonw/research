//! Multisets of solution mappings (SPARQL 1.1 §18.1.6) in the query-wide slot layout.
//!
//! A solution mapping μ is a full-width row of term ids; `TermId::UNBOUND` marks a slot outside
//! dom(μ). Because every id the oracle produces is canonical (sameTerm ⇔ id equality), the
//! compatibility test and merge of §18.3 are plain id comparisons.
use sq_core::{Slot, TermId};

/// A bag of solutions, row-major. Rows keep their insertion order, which is the solution
/// sequence order wherever the algebra defines one (ORDER BY, Slice).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Bag {
    width: usize,
    len: usize,
    data: Vec<TermId>,
}

impl Bag {
    pub(crate) fn new(width: usize) -> Self {
        Bag { width, len: 0, data: Vec::new() }
    }

    /// Ω containing exactly one solution.
    pub(crate) fn single(row: &[TermId]) -> Self {
        let mut b = Bag::new(row.len());
        b.push(row);
        b
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(crate) fn row(&self, i: usize) -> &[TermId] {
        &self.data[i * self.width..(i + 1) * self.width]
    }

    /// Rows in order (also for width 0, where every row is the empty mapping).
    pub(crate) fn rows(&self) -> impl ExactSizeIterator<Item = &[TermId]> + '_ {
        (0..self.len).map(move |i| self.row(i))
    }

    pub(crate) fn push(&mut self, row: &[TermId]) {
        debug_assert_eq!(row.len(), self.width, "row width");
        self.data.extend_from_slice(row);
        self.len += 1;
    }

    /// Appends every row of `other` (multiset union, §18.5 Union).
    pub(crate) fn append(&mut self, other: Bag) {
        debug_assert_eq!(other.width, self.width, "bag width");
        if self.is_empty() {
            *self = other;
        } else {
            self.data.extend_from_slice(&other.data);
            self.len += other.len;
        }
    }

    /// The rows at `indices`, in that order.
    pub(crate) fn select(&self, indices: impl IntoIterator<Item = usize>) -> Bag {
        let mut out = Bag::new(self.width);
        for i in indices {
            out.push(self.row(i));
        }
        out
    }
}

/// μ1 and μ2 are compatible iff every slot bound in both holds the same term (§18.3).
#[inline]
pub(crate) fn compatible(a: &[TermId], b: &[TermId]) -> bool {
    a.iter().zip(b).all(|(x, y)| !x.is_bound() || !y.is_bound() || x == y)
}

/// μ1 ∪ μ2 for compatible mappings: copies the bound slots of `src` into `dst`.
#[inline]
pub(crate) fn merge_into(dst: &mut [TermId], src: &[TermId]) {
    for (d, s) in dst.iter_mut().zip(src) {
        if s.is_bound() {
            *d = *s;
        }
    }
}

/// Binds `slot` to `value` in `row` unless it is already bound to a different term.
/// Returns false on that conflict (the extended mapping would not be a function).
#[inline]
pub(crate) fn bind(row: &mut [TermId], slot: Slot, value: TermId) -> bool {
    let cell = &mut row[slot as usize];
    if cell.is_bound() {
        *cell == value
    } else {
        *cell = value;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(v: i64) -> TermId {
        TermId::from_int(v).unwrap()
    }

    #[test]
    fn compatibility_ignores_unbound_slots() {
        let u = TermId::UNBOUND;
        assert!(compatible(&[t(1), u, t(3)], &[t(1), t(2), u]));
        assert!(!compatible(&[t(1), u], &[t(2), u]));
        assert!(compatible(&[u, u], &[u, u]));
        let mut d = [t(1), u, u];
        merge_into(&mut d, &[u, t(2), u]);
        assert_eq!(d, [t(1), t(2), u]);
    }

    #[test]
    fn bind_refuses_conflicts() {
        let mut r = [TermId::UNBOUND, t(5)];
        assert!(bind(&mut r, 0, t(1)));
        assert!(bind(&mut r, 0, t(1)));
        assert!(!bind(&mut r, 1, t(6)));
        assert_eq!(r, [t(1), t(5)]);
    }

    #[test]
    fn zero_width_rows_are_counted() {
        let mut b = Bag::new(0);
        b.push(&[]);
        b.push(&[]);
        assert_eq!(b.len(), 2);
        assert_eq!(b.rows().count(), 2);
        let mut c = Bag::single(&[]);
        c.append(b);
        assert_eq!(c.len(), 3);
        assert_eq!(c.select([0, 2]).len(), 2);
    }
}
