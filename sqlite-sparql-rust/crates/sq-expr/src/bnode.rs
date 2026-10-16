//! `BNODE(str)` scope (§6.6, SPARQL 1.1 §17.4.2.9): the same simple literal gives the same
//! blank node within one solution, across every expression evaluated on it (several SELECT
//! expressions, chained BINDs), and distinct blank nodes for different solutions.
//!
//! The evaluator sees rows, not solutions. Operators run batch by batch, so the Extends of one
//! solution interleave with other solutions' rows, and a solution is keyed by its row content.
//! A row that an earlier `BNODE(str)` binding extended still belongs to the solution the blank
//! node was generated for, so such blank nodes are masked out of the key. This happens only
//! along the row's own lineage: a generated blank node is masked when the row, with it and its
//! siblings from the same solution set back to UNBOUND, is exactly that solution's key. A
//! blank node generated for another solution (for example one carried out of a sub-select) is
//! an ordinary value that tells solutions apart.
//!
//! Memory is bounded in bytes by two generations. When the current generation is over budget
//! at the start of a new solution, it becomes the previous generation and the old previous one
//! is dropped. Lookups consult both, and a solution found in the previous generation moves back
//! into the current one. So a solution in use is never split, and every solution outlives at
//! least a full generation of newer ones, which is far more than one batch.
//!
//! Limitation: duplicate solutions of a multiset (`VALUES ?x { 1 1 }`) have identical rows and
//! share their blank nodes. Telling them apart needs a solution identity the batch-wise host
//! cannot supply cheaply. Counting evaluations instead would wrongly give a re-evaluated
//! subtree (a BindJoin probe, the adaptive HashJoin fallback) fresh blank nodes for the same
//! solution.
use std::sync::Arc;

use rustc_hash::FxHashMap;
use sq_core::TermId;

/// Default byte budget of one generation (the scope holds at most about twice this).
const DEFAULT_BUDGET: usize = 8 << 20;
/// Approximate fixed heap cost of one solution entry besides its key slots (key allocation
/// header, map slots, label vector header).
const SOLUTION_OVERHEAD: usize = 96;
/// Approximate heap cost of one (label, blank node) entry besides the label's bytes (vector
/// slot, origin map slot, label allocation header).
const LABEL_OVERHEAD: usize = 64;

/// A solution's key: its row with the lineage blank nodes set to UNBOUND.
type Key = Arc<[TermId]>;
/// The blank nodes generated for one solution, by label.
type Labels = Vec<(Arc<str>, TermId)>;

#[derive(Default)]
struct Generation {
    /// Solution key → its blank nodes.
    solutions: FxHashMap<Key, Labels>,
    /// Generated blank node → key of the solution it was generated for. Always in the same
    /// generation as that solution.
    origin: FxHashMap<TermId, Key>,
    /// Approximate heap bytes held.
    bytes: usize,
}

/// Approximate heap bytes of one solution entry.
fn solution_bytes(key: &[TermId], labels: &Labels) -> usize {
    SOLUTION_OVERHEAD
        + std::mem::size_of_val(key)
        + labels.iter().map(|(l, _)| LABEL_OVERHEAD + l.len()).sum::<usize>()
}

fn find(labels: &Labels, label: &str) -> Option<TermId> {
    labels.iter().find(|(l, _)| **l == *label).map(|(_, id)| *id)
}

/// Same solution key (keys are shared `Arc`s, so the pointer test almost always decides).
fn same_key(a: &Key, b: &Key) -> bool {
    Arc::ptr_eq(a, b) || a[..] == b[..]
}

/// Per-execution `BNODE(str)` scope (held by the `ValueCache`).
pub(crate) struct BNodeScope {
    current: Generation,
    previous: Generation,
    budget: usize,
}

impl Default for BNodeScope {
    fn default() -> Self {
        Self::with_budget(DEFAULT_BUDGET)
    }
}

impl BNodeScope {
    /// A scope whose generations hold about `budget` bytes each.
    pub(crate) fn with_budget(budget: usize) -> Self {
        BNodeScope { current: Generation::default(), previous: Generation::default(), budget }
    }

