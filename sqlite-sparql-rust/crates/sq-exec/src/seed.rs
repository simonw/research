//! Seeds (§5.1): values bound by the caller when an operator subtree is (re)opened.
//!
//! A seed is a full-width row in which exactly the seeded slots are bound. Each seeded slot is
//! either a **Join-mode** seed (sideways information passing from a BindJoin: the subtree
//! behaves as if the slot were bound by a join partner, and its rows carry the value) or a
//! **Substitute-mode** seed (EXISTS, spec 18.6: the slot behaves as a constant everywhere in
//! the pattern, including OPTIONAL / MINUS / FILTER and sub-selects). Both kinds can be live at
//! once: a BindJoin inside an EXISTS pattern opens its right input with its own Join-mode seed
//! on top of the substitution.
//!
//! Propagation (the planner's seed contract, `sq_plan::physical`): leaves (Unit, Scan, Values,
//! Path, GraphNames) merge the whole seed into their rows; Join-mode seeds reach every input of
//! an inner join, the left input only of Left / Semi / Anti joins and Minus, every Union
//! branch, and the input of Filter / Extend / Project / Distinct / Reduced / Sort / SortBySlot
//! / Group; Slice, TopK, Count and GraphLoop never pass them on. Substitution reaches every
//! input of every operator, and operators that reset slots (Project, Group, Count) put the
//! substituted constants back on their output rows.
use sq_core::{Slot, SlotSet, TermId};

/// A borrowed seed: `row` has exactly the seeded slots bound; `subst` marks the slots seeded
/// in `SeedMode::Substitute` (every other bound slot of `row` is a Join-mode seed).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Seed<'a> {
    pub row: &'a [TermId],
    pub subst: SlotSet,
}

impl<'a> Seed<'a> {
    /// A Join-mode seed (`SeedMode::Join`) of every bound slot of `row`.
    #[cfg(test)]
    pub fn join(row: &'a [TermId]) -> Self {
        Seed { row, subst: SlotSet::EMPTY }
    }
}

/// An operator's copy of the seed it was opened with.
///
/// `base` is the full-width row of seeded values (all UNBOUND without a seed): the first image
/// of every row a leaf produces. `subst_row` holds only the substituted values, for inputs that
/// receive the substitution but not the Join-mode seed.
#[derive(Clone, Debug)]
pub(crate) struct SeedBuf {
    base: Vec<TermId>,
    subst_row: Vec<TermId>,
    subst: SlotSet,
    /// Seeded slots (both modes), in slot order.
    slots: Vec<Slot>,
    /// Substituted slots, in slot order.
    subst_slots: Vec<Slot>,
}

impl SeedBuf {
    pub fn new(width: usize) -> Self {
        SeedBuf {
            base: vec![TermId::UNBOUND; width],
            subst_row: vec![TermId::UNBOUND; width],
            subst: SlotSet::EMPTY,
            slots: Vec::new(),
            subst_slots: Vec::new(),
        }
    }

    /// Takes a copy of `seed` (no allocation once the buffers have grown).
    pub fn set(&mut self, seed: Option<Seed<'_>>) {
        for s in self.slots.drain(..) {
            self.base[s as usize] = TermId::UNBOUND;
        }
        for s in self.subst_slots.drain(..) {
            self.subst_row[s as usize] = TermId::UNBOUND;
        }
        self.subst = SlotSet::EMPTY;
        let Some(seed) = seed else { return };
        for (i, v) in seed.row.iter().enumerate().take(self.base.len()) {
            if v.is_bound() {
                let s = i as Slot;
                self.base[i] = *v;
                self.slots.push(s);
                if seed.subst.contains(s) {
                    self.subst_row[i] = *v;
                    self.subst_slots.push(s);
                    self.subst.insert(s);
                }
            }
        }
    }

    /// The full seed (both modes), if any slot is seeded.
    pub fn full(&self) -> Option<Seed<'_>> {
        (!self.slots.is_empty()).then_some(Seed { row: &self.base, subst: self.subst })
    }

    /// Only the substituted slots, if any (inputs that never receive Join-mode seeds).
    pub fn subst_only(&self) -> Option<Seed<'_>> {
        (!self.subst_slots.is_empty()).then_some(Seed { row: &self.subst_row, subst: self.subst })
    }

    /// Full-width row of the seeded values (all UNBOUND without a seed).
    #[inline]
    pub fn base(&self) -> &[TermId] {
        &self.base
    }

    /// Seeded value of `s` (either mode).
    #[inline]
    pub fn value(&self, s: Slot) -> Option<TermId> {
        self.base.get(s as usize).copied().filter(|v| v.is_bound())
    }

    #[inline]
    pub fn is_subst(&self, s: Slot) -> bool {
        self.subst.contains(s)
    }

    pub fn subst(&self) -> SlotSet {
        self.subst
    }

    /// Substituted slots, in slot order.
    pub fn subst_slots(&self) -> &[Slot] {
        &self.subst_slots
    }

    /// Writes the substituted constants into `row` where it is unbound (Project / Group /
    /// Count outputs inside an EXISTS pattern).
    #[inline]
    pub fn refill_subst(&self, row: &mut [TermId]) {
        for &s in &self.subst_slots {
            let v = &mut row[s as usize];
            if !v.is_bound() {
                *v = self.subst_row[s as usize];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_full_and_subst_only() {
        let mut b = SeedBuf::new(4);
        assert!(b.full().is_none() && b.subst_only().is_none());
        let row = [TermId(16 | 1), TermId::UNBOUND, TermId(32 | 1), TermId::UNBOUND];
        b.set(Some(Seed { row: &row, subst: SlotSet::from_slots([2]) }));
        assert_eq!(b.value(0), Some(TermId(17)));
        assert_eq!(b.value(1), None);
        assert!(b.is_subst(2) && !b.is_subst(0));
        let only = b.subst_only().unwrap();
        assert_eq!(only.row, &[TermId::UNBOUND, TermId::UNBOUND, TermId(33), TermId::UNBOUND]);
        let mut out = [TermId::UNBOUND, TermId::UNBOUND, TermId(49), TermId::UNBOUND];
        b.refill_subst(&mut out);
        assert_eq!(out[2], TermId(49), "bound slots are kept");
        let mut out = [TermId::UNBOUND; 4];
        b.refill_subst(&mut out);
        assert_eq!(out[2], TermId(33));
        b.set(None);
        assert_eq!(b.base(), &[TermId::UNBOUND; 4]);
        assert!(b.full().is_none());
    }
}
