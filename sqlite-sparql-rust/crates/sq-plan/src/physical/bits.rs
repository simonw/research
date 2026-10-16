//! `Bits`: a set of small indices (the units or filter conjuncts of one join group), stored
//! inline up to 128 members so that the ordering search copies sets without allocating.
use smallvec::SmallVec;

#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Bits(SmallVec<[u64; 2]>);

impl Bits {
    pub(crate) fn single(i: usize) -> Bits {
        let mut b = Bits::default();
        b.insert(i);
        b
    }

    pub(crate) fn contains(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w >> (i % 64) & 1 == 1)
    }

    pub(crate) fn insert(&mut self, i: usize) {
        let w = i / 64;
        if self.0.len() <= w {
            self.0.resize(w + 1, 0);
        }
        self.0[w] |= 1 << (i % 64);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(|w| *w == 0)
    }

    /// `self ∩ other`, normalized (no trailing zero words) so that equal sets hash equally.
    pub(crate) fn intersect(&self, other: &Bits) -> Bits {
        let mut len = self.0.len().min(other.0.len());
        while len > 0 && self.0[len - 1] & other.0[len - 1] == 0 {
            len -= 1;
        }
        let mut out = SmallVec::with_capacity(len);
        for w in 0..len {
            out.push(self.0[w] & other.0[w]);
        }
        Bits(out)
    }

    /// The set as one word, when every member is below 64.
    pub(crate) fn as_word(&self) -> Option<u64> {
        match self.0.as_slice() {
            [] => Some(0),
            [w] => Some(*w),
            _ => None,
        }
    }

    /// Members in ascending order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(w, &word)| {
            let mut bits = word;
            std::iter::from_fn(move || {
                (bits != 0).then(|| {
                    let i = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    w * 64 + i
                })
            })
        })
    }
}

/// Members of a `usize` mask in ascending order (DP subsets of at most 64 units).
pub(crate) fn mask_iter(mut mask: usize) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        (mask != 0).then(|| {
            let i = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            i
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_operations() {
        let mut a = Bits::single(3);
        a.insert(70);
        a.insert(130);
        assert!(a.contains(3) && a.contains(70) && a.contains(130) && !a.contains(4));
        assert_eq!(a.iter().collect::<Vec<_>>(), vec![3, 70, 130]);
        let mut b = Bits::single(3);
        b.insert(131);
        let both = a.intersect(&b);
        assert_eq!(both, Bits::single(3), "normalized: no trailing zero words");
        assert!(a.intersect(&Bits::single(5)).is_empty());
        assert!(Bits::default().is_empty());
        assert_eq!(Bits::single(5).as_word(), Some(1 << 5));
        assert_eq!(Bits::default().as_word(), Some(0));
        assert_eq!(a.as_word(), None);
        assert_eq!(mask_iter(0b1010_0001).collect::<Vec<_>>(), vec![0, 5, 7]);
    }
}