    /// The blank node for `label` in the solution of `row`; `fresh` makes a new one.
    pub(crate) fn get(&mut self, row: &[TermId], label: &Arc<str>, fresh: impl FnOnce() -> TermId) -> TermId {
        let lineage = self.lineage(row);
        let key_slots: &[TermId] = lineage.as_deref().unwrap_or(row);
        // The common case: a known solution of the current generation, no allocation.
        if let Some(id) = self.current.solutions.get(key_slots).and_then(|labels| find(labels, label)) {
            return id;
        }
        let key = self.make_current(key_slots);
        let labels = self.current.solutions.get_mut(&key).expect("solution made current");
        if let Some(id) = find(labels, label) {
            return id;
        }
        let id = fresh();
        labels.push((label.clone(), id));
        self.current.bytes += LABEL_OVERHEAD + label.len();
        self.current.origin.insert(id, key);
        id
    }

    /// Approximate heap bytes held by both generations.
    #[cfg(test)]
    pub(crate) fn bytes(&self) -> usize {
        self.current.bytes + self.previous.bytes
    }

    /// Key of the solution a generated blank node belongs to.
    fn origin(&self, id: TermId) -> Option<&Key> {
        self.current.origin.get(&id).or_else(|| self.previous.origin.get(&id))
    }

    /// The key of the solution that `row` extends with blank nodes generated for it, if any
    /// (None: the row itself is the key).
    fn lineage(&self, row: &[TermId]) -> Option<Key> {
        let mut tried: Option<&Key> = None;
        for &id in row {
            if !id.is_bnode() {
                continue;
            }
            let Some(key) = self.origin(id) else { continue };
            if tried.is_some_and(|t| Arc::ptr_eq(t, key)) {
                continue;
            }
            if self.extends(row, key) {
                return Some(key.clone());
            }
            tried = Some(key);
        }
        None
    }

    /// True if `row` is `key` with some of its UNBOUND slots bound to blank nodes generated for
    /// `key`.
    fn extends(&self, row: &[TermId], key: &Key) -> bool {
        row.len() == key.len()
            && row.iter().zip(key.iter()).all(|(r, k)| {
                r == k || (*k == TermId::UNBOUND && self.origin(*r).is_some_and(|o| same_key(o, key)))
            })
    }

