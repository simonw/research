//! Solution rows: one global slot layout per query; a row is `[TermId; width]`, 0 = unbound.
use crate::id::TermId;

/// Index of a variable (user, blank-node, path-middle, renamed sub-select, aggregate) in the
/// query-wide slot layout.
pub type Slot = u16;
pub const BATCH_ROWS: usize = 1024;

/// Row-major batch of solutions.
#[derive(Clone, Debug, Default)]
pub struct Batch {
    width: usize,
    len: usize,
    data: Vec<TermId>,
}

impl Batch {
    pub fn new(width: usize) -> Self {
        Self::with_capacity(width, BATCH_ROWS)
    }
    pub fn with_capacity(width: usize, rows: usize) -> Self {
        Batch { width, len: 0, data: Vec::with_capacity(width.max(1) * rows) }
    }
    #[inline]
    pub fn width(&self) -> usize {
        self.width
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
    pub fn is_full(&self) -> bool {
        self.len >= BATCH_ROWS
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
    pub fn get(&self, i: usize, s: Slot) -> TermId {
        self.data[i * self.width + s as usize]
    }
    #[inline]
    pub fn push_row(&mut self, r: &[TermId]) {
        debug_assert_eq!(r.len(), self.width);
        self.data.extend_from_slice(r);
        self.len += 1;
    }
    /// Append an all-unbound row and return it for filling.
    #[inline]
    pub fn push_unbound(&mut self) -> &mut [TermId] {
        let start = self.data.len();
        self.data.resize(start + self.width, TermId::UNBOUND);
        self.len += 1;
        &mut self.data[start..]
    }
    /// Copy `base`, then overwrite (slot, value) pairs: the INL / merge-join emit primitive.
    #[inline]
    pub fn push_extended(&mut self, base: &[TermId], sets: &[(Slot, TermId)]) {
        let start = self.data.len();
        self.data.extend_from_slice(base);
        for &(s, v) in sets {
            self.data[start + s as usize] = v;
        }
        self.len += 1;
    }
    pub fn clear(&mut self) {
        self.data.clear();
        self.len = 0;
    }
    pub fn truncate(&mut self, rows: usize) {
        if rows < self.len {
            self.data.truncate(rows * self.width);
            self.len = rows;
        }
    }
    pub fn rows(&self) -> std::slice::ChunksExact<'_, TermId> {
        self.data[..self.len * self.width].chunks_exact(self.width.max(1))
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&[TermId]) -> bool) {
        let w = self.width;
        let mut out = 0;
        for i in 0..self.len {
            if keep(&self.data[i * w..(i + 1) * w]) {
                if out != i {
                    self.data.copy_within(i * w..(i + 1) * w, out * w);
                }
                out += 1;
            }
        }
        self.truncate(out);
    }
}

/// SPARQL compatibility merge: for every slot in `check` (slots possibly bound on both sides)
/// both values must be equal when both are bound; then every bound slot of `src` is copied
/// into `dst`. Returns false (dst unspecified) if incompatible.
#[inline]
pub fn merge_compatible(dst: &mut [TermId], src: &[TermId], check: &[Slot]) -> bool {
    for &s in check {
        let (a, b) = (dst[s as usize], src[s as usize]);
        if a.is_bound() && b.is_bound() && a != b {
            return false;
        }
    }
    for (d, s) in dst.iter_mut().zip(src) {
        if s.is_bound() {
            *d = *s;
        }
    }
    true
}

/// Fixed-size set of slots (queries have at most 256 slots; more is Error::Unsupported).
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub struct SlotSet(pub [u64; 4]);

impl SlotSet {
    pub const EMPTY: SlotSet = SlotSet([0; 4]);
    pub const MAX_SLOTS: usize = 256;
    #[inline]
    pub fn insert(&mut self, s: Slot) {
        self.0[(s >> 6) as usize] |= 1 << (s & 63);
    }
    #[inline]
    pub fn remove(&mut self, s: Slot) {
        self.0[(s >> 6) as usize] &= !(1 << (s & 63));
    }
    #[inline]
    pub fn contains(&self, s: Slot) -> bool {
        self.0[(s >> 6) as usize] >> (s & 63) & 1 == 1
    }
    pub fn union(self, o: SlotSet) -> SlotSet {
        SlotSet([self.0[0] | o.0[0], self.0[1] | o.0[1], self.0[2] | o.0[2], self.0[3] | o.0[3]])
    }
    pub fn intersect(self, o: SlotSet) -> SlotSet {
        SlotSet([self.0[0] & o.0[0], self.0[1] & o.0[1], self.0[2] & o.0[2], self.0[3] & o.0[3]])
    }
    pub fn minus(self, o: SlotSet) -> SlotSet {
        SlotSet([self.0[0] & !o.0[0], self.0[1] & !o.0[1], self.0[2] & !o.0[2], self.0[3] & !o.0[3]])
    }
    pub fn is_empty(&self) -> bool {
        self.0 == [0; 4]
    }
    pub fn is_subset(&self, o: &SlotSet) -> bool {
        self.minus(*o).is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = Slot> + '_ {
        (0..256u16).filter(|s| self.contains(*s))
    }
    pub fn from_slots(slots: impl IntoIterator<Item = Slot>) -> SlotSet {
        let mut s = SlotSet::EMPTY;
        for x in slots {
            s.insert(x);
        }
        s
    }
}