    /// Makes the solution `key_slots` present in the current generation (moving it from the
    /// previous one, or starting it) and returns its shared key. A lineage key always names a
    /// known solution, so only a row key starts a new one.
    fn make_current(&mut self, key_slots: &[TermId]) -> Key {
        if let Some((key, _)) = self.current.solutions.get_key_value(key_slots) {
            return key.clone();
        }
        if let Some((key, labels)) = self.previous.solutions.remove_entry(key_slots) {
            let bytes = solution_bytes(&key, &labels);
            self.previous.bytes = self.previous.bytes.saturating_sub(bytes);
            self.current.bytes += bytes;
            for (_, id) in &labels {
                if let Some(origin) = self.previous.origin.remove(id) {
                    self.current.origin.insert(*id, origin);
                }
            }
            self.current.solutions.insert(key.clone(), labels);
            return key;
        }
        // A new solution: rotate first, so that no solution is split across a drop.
        if self.current.bytes >= self.budget {
            self.previous = std::mem::take(&mut self.current);
        }
        let key: Key = key_slots.into();
        self.current.bytes += solution_bytes(&key, &Labels::new());
        self.current.solutions.insert(key.clone(), Labels::new());
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const U: TermId = TermId::UNBOUND;

    /// A scope plus a fresh-blank-node counter (TEMP blank-node ids).
    struct Fixture {
        scope: BNodeScope,
        next: u32,
    }

    impl Fixture {
        fn new(budget: usize) -> Self {
            Fixture { scope: BNodeScope::with_budget(budget), next: 0 }
        }

        fn get(&mut self, row: &[TermId], label: &str) -> TermId {
            let next = &mut self.next;
            self.scope.get(row, &Arc::from(label), || {
                *next += 1;
                TermId::temp(*next, sq_core::Tag::BNode)
            })
        }
    }

    fn int(i: i64) -> TermId {
        TermId::from_int(i).unwrap()
    }

    #[test]
    fn same_label_same_solution_across_chained_extends() {
        let mut f = Fixture::new(DEFAULT_BUDGET);
        let mut row = vec![int(1), U, U, U];
        let b1 = f.get(&row, "x");
        row[1] = b1;
        // A second expression on the extended row: same solution.
        assert_eq!(f.get(&row, "x"), b1);
        let other = f.get(&row, "y");
        assert_ne!(other, b1);
        row[2] = other;
        assert_eq!(f.get(&row, "y"), other);
        assert_eq!(f.get(&row, "x"), b1);
        // Another solution gets fresh blank nodes.
        assert_ne!(f.get(&[int(2), U, U, U], "x"), b1);
    }

    #[test]
    fn foreign_generated_blank_nodes_distinguish_solutions() {
        let mut f = Fixture::new(DEFAULT_BUDGET);
        // Sub-select: two solutions, ?v projected away afterwards.
        let k1 = f.get(&[int(1), U, U], "k");
        let k2 = f.get(&[int(2), U, U], "k");
        assert_ne!(k1, k2);
        let a1 = f.get(&[U, k1, U], "a");
        let a2 = f.get(&[U, k2, U], "a");
        assert_ne!(a1, a2);
        // Lineage continues from the outer solutions.
        assert_eq!(f.get(&[U, k1, a1], "a"), a1);
        assert_eq!(f.get(&[U, k2, a2], "a"), a2);
        // Binding more UNBOUND slots to k1 (`BIND(?b1 AS ?b2)`) stays in its solution...
        assert_eq!(f.get(&[int(1), k1, k1], "k"), k1);
        // ...but a row where a slot that was bound when k1 was generated changed is another
        // solution.
        assert_ne!(f.get(&[k1, k1, U], "k"), k1);
    }

    #[test]
    fn mixed_lineage_masks_only_own_blank_nodes() {
        let mut f = Fixture::new(DEFAULT_BUDGET);
        let foreign = f.get(&[int(7), U, U, U], "f");
        // Outer solution carries `foreign` in slot 1; it generates `g` into slot 2.
        let g = f.get(&[U, foreign, U, U], "x");
        assert_ne!(g, foreign);
        assert_eq!(f.get(&[U, foreign, g, U], "x"), g);
        let h = f.get(&[U, foreign, g, U], "y");
        assert_eq!(f.get(&[U, foreign, g, h], "y"), h);
        assert_eq!(f.get(&[U, foreign, g, h], "x"), g);
    }

    #[test]
    fn memory_is_bounded_and_live_solutions_survive_rotation() {
        let budget = 4096;
        let mut f = Fixture::new(budget);
        let width = 8;
        let row = |i: i64| -> Vec<TermId> {
            let mut r = vec![U; width];
            r[0] = int(i);
            r
        };
        // A long-lived solution, touched once per round.
        let mut live = row(-1);
        let b = f.get(&live, "live");
        live[1] = b;
        for i in 0..10_000 {
            f.get(&row(i), "x");
            if i % 16 == 0 {
                assert_eq!(f.get(&live, "live"), b, "row {i}");
            }
            assert!(f.scope.bytes() <= 2 * budget + 1024, "bytes {}", f.scope.bytes());
        }
        // Batch-wise interleaving: a whole batch of new solutions, then a second expression
        // over the same batch, sees the same blank nodes.
        let batch: Vec<Vec<TermId>> = (0..16).map(|i| row(100_000 + i)).collect();
        let first: Vec<TermId> = batch.iter().map(|r| f.get(r, "z")).collect();
        for (r, b) in batch.iter().zip(&first) {
            let mut r = r.clone();
            r[1] = *b;
            assert_eq!(f.get(&r, "z"), *b);
        }
    }
}
